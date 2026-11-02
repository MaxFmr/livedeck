//! Controller → action mapping, loaded from `config/mapping.toml`.
//!
//! Controls have readable names (`a.play`, `b.pad.fx.3`, `mixer.crossfader`, `a.jog.edge`…) and an
//! optional `shift` layer. Absolute controls can use soft-takeover (`pickup`): after the software
//! value changed elsewhere, the control only takes over once its physical position meets it.
//!
//! Runs on: control thread.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use engine::drums::DrumVoice;
use engine::repeat::Division;
use engine::sequencer::voice_bit;
use serde::Deserialize;

use crate::controller::{
    Button, ControllerEvent, Deck, DeckKnob, JogSurface, Led, MAX_14BIT, MixerControl, PadMode,
};

/// A physical control, independent of its current value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    Button(Deck, Button),
    Pad(Deck, PadMode, u8),
    Mixer(MixerControl),
    Knob(Deck, DeckKnob),
    Tempo(Deck),
    Jog(Deck, JogSurface),
    Vinyl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlId {
    pub control: Control,
    pub shift: bool,
}

/// What a control sends, normalized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Press(bool),
    /// 0.0..=1.0 from a 14-bit value.
    Absolute(f32),
    /// Jog ticks.
    Relative(i32),
}

/// Values the software owns and that controls may take over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Tempo,
    MasterGain,
    MetronomeLevel,
    DrumLevel,
}

/// Result of a mapped gesture, applied by the control thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    TransportToggle,
    TransportRestart,
    MetronomeToggle,
    /// Plays a drum voice; `ts_us` is the gesture's CoreMIDI host time (latency measurement).
    Drum {
        voice: DrumVoice,
        velocity: f32,
        ts_us: u64,
    },
    /// Drum pattern, 0-based.
    QueuePattern(u8),
    /// Swing change, as a fraction (0.01 = 1 %).
    NudgeSwing(f32),
    /// Mutes or unmutes a group of drum voices (bit `i` = `DrumVoice::ALL[i]`).
    ToggleMute(u8),
    /// Drum fill, 0-based.
    Fill(u8),
    /// Beat-repeat pad pressed (`held`) or released.
    BeatRepeat {
        division: Division,
        held: bool,
    },
    /// Drum density 0..=1.
    SetDensity(f32),
    /// Bass line alone, 0-based.
    QueueBassLine(u8),
    ToggleBassMute,
    /// Tonic change, in semitones.
    NudgeTonic(i8),
    /// Bass filter cutoff, 0..=1.
    SetBassCutoff(f32),
    SetTempo(f64),
    NudgeTempo(f64),
    /// Normalized 0..=1.
    SetLevel(Target, f32),
}

/// Action names as written in the mapping file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionName {
    TransportToggle,
    TransportRestart,
    TapTempo,
    MetronomeToggle,
    Drum,
    Pattern,
    Swing,
    Mute,
    Fill,
    BeatRepeat,
    DrumDensity,
    BassLine,
    BassMute,
    Tonic,
    BassCutoff,
    Tempo,
    TempoNudge,
    MasterGain,
    MetronomeLevel,
    DrumLevel,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindSpec {
    control: String,
    #[serde(default)]
    shift: bool,
    action: ActionName,
    /// Soft-takeover for absolute controls.
    #[serde(default)]
    pickup: bool,
    /// Value range for `tempo` (BPM).
    range: Option<[f64; 2]>,
    /// Amount per jog tick for `tempo_nudge` (BPM) or `swing` (%), per press for `tonic`
    /// (semitones).
    step: Option<f64>,
    /// Voice played by `drum` (kick, snare, closed_hat, open_hat, clap, perc).
    voice: Option<String>,
    /// Pattern (1–4) launched by `pattern`, or bass line by `bass_line`.
    pattern: Option<u8>,
    /// Voices silenced together by `mute`.
    voices: Option<Vec<String>>,
    /// Fill (1–4) played by `fill`.
    fill: Option<u8>,
    /// Note value repeated by `beat_repeat`: 4, 8, 16 or 32 (1/4 … 1/32).
    division: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingSpec {
    #[serde(default)]
    bind: Vec<BindSpec>,
}

#[derive(Debug, thiserror::Error)]
pub enum MappingError {
    #[error("lecture de {path} : {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("syntaxe du mapping : {0}")]
    Toml(#[from] toml::de::Error),
    #[error("contrôle inconnu « {0} » (ex. a.play, b.pad.fx.3, mixer.crossfader, a.jog.edge)")]
    UnknownControl(String),
    #[error("« {control} » : l'action {action} ne convient pas à ce contrôle")]
    Incompatible { control: String, action: String },
    #[error("« {0} » est lié deux fois (même couche shift)")]
    Duplicate(String),
    #[error(
        "« {control} » : voix de batterie « {voice} » inconnue (kick, snare, closed_hat, open_hat, clap, perc)"
    )]
    UnknownVoice { control: String, voice: String },
    #[error("« {0} » : pattern attendu entre 1 et 4")]
    BadPattern(String),
    #[error("« {0} » : fill attendu entre 1 et 4")]
    BadFill(String),
    #[error("« {0} » : division attendue parmi 4, 8, 16, 32")]
    BadDivision(String),
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    Press,
    Absolute,
    Relative,
}

#[derive(Debug, Clone, Copy)]
struct Binding {
    action: ActionName,
    pickup: bool,
    range: [f64; 2],
    step: f64,
    voice: DrumVoice,
    /// 0-based.
    pattern: u8,
    /// Voices of `mute`, bit `i` = `DrumVoice::ALL[i]`.
    mask: u8,
    /// 0-based.
    fill: u8,
    division: Division,
}

/// Soft-takeover state of one absolute control.
#[derive(Debug, Clone, Copy, Default)]
struct Pickup {
    caught: bool,
    last_hw: Option<f32>,
}

/// Physical touch must last this long before top-of-jog ticks count as "top": while spinning the
/// edge fast, the touch sensor flickers (verified in iteration 003).
pub const TOUCH_CONFIRM_US: u64 = 80_000;
/// Taps further apart than this start a new tap sequence.
const TAP_RESET_US: u64 = 2_000_000;
/// Pickup catches when the control is within this distance of the software value.
const PICKUP_WINDOW: f32 = 0.02;

pub struct Mapping {
    bindings: HashMap<ControlId, Binding>,
    pickups: HashMap<ControlId, Pickup>,
    soft: HashMap<Target, f32>,
    touch_down_us: [Option<u64>; 2],
    /// Top ticks received while the touch is not yet confirmed: they go to the top once it is,
    /// or to the edge if the touch ends first (sensor flicker while spinning the edge).
    held_ticks: [i32; 2],
    taps: Vec<u64>,
}

/// Default mapping, compiled in (used when `config/mapping.toml` is missing).
pub const DEFAULT_MAPPING: &str = include_str!("../../../config/mapping.toml");

impl Mapping {
    pub fn from_toml_str(text: &str) -> Result<Self, MappingError> {
        let spec: MappingSpec = toml::from_str(text)?;
        let mut bindings = HashMap::new();
        for b in spec.bind {
            let control = parse_control(&b.control)
                .ok_or_else(|| MappingError::UnknownControl(b.control.clone()))?;
            if !compatible(kind(control), b.action) {
                return Err(MappingError::Incompatible {
                    control: b.control,
                    action: format!("{:?}", b.action),
                });
            }
            let id = ControlId {
                control,
                shift: b.shift,
            };
            let voice_name = b.voice.as_deref().unwrap_or("kick");
            let voice =
                DrumVoice::from_name(voice_name).ok_or_else(|| MappingError::UnknownVoice {
                    control: b.control.clone(),
                    voice: voice_name.to_string(),
                })?;
            let pattern = match b.pattern {
                None => 0,
                Some(p @ 1..=4) => p - 1,
                Some(_) => return Err(MappingError::BadPattern(b.control)),
            };
            let fill = match b.fill {
                None => 0,
                Some(f @ 1..=4) => f - 1,
                Some(_) => return Err(MappingError::BadFill(b.control)),
            };
            let division = match b.division {
                None => Division::Sixteenth,
                Some(n) => Division::from_denominator(n)
                    .ok_or_else(|| MappingError::BadDivision(b.control.clone()))?,
            };
            let mut mask = 0;
            for name in b.voices.iter().flatten() {
                let voice =
                    DrumVoice::from_name(name).ok_or_else(|| MappingError::UnknownVoice {
                        control: b.control.clone(),
                        voice: name.clone(),
                    })?;
                mask |= voice_bit(voice);
            }
            let binding = Binding {
                action: b.action,
                pickup: b.pickup,
                range: b.range.unwrap_or([60.0, 200.0]),
                step: b.step.unwrap_or(0.1),
                voice,
                pattern,
                mask,
                fill,
                division,
            };
            if bindings.insert(id, binding).is_some() {
                return Err(MappingError::Duplicate(b.control));
            }
        }
        Ok(Self {
            bindings,
            pickups: HashMap::new(),
            soft: HashMap::new(),
            touch_down_us: [None; 2],
            held_ticks: [0; 2],
            taps: Vec::with_capacity(8),
        })
    }

    pub fn load(path: &Path) -> Result<Self, MappingError> {
        let text = std::fs::read_to_string(path).map_err(|source| MappingError::Read {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Controls bound to `action` without Shift, with their value: pattern or fill index
    /// (0-based), voice mask of a mute, denominator of a beat-repeat.
    pub fn indexed(&self, action: ActionName) -> impl Iterator<Item = (Control, u8)> + '_ {
        self.bindings
            .iter()
            .filter(move |(id, b)| !id.shift && b.action == action)
            .map(|(id, b)| {
                let value = match b.action {
                    ActionName::Fill => b.fill,
                    ActionName::Mute => b.mask,
                    ActionName::BeatRepeat => b.division.denominator(),
                    _ => b.pattern,
                };
                (id.control, value)
            })
    }

    /// Controls bound to `action` without Shift (the layer their LED shows).
    pub fn bound(&self, action: ActionName) -> impl Iterator<Item = Control> + '_ {
        self.bindings
            .iter()
            .filter(move |(id, b)| !id.shift && b.action == action)
            .map(|(id, _)| id.control)
    }

    /// The software changed `target` (keyboard, tap tempo, scene…): controls bound to it with
    /// pickup must meet the new value before taking over again. `value` is normalized for levels,
    /// in BPM for the tempo.
    pub fn software_changed(&mut self, target: Target, value: f64) {
        self.soft.insert(target, value as f32);
        for (id, binding) in &self.bindings {
            if binding.pickup && target_of(binding.action) == Some(target) {
                let soft = normalized(binding, target, value);
                let state = self.pickups.entry(*id).or_default();
                // Already sitting on the new value: nothing to catch.
                state.caught = state
                    .last_hw
                    .is_some_and(|hw| (hw - soft).abs() <= PICKUP_WINDOW);
            }
        }
    }

    /// Maps one decoded event received at `ts_us` (CoreMIDI host time, µs).
    pub fn handle(&mut self, ts_us: u64, event: ControllerEvent) -> Option<Action> {
        let (mut id, mut input) = match self.jog_touch(ts_us, event) {
            Some(released) => released,
            None => identify(event)?,
        };
        if let (Control::Jog(deck, JogSurface::Top), Input::Relative(ticks)) = (id.control, input) {
            let d = deck_index(deck);
            match self.touch_down_us.get(d).copied().flatten() {
                Some(down) if ts_us.saturating_sub(down) >= TOUCH_CONFIRM_US => {
                    let held = self.held_ticks.get_mut(d).map_or(0, std::mem::take);
                    input = Input::Relative(ticks + held);
                }
                Some(_) => {
                    if let Some(held) = self.held_ticks.get_mut(d) {
                        *held += ticks;
                    }
                    return None;
                }
                None => id.control = Control::Jog(deck, JogSurface::Edge),
            }
        }
        let binding = *self.bindings.get(&id)?;
        match (binding.action, input) {
            (ActionName::TransportToggle, Input::Press(true)) => Some(Action::TransportToggle),
            (ActionName::TransportRestart, Input::Press(true)) => Some(Action::TransportRestart),
            (ActionName::MetronomeToggle, Input::Press(true)) => Some(Action::MetronomeToggle),
            (ActionName::Drum, Input::Press(true)) => Some(Action::Drum {
                voice: binding.voice,
                velocity: velocity_of(event),
                ts_us,
            }),
            (ActionName::TapTempo, Input::Press(true)) => self.tap(ts_us),
            (ActionName::Pattern, Input::Press(true)) => {
                Some(Action::QueuePattern(binding.pattern))
            }
            (ActionName::Mute, Input::Press(true)) => Some(Action::ToggleMute(binding.mask)),
            (ActionName::Fill, Input::Press(true)) => Some(Action::Fill(binding.fill)),
            (ActionName::BeatRepeat, Input::Press(held)) => Some(Action::BeatRepeat {
                division: binding.division,
                held,
            }),
            (ActionName::DrumDensity, Input::Absolute(v)) => Some(Action::SetDensity(v)),
            (ActionName::BassLine, Input::Press(true)) => {
                Some(Action::QueueBassLine(binding.pattern))
            }
            (ActionName::BassMute, Input::Press(true)) => Some(Action::ToggleBassMute),
            (ActionName::Tonic, Input::Press(true)) => Some(Action::NudgeTonic(
                binding.step.round().clamp(-11.0, 11.0) as i8,
            )),
            (ActionName::BassCutoff, Input::Absolute(v)) => Some(Action::SetBassCutoff(v)),
            (ActionName::Swing, Input::Relative(ticks)) => Some(Action::NudgeSwing(
                ticks as f32 * binding.step as f32 / 100.0,
            )),
            (ActionName::TempoNudge, Input::Relative(ticks)) => {
                Some(Action::NudgeTempo(f64::from(ticks) * binding.step))
            }
            (ActionName::Tempo, Input::Absolute(v)) => {
                let [lo, hi] = binding.range;
                let to_bpm = |x: f32| lo + f64::from(x) * (hi - lo);
                let soft = self
                    .soft
                    .get(&Target::Tempo)
                    .map(|bpm| ((f64::from(*bpm) - lo) / (hi - lo)).clamp(0.0, 1.0) as f32);
                self.take_over(id, binding.pickup, soft, v)
                    .then(|| Action::SetTempo(to_bpm(v)))
            }
            (
                ActionName::MasterGain | ActionName::MetronomeLevel | ActionName::DrumLevel,
                Input::Absolute(v),
            ) => {
                let target = target_of(binding.action)?;
                let soft = self.soft.get(&target).copied();
                self.take_over(id, binding.pickup, soft, v)
                    .then_some(Action::SetLevel(target, v))
            }
            _ => None,
        }
    }

    /// Tracks jog touches; when a touch ends with held ticks, returns them as a gesture on the top
    /// (touch was confirmed) or the edge (flicker).
    fn jog_touch(&mut self, ts_us: u64, event: ControllerEvent) -> Option<(ControlId, Input)> {
        let ControllerEvent::Button {
            deck,
            button: Button::JogTouch,
            pressed,
            shift,
        } = event
        else {
            return None;
        };
        let d = deck_index(deck);
        let down = self.touch_down_us.get_mut(d)?;
        let held = self.held_ticks.get_mut(d)?;
        if pressed {
            *down = Some(ts_us);
            *held = 0;
            return None;
        }
        let confirmed = down
            .take()
            .is_some_and(|t| ts_us.saturating_sub(t) >= TOUCH_CONFIRM_US);
        let ticks = std::mem::take(held);
        (ticks != 0).then(|| {
            let surface = if confirmed {
                JogSurface::Top
            } else {
                JogSurface::Edge
            };
            (
                ControlId {
                    control: Control::Jog(deck, surface),
                    shift,
                },
                Input::Relative(ticks),
            )
        })
    }

    /// Soft-takeover: true when the control may apply `hw`.
    fn take_over(&mut self, id: ControlId, enabled: bool, soft: Option<f32>, hw: f32) -> bool {
        if !enabled {
            return true;
        }
        let state = self.pickups.entry(id).or_default();
        let last = state.last_hw.replace(hw);
        if state.caught {
            return true;
        }
        let Some(soft) = soft else {
            state.caught = true;
            return true;
        };
        let near = (hw - soft).abs() <= PICKUP_WINDOW;
        let crossed = last.is_some_and(|prev| (prev - soft) * (hw - soft) <= 0.0);
        state.caught = near || crossed;
        state.caught
    }

    /// The control currently waiting to take over `target`, with its physical position (0..=1).
    #[must_use]
    pub fn pending_pickup(&self, target: Target) -> Option<f32> {
        self.bindings.iter().find_map(|(id, b)| {
            let state = self.pickups.get(id)?;
            (b.pickup && target_of(b.action) == Some(target) && !state.caught)
                .then_some(state.last_hw)
                .flatten()
        })
    }

    fn tap(&mut self, ts_us: u64) -> Option<Action> {
        if self
            .taps
            .last()
            .is_some_and(|&last| ts_us.saturating_sub(last) > TAP_RESET_US)
        {
            self.taps.clear();
        }
        if self.taps.len() == 5 {
            self.taps.remove(0);
        }
        self.taps.push(ts_us);
        let (first, last) = (*self.taps.first()?, *self.taps.last()?);
        let intervals = self.taps.len().checked_sub(1).filter(|&n| n > 0)?;
        let mean_us = last.saturating_sub(first) as f64 / intervals as f64;
        if mean_us <= 0.0 {
            return None;
        }
        let bpm = 60_000_000.0 / mean_us;
        // A tapped tempo is a software change: the tempo fader must meet it before taking over.
        self.software_changed(Target::Tempo, bpm);
        Some(Action::SetTempo(bpm))
    }
}

/// Software value of `target` on the 0..=1 scale of `binding`'s control.
fn normalized(binding: &Binding, target: Target, value: f64) -> f32 {
    match target {
        Target::Tempo => {
            let [lo, hi] = binding.range;
            ((value - lo) / (hi - lo)).clamp(0.0, 1.0) as f32
        }
        Target::MasterGain | Target::MetronomeLevel | Target::DrumLevel => {
            value.clamp(0.0, 1.0) as f32
        }
    }
}

fn deck_index(deck: Deck) -> usize {
    match deck {
        Deck::A => 0,
        Deck::B => 1,
    }
}

fn target_of(action: ActionName) -> Option<Target> {
    match action {
        ActionName::Tempo | ActionName::TempoNudge | ActionName::TapTempo => Some(Target::Tempo),
        ActionName::MasterGain => Some(Target::MasterGain),
        ActionName::DrumLevel => Some(Target::DrumLevel),
        ActionName::MetronomeLevel => Some(Target::MetronomeLevel),
        _ => None,
    }
}

fn kind(control: Control) -> Kind {
    match control {
        Control::Button(..) | Control::Pad(..) | Control::Vinyl => Kind::Press,
        Control::Mixer(_) | Control::Knob(..) | Control::Tempo(_) => Kind::Absolute,
        Control::Jog(..) => Kind::Relative,
    }
}

fn compatible(kind: Kind, action: ActionName) -> bool {
    match action {
        ActionName::TransportToggle
        | ActionName::TransportRestart
        | ActionName::TapTempo
        | ActionName::MetronomeToggle
        | ActionName::Drum
        | ActionName::Pattern
        | ActionName::BassLine
        | ActionName::BassMute
        | ActionName::Tonic
        | ActionName::Mute
        | ActionName::Fill
        | ActionName::BeatRepeat => matches!(kind, Kind::Press),
        ActionName::Tempo
        | ActionName::DrumDensity
        | ActionName::BassCutoff
        | ActionName::MasterGain
        | ActionName::MetronomeLevel
        | ActionName::DrumLevel => {
            matches!(kind, Kind::Absolute)
        }
        ActionName::TempoNudge | ActionName::Swing => matches!(kind, Kind::Relative),
    }
}

fn normalize(value: u16) -> f32 {
    f32::from(value) / f32::from(MAX_14BIT)
}

/// Pad velocity as 0..=1 (the DJControl Mix always sends 127); buttons play at full velocity.
fn velocity_of(event: ControllerEvent) -> f32 {
    match event {
        ControllerEvent::Pad { velocity, .. } => f32::from(velocity.min(127)) / 127.0,
        _ => 1.0,
    }
}

/// The LED of a control, if it has one (the headphone button and continuous controls have none).
#[must_use]
pub fn led_of(control: Control) -> Option<Led> {
    match control {
        Control::Button(deck, Button::Sync) => Some(Led::Sync(deck)),
        Control::Button(deck, Button::Cue) => Some(Led::Cue(deck)),
        Control::Button(deck, Button::Play) => Some(Led::Play(deck)),
        Control::Pad(deck, mode, pad) => Some(Led::Pad { deck, mode, pad }),
        Control::Vinyl => Some(Led::Vinyl),
        _ => None,
    }
}

/// Splits a decoded event into the control it comes from and its value.
#[must_use]
pub fn identify(event: ControllerEvent) -> Option<(ControlId, Input)> {
    let plain = |control| ControlId {
        control,
        shift: false,
    };
    Some(match event {
        ControllerEvent::Button {
            deck,
            button,
            shift,
            pressed,
        } => (
            ControlId {
                control: Control::Button(deck, button),
                shift,
            },
            Input::Press(pressed),
        ),
        ControllerEvent::Pad {
            deck,
            mode,
            pad,
            shift,
            pressed,
            ..
        } => (
            ControlId {
                control: Control::Pad(deck, mode, pad),
                shift,
            },
            Input::Press(pressed),
        ),
        ControllerEvent::Vinyl { pressed } => (plain(Control::Vinyl), Input::Press(pressed)),
        ControllerEvent::Mixer { control, value } => (
            plain(Control::Mixer(control)),
            Input::Absolute(normalize(value)),
        ),
        ControllerEvent::Knob { deck, knob, value } => (
            plain(Control::Knob(deck, knob)),
            Input::Absolute(normalize(value)),
        ),
        ControllerEvent::Tempo { deck, value } => (
            plain(Control::Tempo(deck)),
            Input::Absolute(normalize(value)),
        ),
        ControllerEvent::Jog {
            deck,
            surface,
            shift,
            delta,
        } => (
            ControlId {
                control: Control::Jog(deck, surface),
                shift,
            },
            Input::Relative(i32::from(delta)),
        ),
        ControllerEvent::Shift { .. }
        | ControllerEvent::FilterBassToggle { .. }
        | ControllerEvent::PadModeSelect { .. }
        | ControllerEvent::Unknown { .. } => return None,
    })
}

/// Parses `a.play`, `b.pad.loop.4`, `mixer.master`, `a.jog.top`, `vinyl`…
#[must_use]
pub fn parse_control(name: &str) -> Option<Control> {
    let lower = name.trim().to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('.').collect();
    let deck = |s: &str| match s {
        "a" => Some(Deck::A),
        "b" => Some(Deck::B),
        _ => None,
    };
    Some(match parts.as_slice() {
        ["vinyl"] => Control::Vinyl,
        ["mixer", "crossfader"] => Control::Mixer(MixerControl::Crossfader),
        ["mixer", "master"] => Control::Mixer(MixerControl::Master),
        ["mixer", "headphone"] => Control::Mixer(MixerControl::Headphone),
        [d, "pad", mode, n] => {
            let mode = match *mode {
                "hotcue" => PadMode::HotCue,
                "sampler" => PadMode::Sampler,
                "fx" => PadMode::Fx,
                "loop" => PadMode::Loop,
                _ => return None,
            };
            let n: u8 = n.parse().ok().filter(|n| (1..=4).contains(n))?;
            Control::Pad(deck(d)?, mode, n - 1)
        }
        [d, "jog", "edge"] => Control::Jog(deck(d)?, JogSurface::Edge),
        [d, "jog", "top"] => Control::Jog(deck(d)?, JogSurface::Top),
        [d, what] => {
            let deck = deck(d)?;
            match *what {
                "play" => Control::Button(deck, Button::Play),
                "cue" => Control::Button(deck, Button::Cue),
                "sync" => Control::Button(deck, Button::Sync),
                "pfl" => Control::Button(deck, Button::Pfl),
                "volume" => Control::Knob(deck, DeckKnob::Volume),
                "filter" => Control::Knob(deck, DeckKnob::Filter),
                "bass" => Control::Knob(deck, DeckKnob::Bass),
                "tempo" => Control::Tempo(deck),
                _ => return None,
            }
        }
        _ => return None,
    })
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = |deck: &Deck| match deck {
            Deck::A => "a",
            Deck::B => "b",
        };
        match self {
            Self::Button(deck, b) => write!(f, "{}.{}", d(deck), format!("{b:?}").to_lowercase()),
            Self::Pad(deck, mode, pad) => write!(
                f,
                "{}.pad.{}.{}",
                d(deck),
                format!("{mode:?}").to_lowercase(),
                pad + 1
            ),
            Self::Mixer(m) => write!(f, "mixer.{}", format!("{m:?}").to_lowercase()),
            Self::Knob(deck, k) => write!(f, "{}.{}", d(deck), format!("{k:?}").to_lowercase()),
            Self::Tempo(deck) => write!(f, "{}.tempo", d(deck)),
            Self::Jog(deck, s) => write!(f, "{}.jog.{}", d(deck), format!("{s:?}").to_lowercase()),
            Self::Vinyl => write!(f, "vinyl"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000;

    fn mapping() -> Mapping {
        Mapping::from_toml_str(DEFAULT_MAPPING).expect("default mapping is valid")
    }

    fn tempo_event(value: u16) -> ControllerEvent {
        ControllerEvent::Tempo {
            deck: Deck::A,
            value,
        }
    }

    #[test]
    fn default_mapping_loads() {
        assert!(mapping().len() >= 8);
    }

    #[test]
    fn bass_lines_mute_tonic_and_cutoff() {
        let mut m = mapping();
        let hot_cue = |shift| ControllerEvent::Pad {
            deck: Deck::A,
            mode: PadMode::HotCue,
            pad: 2,
            shift,
            pressed: true,
            velocity: 127,
        };
        let button = |deck, shift| ControllerEvent::Button {
            deck,
            button: Button::Cue,
            shift,
            pressed: true,
        };
        assert_eq!(m.handle(1, hot_cue(false)), Some(Action::QueuePattern(2)));
        assert_eq!(m.handle(2, hot_cue(true)), Some(Action::QueueBassLine(2)));
        assert_eq!(
            m.handle(3, button(Deck::A, false)),
            Some(Action::ToggleBassMute)
        );
        assert_eq!(
            m.handle(4, button(Deck::B, false)),
            Some(Action::NudgeTonic(-1))
        );
        assert_eq!(
            m.handle(5, button(Deck::B, true)),
            Some(Action::NudgeTonic(1))
        );
        let knob = ControllerEvent::Knob {
            deck: Deck::A,
            knob: DeckKnob::Bass,
            value: MAX_14BIT,
        };
        assert_eq!(m.handle(6, knob), Some(Action::SetBassCutoff(1.0)));
        assert_eq!(
            m.bound(ActionName::BassMute)
                .filter_map(led_of)
                .collect::<Vec<_>>(),
            vec![Led::Cue(Deck::A)]
        );
    }

    #[test]
    fn sampler_pads_fill_and_play_drums_with_shift() {
        let mut m = mapping();
        let pad = |pressed, shift| ControllerEvent::Pad {
            deck: Deck::A,
            mode: PadMode::Sampler,
            pad: 0,
            shift,
            pressed,
            velocity: if pressed { 127 } else { 0 },
        };
        assert_eq!(m.handle(40, pad(true, false)), Some(Action::Fill(0)));
        assert_eq!(m.handle(41, pad(false, false)), None);
        assert_eq!(
            m.handle(42, pad(true, true)),
            Some(Action::Drum {
                voice: DrumVoice::Kick,
                velocity: 1.0,
                ts_us: 42
            })
        );
        assert_eq!(m.handle(43, pad(false, true)), None);
        let leds: Vec<_> = m.bound(ActionName::Fill).filter_map(led_of).collect();
        assert!(leds.contains(&Led::Pad {
            deck: Deck::A,
            mode: PadMode::Sampler,
            pad: 0
        }));
        assert_eq!(
            m.bound(ActionName::TransportToggle)
                .filter_map(led_of)
                .count(),
            2,
            "Play A and B, not the Shift layer"
        );
    }

    #[test]
    fn hot_cue_pads_queue_patterns_and_jog_edge_swings() {
        let mut m = mapping();
        let pad = ControllerEvent::Pad {
            deck: Deck::A,
            mode: PadMode::HotCue,
            pad: 2,
            shift: false,
            pressed: true,
            velocity: 127,
        };
        assert_eq!(m.handle(0, pad), Some(Action::QueuePattern(2)));
        let jog = ControllerEvent::Jog {
            deck: Deck::A,
            surface: JogSurface::Edge,
            shift: false,
            delta: 5,
        };
        match m.handle(0, jog) {
            Some(Action::NudgeSwing(d)) => assert!((d - 0.01).abs() < 1.0e-6),
            other => panic!("{other:?}"),
        }
        let pads: Vec<_> = m.indexed(ActionName::Pattern).collect();
        assert_eq!(pads.len(), 4);
        assert!(pads.contains(&(Control::Pad(Deck::A, PadMode::HotCue, 3), 3)));
        assert!(
            Mapping::from_toml_str(
                "[[bind]]\ncontrol = \"a.pad.fx.1\"\naction = \"pattern\"\npattern = 9"
            )
            .is_err()
        );
    }

    #[test]
    fn fx_pads_mute_groups_and_loop_pads_repeat_while_held() {
        let mut m = mapping();
        let pad = |mode, pad, pressed| ControllerEvent::Pad {
            deck: Deck::A,
            mode,
            pad,
            shift: false,
            pressed,
            velocity: 127,
        };
        let snare_clap = voice_bit(DrumVoice::Snare) | voice_bit(DrumVoice::Clap);
        assert_eq!(
            m.handle(0, pad(PadMode::Fx, 1, true)),
            Some(Action::ToggleMute(snare_clap))
        );
        assert_eq!(m.handle(0, pad(PadMode::Fx, 1, false)), None);
        assert_eq!(
            m.handle(0, pad(PadMode::Loop, 2, true)),
            Some(Action::BeatRepeat {
                division: Division::Sixteenth,
                held: true
            })
        );
        assert_eq!(
            m.handle(0, pad(PadMode::Loop, 2, false)),
            Some(Action::BeatRepeat {
                division: Division::Sixteenth,
                held: false
            })
        );
        let mutes: Vec<_> = m.indexed(ActionName::Mute).collect();
        assert!(mutes.contains(&(Control::Pad(Deck::A, PadMode::Fx, 1), snare_clap)));
        assert!(
            Mapping::from_toml_str(
                "[[bind]]\ncontrol = \"a.pad.loop.1\"\naction = \"beat_repeat\"\ndivision = 3"
            )
            .is_err()
        );
    }

    #[test]
    fn tempo_fader_sets_density_directly() {
        let mut m = mapping();
        assert_eq!(m.handle(0, tempo_event(0)), Some(Action::SetDensity(0.0)));
        assert_eq!(
            m.handle(0, tempo_event(MAX_14BIT)),
            Some(Action::SetDensity(1.0))
        );
    }

    #[test]
    fn unknown_drum_voice_is_named() {
        let result = Mapping::from_toml_str(
            "[[bind]]\ncontrol = \"a.pad.fx.1\"\naction = \"drum\"\nvoice = \"cowbell\"",
        );
        assert!(
            result
                .err()
                .is_some_and(|e| e.to_string().contains("cowbell"))
        );
    }

    #[test]
    fn control_names_round_trip() {
        for name in [
            "a.play",
            "b.sync",
            "a.pad.hotcue.1",
            "b.pad.loop.4",
            "mixer.crossfader",
            "a.tempo",
            "b.jog.top",
            "a.filter",
            "vinyl",
        ] {
            let control = parse_control(name).expect(name);
            assert_eq!(control.to_string(), name);
        }
        assert_eq!(parse_control("a.pad.fx.5"), None);
        assert_eq!(parse_control("c.play"), None);
    }

    #[test]
    fn errors_name_the_culprit() {
        let err =
            Mapping::from_toml_str("[[bind]]\ncontrol = \"a.pley\"\naction = \"transport_toggle\"")
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
        assert!(err.contains("a.pley"), "{err}");
        let err = Mapping::from_toml_str(
            "[[bind]]\ncontrol = \"a.tempo\"\naction = \"transport_toggle\"",
        )
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default();
        assert!(err.contains("ne convient pas"), "{err}");
    }

    #[test]
    fn play_toggles_on_press_only() {
        let mut m = mapping();
        let play = |pressed| ControllerEvent::Button {
            deck: Deck::A,
            button: Button::Play,
            shift: false,
            pressed,
        };
        assert_eq!(m.handle(0, play(true)), Some(Action::TransportToggle));
        assert_eq!(m.handle(0, play(false)), None);
    }

    #[test]
    fn tempo_fader_waits_for_pickup() {
        let mut m = Mapping::from_toml_str(
            "[[bind]]\ncontrol = \"a.tempo\"\naction = \"tempo\"\nrange = [60.0, 200.0]\npickup = true",
        )
        .expect("valid");
        // Software tempo is 120 BPM = 0.43 of the 60..200 range.
        m.software_changed(Target::Tempo, 120.0);
        // Fader at the bottom: ignored.
        assert_eq!(m.handle(0, tempo_event(0)), None);
        assert!(m.pending_pickup(Target::Tempo).is_some());
        // Moving up past 0.43: caught, applies.
        let Some(Action::SetTempo(bpm)) = m.handle(0, tempo_event(8_192)) else {
            panic!("fader should have taken over");
        };
        assert!((bpm - 130.0).abs() < 0.1, "{bpm}");
        assert_eq!(m.pending_pickup(Target::Tempo), None);
        // Caught: follows from now on.
        assert!(matches!(
            m.handle(0, tempo_event(0)),
            Some(Action::SetTempo(_))
        ));
        // Fader resting at the top (200 BPM): a software change to 200 needs no pickup.
        m.handle(0, tempo_event(MAX_14BIT));
        m.software_changed(Target::Tempo, 200.0);
        assert_eq!(m.pending_pickup(Target::Tempo), None);
    }

    #[test]
    fn jog_top_needs_a_confirmed_touch() {
        let mut m = mapping();
        let touch = |pressed| ControllerEvent::Button {
            deck: Deck::A,
            button: Button::JogTouch,
            shift: false,
            pressed,
        };
        let top = ControllerEvent::Jog {
            deck: Deck::A,
            surface: JogSurface::Top,
            shift: false,
            delta: 1,
        };
        // Flicker while spinning the edge: ticks are held, then go to the edge (swing) on release.
        m.handle(0, touch(true));
        assert_eq!(m.handle(10 * MS, top), None);
        assert_eq!(m.handle(20 * MS, top), None);
        match m.handle(30 * MS, touch(false)) {
            Some(Action::NudgeSwing(d)) => assert!((d - 0.004).abs() < 1.0e-6),
            other => panic!("{other:?}"),
        }
        // Real touch: early ticks are held, then join the top (tempo) once confirmed.
        m.handle(200 * MS, touch(true));
        assert_eq!(m.handle(210 * MS, top), None);
        assert_eq!(m.handle(300 * MS, top), Some(Action::NudgeTempo(0.5)));
        assert_eq!(m.handle(310 * MS, top), Some(Action::NudgeTempo(0.25)));
        assert_eq!(m.handle(400 * MS, touch(false)), None);
    }

    #[test]
    fn tap_tempo_averages_taps() {
        let mut m = mapping();
        let sync = ControllerEvent::Button {
            deck: Deck::A,
            button: Button::Sync,
            shift: false,
            pressed: true,
        };
        assert_eq!(m.handle(0, sync), None);
        assert_eq!(m.handle(500 * MS, sync), Some(Action::SetTempo(120.0)));
        let Some(Action::SetTempo(bpm)) = m.handle(1_000 * MS, sync) else {
            panic!("third tap");
        };
        assert!((bpm - 120.0).abs() < 1e-9);
        // A pause longer than 2 s restarts the sequence.
        assert_eq!(m.handle(5_000 * MS, sync), None);
    }
}
