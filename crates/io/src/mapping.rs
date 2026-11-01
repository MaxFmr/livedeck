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
}

/// Result of a mapped gesture, applied by the control thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    TransportToggle,
    TransportRestart,
    MetronomeToggle,
    /// Test sound (latency measurement); `ts_us` is the gesture's CoreMIDI host time.
    TestHit {
        ts_us: u64,
    },
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
    TestHit,
    Tempo,
    TempoNudge,
    MasterGain,
    MetronomeLevel,
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
    /// Amount per jog tick for `tempo_nudge` (BPM).
    step: Option<f64>,
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
            let binding = Binding {
                action: b.action,
                pickup: b.pickup,
                range: b.range.unwrap_or([60.0, 200.0]),
                step: b.step.unwrap_or(0.1),
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
        if let ControllerEvent::Button {
            deck,
            button: Button::JogTouch,
            pressed,
            ..
        } = event
            && let Some(slot) = self.touch_down_us.get_mut(deck_index(deck))
        {
            *slot = pressed.then_some(ts_us);
        }
        let (mut id, input) = identify(event)?;
        if let Control::Jog(deck, JogSurface::Top) = id.control {
            let confirmed = self
                .touch_down_us
                .get(deck_index(deck))
                .copied()
                .flatten()
                .is_some_and(|down| ts_us.saturating_sub(down) >= TOUCH_CONFIRM_US);
            if !confirmed {
                id.control = Control::Jog(deck, JogSurface::Edge);
            }
        }
        let binding = *self.bindings.get(&id)?;
        match (binding.action, input) {
            (ActionName::TransportToggle, Input::Press(true)) => Some(Action::TransportToggle),
            (ActionName::TransportRestart, Input::Press(true)) => Some(Action::TransportRestart),
            (ActionName::MetronomeToggle, Input::Press(true)) => Some(Action::MetronomeToggle),
            (ActionName::TestHit, Input::Press(true)) => Some(Action::TestHit { ts_us }),
            (ActionName::TapTempo, Input::Press(true)) => self.tap(ts_us),
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
            (ActionName::MasterGain | ActionName::MetronomeLevel, Input::Absolute(v)) => {
                let target = target_of(binding.action)?;
                let soft = self.soft.get(&target).copied();
                self.take_over(id, binding.pickup, soft, v)
                    .then_some(Action::SetLevel(target, v))
            }
            _ => None,
        }
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
        Target::MasterGain | Target::MetronomeLevel => value.clamp(0.0, 1.0) as f32,
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
        | ActionName::TestHit => matches!(kind, Kind::Press),
        ActionName::Tempo | ActionName::MasterGain | ActionName::MetronomeLevel => {
            matches!(kind, Kind::Absolute)
        }
        ActionName::TempoNudge => matches!(kind, Kind::Relative),
    }
}

fn normalize(value: u16) -> f32 {
    f32::from(value) / f32::from(MAX_14BIT)
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
    fn pads_play_the_test_hit_and_light_their_led() {
        let mut m = mapping();
        let pad = |pressed| ControllerEvent::Pad {
            deck: Deck::A,
            mode: PadMode::HotCue,
            pad: 0,
            shift: false,
            pressed,
            velocity: if pressed { 127 } else { 0 },
        };
        assert_eq!(m.handle(42, pad(true)), Some(Action::TestHit { ts_us: 42 }));
        assert_eq!(m.handle(43, pad(false)), None);
        let leds: Vec<_> = m.bound(ActionName::TestHit).filter_map(led_of).collect();
        assert!(leds.contains(&Led::Pad {
            deck: Deck::A,
            mode: PadMode::HotCue,
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
        let mut m = mapping();
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
        // Flicker while spinning the edge: counted as edge (fine nudge).
        m.handle(0, touch(true));
        assert_eq!(m.handle(10 * MS, top), Some(Action::NudgeTempo(0.1)));
        // Real touch held 100 ms: top (coarse nudge).
        assert_eq!(m.handle(100 * MS, top), Some(Action::NudgeTempo(0.25)));
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
        assert!(
            m.handle(0, tempo_event(0)).is_none(),
            "after a tap, the tempo fader waits for pickup"
        );
        // A pause longer than 2 s restarts the sequence.
        assert_eq!(m.handle(5_000 * MS, sync), None);
    }
}
