//! Mono bass of the rhythm deck: a saw plus a square one octave below through a resonant low-pass
//! with its own envelope, glide between tied notes, accents. Its lines are written in scale
//! degrees: changing the key moves everything without a wrong note.
//!
//! Recipe: docs/rust/dsp-cookbook.md § Basse.
//!
//! Runs on: audio thread. Note changes compute one `exp2`; the filter coefficients (`tan`) are
//! refreshed every `CONTROL` samples.

use crate::dsp::{Decay, SILENCE, Svf, soft_clip};
use crate::sequencer::{PATTERNS, STEPS};
use crate::smooth::Smoothed;

/// Samples between two updates of the filter coefficients.
const CONTROL: usize = 16;
/// Lowest and highest base cutoff of the Bass knob, in Hz.
const CUTOFF_MIN: f32 = 60.0;
const CUTOFF_MAX: f32 = 4_000.0;
/// Time constants of the amplitude (attack, release), in seconds.
const ATTACK: f32 = 0.002;
const RELEASE: f32 = 0.03;
/// Share of the note length the gate stays open (the rest is the release), unless the next note
/// glides from it.
const GATE: f64 = 0.8;

/// Character of the bass, set by the style presets (the cutoff stays on the Bass knob).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassParams {
    /// Levels of the saw and of the square one octave below.
    pub saw: f32,
    pub sub: f32,
    pub resonance: f32,
    /// Filter envelope depth in octaves, normal and accented.
    pub env_octaves: f32,
    pub accent_octaves: f32,
    /// Filter envelope decay to -60 dB, seconds (accents: 60 % of it).
    pub decay: f32,
    /// Glide time constant, seconds.
    pub glide: f32,
    /// Gain into the soft clipper.
    pub drive: f32,
}

impl Default for BassParams {
    fn default() -> Self {
        Self {
            saw: 0.5,
            sub: 0.45,
            resonance: 1.1,
            env_octaves: 2.5,
            accent_octaves: 4.0,
            decay: 0.2,
            glide: 0.045,
            drive: 1.4,
        }
    }
}

/// Scales, as semitones from the tonic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scale {
    #[default]
    Minor,
    Major,
    Dorian,
    Phrygian,
    Mixolydian,
    MinorPentatonic,
}

impl Scale {
    /// Order of the Shift + jog B scroll.
    pub const ALL: [Self; 6] = [
        Self::Minor,
        Self::Major,
        Self::Dorian,
        Self::Phrygian,
        Self::Mixolydian,
        Self::MinorPentatonic,
    ];

    #[must_use]
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0) as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }

    /// Name for display (French).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Minor => "mineur",
            Self::Major => "majeur",
            Self::Dorian => "dorien",
            Self::Phrygian => "phrygien",
            Self::Mixolydian => "mixolydien",
            Self::MinorPentatonic => "penta mineure",
        }
    }

    #[must_use]
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
        }
    }
}

/// Tonic (0 = C … 11 = B) and scale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Key {
    pub tonic: u8,
    pub scale: Scale,
}

impl Key {
    /// MIDI note of `degree` (1-based, may exceed the scale length) shifted by `octave`. The
    /// tonic sits between E2 and D#3, so the sub octave stays above 40 Hz.
    #[must_use]
    pub fn note(self, degree: u8, octave: i8) -> f32 {
        self.note_at(i32::from(degree.max(1)) - 1, octave)
    }

    /// MIDI note of scale step `index` (0 = tonic, negative = below it) shifted by `octave`.
    #[must_use]
    pub fn note_at(self, index: i32, octave: i8) -> f32 {
        let intervals = self.scale.intervals();
        let len = i32::try_from(intervals.len()).unwrap_or(1).max(1);
        let slot = usize::try_from(index.rem_euclid(len)).unwrap_or(0);
        let interval = intervals.get(slot).copied().unwrap_or(0);
        let root = 40 + (i32::from(self.tonic % 12) + 8) % 12;
        let semis = root + i32::from(interval) + 12 * (index.div_euclid(len) + i32::from(octave));
        semis as f32
    }
}

/// Note names for display (sharps).
#[must_use]
pub fn tonic_name(tonic: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES.get(usize::from(tonic % 12)).copied().unwrap_or("?")
}

/// A note starting on a step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BassNote {
    /// Scale degree, 1-based.
    pub degree: u8,
    pub octave: i8,
    /// Length in steps (16ths), ≥ 1.
    pub len: u8,
    pub accent: bool,
    /// Slides from the previous note, which is held until this one (legato).
    pub glide: bool,
}

/// One bar of bass: a note or nothing on each step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BassPattern {
    pub steps: [Option<BassNote>; STEPS],
}

impl BassPattern {
    /// Parses 16 space-separated tokens: `.` rest, `-` holds the previous note one more step,
    /// else a degree `1`–`9` with optional marks: `'` octave up, `,` octave down (repeatable),
    /// `!` accent, and a leading `~` to glide into it.
    /// Example: `"~1! - . 1 . . 5, . 7, - . . 1 . 3 ."`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut pattern = Self::default();
        let mut last: Option<usize> = None;
        for (i, token) in text.split_whitespace().take(STEPS).enumerate() {
            if token == "-" {
                if let Some(note) = last
                    .and_then(|l| pattern.steps.get_mut(l))
                    .and_then(|s| s.as_mut())
                {
                    note.len = note.len.saturating_add(1);
                }
                continue;
            }
            let glide = token.starts_with('~');
            let mut chars = token.trim_start_matches('~').chars();
            let Some(degree) = chars.next().and_then(|c| c.to_digit(10)).filter(|&d| d > 0) else {
                last = None;
                continue;
            };
            let mut note = BassNote {
                degree: degree as u8,
                octave: 0,
                len: 1,
                accent: false,
                glide,
            };
            for mark in chars {
                match mark {
                    '\'' => note.octave += 1,
                    ',' => note.octave -= 1,
                    '!' => note.accent = true,
                    _ => {}
                }
            }
            if let Some(slot) = pattern.steps.get_mut(i) {
                *slot = Some(note);
                last = Some(i);
            }
        }
        pattern
    }

    /// Whether the note sounding at `step` is followed, right at its end, by a note that glides
    /// from it (then its gate stays open until that note).
    #[must_use]
    pub fn held_into_glide(&self, step: usize, len: u8) -> bool {
        let next = (step + usize::from(len)) % STEPS;
        self.steps
            .get(next)
            .copied()
            .flatten()
            .is_some_and(|n| n.glide)
    }
}

/// Bass lines paired with the drum patterns A–D (minor): A dub, B groove, C house offbeats,
/// D busy break. Style presets (014) will replace them.
#[must_use]
pub fn default_bass_patterns() -> [BassPattern; PATTERNS] {
    [
        BassPattern::parse("1! - - - . . 1 . 5, - - . 7, - . ."),
        BassPattern::parse("~1! . 1 . . 3 . 1 . . 5 4 . 3 . 7,"),
        BassPattern::parse(". . 1! . . . 1 . . . 1! . . 7, ~1 ."),
        BassPattern::parse("1! . 1' 1 . 1 ~3 . 1 . 5, . 7, ~1 . 4,"),
    ]
}

fn midi_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

fn one_pole(sample_rate: f32, seconds: f32) -> f32 {
    1.0 - (-1.0 / (seconds.max(1.0e-4) * sample_rate)).exp()
}

/// PolyBLEP residual: smooths the step of a naive saw or square at phase `t` (0..1).
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

/// The bass synthesizer: one voice, retriggered or slid by each note.
#[derive(Debug, Clone)]
pub struct BassVoice {
    sample_rate: f32,
    /// Current and target pitch (MIDI note, fractional while gliding).
    pitch: f32,
    target: f32,
    params: BassParams,
    glide_coef: f32,
    /// Phase increment of the saw (cycles per sample).
    inc: f32,
    saw: f32,
    sub: f32,
    filter: Svf,
    filter_env: Decay,
    env_octaves: f32,
    /// Bass knob, 0..=1, smoothed.
    cutoff: Smoothed,
    amp: f32,
    amp_target: f32,
    attack_coef: f32,
    release_coef: f32,
    /// Samples left before the gate closes.
    gate: u64,
    control: usize,
}

impl BassVoice {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            sample_rate: sr,
            pitch: 45.0,
            target: 45.0,
            params: BassParams::default(),
            glide_coef: one_pole(sr, BassParams::default().glide),
            inc: midi_to_hz(45.0) / sr,
            saw: 0.0,
            sub: 0.0,
            filter: Svf::default(),
            filter_env: Decay::default(),
            env_octaves: BassParams::default().env_octaves,
            cutoff: Smoothed::new(0.45, sample_rate, 0.02),
            amp: 0.0,
            amp_target: 0.0,
            attack_coef: one_pole(sr, ATTACK),
            release_coef: one_pole(sr, RELEASE),
            gate: 0,
            control: 0,
        }
    }

    /// Bass knob, 0..=1 (base cutoff, exponential from 60 Hz to 4 kHz).
    pub fn set_cutoff(&mut self, value: f32) {
        if value.is_finite() {
            self.cutoff.set(value.clamp(0.0, 1.0));
        }
    }

    /// Sound of the next notes (a style change).
    pub fn set_params(&mut self, params: BassParams) {
        self.params = params;
        self.glide_coef = one_pole(self.sample_rate, params.glide);
    }

    /// Plays `note` (MIDI) with its gate open for `gate` samples. `glide` slides from the note
    /// still held instead of retriggering it.
    pub fn note_on(&mut self, note: f32, accent: bool, glide: bool, gate: u64) {
        let legato = glide && self.gate > 0;
        self.target = note;
        if !legato {
            self.pitch = note;
            self.inc = midi_to_hz(note) / self.sample_rate;
            let p = self.params;
            let decay = if accent { p.decay * 0.6 } else { p.decay };
            self.filter_env.trigger(1.0, self.sample_rate, decay);
            self.env_octaves = if accent {
                p.accent_octaves
            } else {
                p.env_octaves
            };
        }
        self.amp_target = if accent { 1.0 } else { 0.75 };
        self.gate = gate.max(1);
    }

    /// Closes the gate now (mute, transport stop).
    pub fn note_off(&mut self) {
        self.gate = 0;
        self.amp_target = 0.0;
    }

    /// Gate open (a note is held).
    #[must_use]
    pub fn gated(&self) -> bool {
        self.gate > 0
    }

    fn update_filter(&mut self) {
        let knob = self.cutoff.skip(CONTROL);
        let base = CUTOFF_MIN * (CUTOFF_MAX / CUTOFF_MIN).powf(knob);
        let env = self.filter_env.level * self.env_octaves;
        self.filter
            .set(self.sample_rate, base * env.exp2(), self.params.resonance);
    }

    /// Adds the voice into `out`.
    pub fn render_add(&mut self, out: &mut [f32]) {
        if self.amp < SILENCE && self.amp_target == 0.0 {
            self.amp = 0.0;
            self.pitch = self.target;
            return;
        }
        for sample in out.iter_mut() {
            if self.control == 0 {
                self.update_filter();
                self.control = CONTROL;
            }
            self.control -= 1;
            if self.pitch != self.target {
                // Snap within half a cent, or when the step no longer moves an f32 this large.
                let next = self.pitch + (self.target - self.pitch) * self.glide_coef;
                self.pitch = if next == self.pitch || (self.target - next).abs() < 5.0e-3 {
                    self.target
                } else {
                    next
                };
                self.inc = midi_to_hz(self.pitch) / self.sample_rate;
            }
            if self.gate > 0 {
                self.gate -= 1;
                if self.gate == 0 {
                    self.amp_target = 0.0;
                }
            }
            let coef = if self.amp_target > self.amp {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.amp += (self.amp_target - self.amp) * coef;
            self.filter_env.tick();

            let dt = self.inc;
            let saw = 2.0 * self.saw - 1.0 - poly_blep(self.saw, dt);
            let sub_dt = dt * 0.5;
            let mut sub = if self.sub < 0.5 { 1.0 } else { -1.0 };
            sub += poly_blep(self.sub, sub_dt);
            sub -= poly_blep((self.sub + 0.5) % 1.0, sub_dt);
            self.saw += dt;
            if self.saw >= 1.0 {
                self.saw -= 1.0;
            }
            self.sub += sub_dt;
            if self.sub >= 1.0 {
                self.sub -= 1.0;
            }

            let p = &self.params;
            let filtered = self.filter.process(p.saw * saw + p.sub * sub).lp;
            *sample += soft_clip(p.drive * filtered * self.amp) * 0.6;
        }
        if self.amp < SILENCE && self.amp_target == 0.0 {
            self.amp = 0.0;
            self.filter.reset();
        }
    }
}

/// What the bass plays: four lines, the one playing and the one waiting for the bar, the key
/// (a tonic change also waits for the bar) and the mute.
#[derive(Debug, Clone)]
pub struct BassLine {
    patterns: [BassPattern; PATTERNS],
    current: u8,
    pending: Option<u8>,
    key: Key,
    pending_tonic: Option<u8>,
    pending_scale: Option<Scale>,
    muted: bool,
}

impl Default for BassLine {
    fn default() -> Self {
        Self::new()
    }
}

impl BassLine {
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: default_bass_patterns(),
            current: 0,
            pending: None,
            // A minor.
            key: Key {
                tonic: 9,
                scale: Scale::Minor,
            },
            pending_tonic: None,
            pending_scale: None,
            muted: false,
        }
    }

    /// Queues line `index` (0-based) for the next bar, or switches now if stopped.
    pub fn queue(&mut self, index: u8, playing: bool) {
        if usize::from(index) >= PATTERNS {
            return;
        }
        if playing && index != self.current {
            self.pending = Some(index);
        } else {
            self.current = index;
            self.pending = None;
        }
    }

    /// Tonic (0–11) for the next bar, or now if stopped.
    pub fn queue_tonic(&mut self, tonic: u8, playing: bool) {
        let tonic = tonic % 12;
        if playing && tonic != self.key.tonic {
            self.pending_tonic = Some(tonic);
        } else {
            self.key.tonic = tonic;
            self.pending_tonic = None;
        }
    }

    /// Scale for the next bar, or now if stopped.
    pub fn queue_scale(&mut self, scale: Scale, playing: bool) {
        if playing && scale != self.key.scale {
            self.pending_scale = Some(scale);
        } else {
            self.key.scale = scale;
            self.pending_scale = None;
        }
    }

    #[must_use]
    pub fn pending_scale(&self) -> Option<Scale> {
        self.pending_scale
    }

    /// Replaces the four lines (a style change); the line index playing is kept.
    pub fn set_patterns(&mut self, patterns: [BassPattern; PATTERNS]) {
        self.patterns = patterns;
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Applies what waits for the bar (transport start).
    pub fn reset(&mut self) {
        if let Some(p) = self.pending.take() {
            self.current = p;
        }
        if let Some(t) = self.pending_tonic.take() {
            self.key.tonic = t;
        }
        if let Some(s) = self.pending_scale.take() {
            self.key.scale = s;
        }
    }

    #[must_use]
    pub fn current(&self) -> u8 {
        self.current
    }

    #[must_use]
    pub fn pending(&self) -> Option<u8> {
        self.pending
    }

    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    #[must_use]
    pub fn pending_tonic(&self) -> Option<u8> {
        self.pending_tonic
    }

    #[must_use]
    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Plays `step` (0..16) of the bar. `gate_of(len, held)` gives the gate in samples of a note
    /// of `len` steps (`held`: kept open until the next note, which glides from it).
    pub fn fire(
        &mut self,
        step: usize,
        voice: &mut BassVoice,
        enabled: bool,
        gate_of: impl Fn(u8, bool) -> u64,
    ) {
        if step == 0 {
            self.reset();
        }
        if self.muted || !enabled {
            if voice.gated() {
                voice.note_off();
            }
            return;
        }
        let Some(pattern) = self.patterns.get(usize::from(self.current)) else {
            return;
        };
        let Some(note) = pattern.steps.get(step).copied().flatten() else {
            return;
        };
        let held = pattern.held_into_glide(step, note.len);
        voice.note_on(
            self.key.note(note.degree, note.octave),
            note.accent,
            note.glide,
            gate_of(note.len, held),
        );
    }
}

/// Gate of a note lasting `samples`: most of it, or all of it plus a little overlap when the next
/// note glides from it.
#[must_use]
pub fn gate_samples(samples: u64, held: bool) -> u64 {
    if held {
        samples + 64
    } else {
        (samples as f64 * GATE) as u64
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn degrees_follow_the_key() {
        let a_minor = Key {
            tonic: 9,
            scale: Scale::Minor,
        };
        assert_eq!(a_minor.note(1, 0), 45.0); // A2
        assert_eq!(a_minor.note(3, 0), 48.0); // C3
        assert_eq!(a_minor.note(8, 0), 57.0); // next octave
        assert_eq!(a_minor.note(5, -1), 40.0); // E2
        let e = Key {
            tonic: 4,
            ..a_minor
        };
        assert_eq!(e.note(1, 0), 40.0); // E2, the lowest tonic
        let d_sharp = Key {
            tonic: 3,
            ..a_minor
        };
        assert_eq!(d_sharp.note(1, 0), 51.0); // D#3, the highest
    }

    #[test]
    fn parses_degrees_ties_marks_and_glides() {
        let p = BassPattern::parse("1! - - . 5, ~7' . x 3 - . . . . . . .");
        assert_eq!(
            p.steps[0],
            Some(BassNote {
                degree: 1,
                octave: 0,
                len: 3,
                accent: true,
                glide: false
            })
        );
        assert_eq!(p.steps[4].unwrap().octave, -1);
        let seven = p.steps[5].unwrap();
        assert!(seven.glide && seven.octave == 1 && seven.degree == 7);
        assert_eq!(p.steps[7], None, "unknown token = rest");
        assert_eq!(p.steps[8].unwrap().len, 2);
        assert!(p.held_into_glide(4, 1));
        assert!(!p.held_into_glide(0, 3));
    }

    #[test]
    fn default_lines_have_sixteen_steps_worth() {
        for p in default_bass_patterns() {
            assert!(p.steps.iter().any(Option::is_some));
        }
        // Each default line is written with exactly 16 tokens.
        for text in [
            "1! - - - . . 1 . 5, - - . 7, - . .",
            "~1! . 1 . . 3 . 1 . . 5 4 . 3 . 7,",
            ". . 1! . . . 1 . . . 1! . . 7, ~1 .",
            "1! . 1' 1 . 1 ~3 . 1 . 5, . 7, ~1 . 4,",
        ] {
            assert_eq!(text.split_whitespace().count(), STEPS, "{text}");
        }
    }

    fn render(voice: &mut BassVoice, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames];
        voice.render_add(&mut out);
        out
    }

    /// Dominant period of `x`, by zero crossings going up.
    fn period(x: &[f32]) -> f32 {
        let ups: Vec<usize> = x
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] <= 0.0 && w[1] > 0.0)
            .map(|(i, _)| i)
            .collect();
        (ups[ups.len() - 1] - ups[0]) as f32 / (ups.len() - 1) as f32
    }

    #[test]
    fn plays_the_note_then_releases_to_silence() {
        let mut v = BassVoice::new(SR);
        v.set_cutoff(0.2);
        v.note_on(45.0, false, false, 4_800);
        let out = render(&mut v, 4_800);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() < 1.0));
        assert!(out[1_000..].iter().any(|s| s.abs() > 0.1));
        // A2 = 110 Hz but the square sub (55 Hz) dominates once the filter closes.
        let p = period(&out[2_400..]);
        assert!((p - 48_000.0 / 55.0).abs() < 20.0, "{p}");
        let tail = render(&mut v, 19_200);
        assert!(tail[14_400..].iter().all(|s| s.abs() < 1.0e-3));
        assert!(!v.gated());
        assert!(
            render(&mut v, 64).iter().all(|&s| s == 0.0),
            "idle = silence"
        );
    }

    #[test]
    fn glide_slides_only_from_a_held_note() {
        let mut v = BassVoice::new(SR);
        v.note_on(45.0, false, false, 2_000);
        render(&mut v, 1_000);
        v.note_on(57.0, false, true, 2_000);
        assert!(v.pitch < 46.0, "held: the pitch slides");
        render(&mut v, 48_000);
        assert_eq!(v.pitch, 57.0, "and gets there");
        // Gate closed: a glide note jumps.
        render(&mut v, 4_800);
        v.note_on(45.0, false, true, 2_000);
        assert_eq!(v.pitch, 45.0);
    }

    #[test]
    fn retrigger_does_not_click() {
        let mut v = BassVoice::new(SR);
        // Dark filter: the saw edges are smooth, so a jump could only come from the retrigger.
        v.set_cutoff(0.0);
        v.note_on(45.0, false, false, 6_000);
        let mut out = render(&mut v, 3_000);
        v.note_on(52.0, false, false, 6_000);
        out.extend(render(&mut v, 3_000));
        let jump_in = |x: &[f32]| {
            x.windows(2)
                .fold(0.0_f32, |m, w| m.max((w[1] - w[0]).abs()))
        };
        // No bigger step at the retrigger than the waveform itself makes.
        let jump = jump_in(&out[2_990..3_010]);
        let steady = jump_in(&out[1_000..2_990]).max(jump_in(&out[3_010..]));
        assert!(jump <= steady, "{jump} > {steady}");
    }

    #[test]
    fn line_follows_the_bar_and_the_mute() {
        let mut line = BassLine::new();
        let mut v = BassVoice::new(SR);
        line.queue(1, true);
        line.queue_tonic(4, true);
        assert_eq!(line.current(), 0);
        line.fire(0, &mut v, true, gate_samples_fixed);
        assert_eq!(line.current(), 1, "switches on the bar");
        assert_eq!(line.key().tonic, 4);
        assert!(v.gated());
        assert_eq!(v.target, 40.0, "line B starts on degree 1 of E");
        line.set_muted(true);
        line.fire(1, &mut v, true, gate_samples_fixed);
        assert!(!v.gated(), "mute closes the gate");
        line.set_muted(false);
        line.fire(2, &mut v, true, gate_samples_fixed);
        assert!(v.gated());
    }

    fn gate_samples_fixed(len: u8, held: bool) -> u64 {
        gate_samples(u64::from(len) * 6_000, held)
    }
}
