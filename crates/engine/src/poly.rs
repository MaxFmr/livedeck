//! Polyphonic synth of the melody deck: a fixed pool of 16 voices, each with two PolyBLEP
//! oscillators, a resonant low-pass and two ADSR envelopes (amplitude, filter). The sound comes
//! from a `SynthPatch`, one per style. Recipe: docs/rust/dsp-cookbook.md § Poly synthé.
//!
//! Notes carry an owner (the pad that played them) so releasing a pad releases its notes only,
//! even when two chords share a note.
//!
//! Runs on: audio thread. A note-on computes one `exp2`; filter coefficients (`tan`) are refreshed
//! every `CONTROL` samples per sounding voice.

use crate::dsp::{SILENCE, Svf, soft_clip};
use crate::smooth::Smoothed;

pub const VOICES: usize = 16;
/// Samples between two updates of a voice's filter.
const CONTROL: usize = 16;
/// Range of the brightness control around the patch cutoff, in octaves (±).
const BRIGHTNESS_OCTAVES: f32 = 2.5;
/// A stolen voice fades out this fast before its new note (no click).
const STEAL_S: f32 = 0.003;
/// Gain of one voice into the synth bus: 0.35 sounded too quiet under the drums (test of 016),
/// raised by 4 dB.
const MIX: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wave {
    Saw,
    Square,
}

/// Attack, decay, release in seconds; sustain 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Adsr {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

/// The sound of the synth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynthPatch {
    pub wave1: Wave,
    pub wave2: Wave,
    /// Oscillator 2 detune, cents (oscillator 1 is detuned by the opposite half).
    pub detune: f32,
    /// Filter cutoff with brightness in the middle, Hz.
    pub cutoff: f32,
    pub resonance: f32,
    /// Filter envelope depth, octaves.
    pub env_octaves: f32,
    pub amp: Adsr,
    pub filter: Adsr,
    pub level: f32,
}

impl Default for SynthPatch {
    /// Warm pad of the hybrid style.
    fn default() -> Self {
        Self {
            wave1: Wave::Saw,
            wave2: Wave::Saw,
            detune: 8.0,
            cutoff: 1_200.0,
            resonance: 0.9,
            env_octaves: 1.5,
            amp: Adsr {
                attack: 0.25,
                decay: 0.5,
                sustain: 0.8,
                release: 0.6,
            },
            filter: Adsr {
                attack: 0.3,
                decay: 0.8,
                sustain: 0.4,
                release: 0.6,
            },
            level: 0.5,
        }
    }
}

fn coef(sample_rate: f32, seconds: f32) -> f32 {
    1.0 - (-1.0 / (seconds.max(1.0e-4) * sample_rate)).exp()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Decay,
    Release,
}

/// Exponential ADSR: each segment is a one-pole towards its target (the attack aims past 1 so it
/// ends in finite time).
#[derive(Debug, Clone, Copy, Default)]
struct Envelope {
    stage: Stage,
    level: f32,
    attack: f32,
    decay: f32,
    release: f32,
    sustain: f32,
}

impl Envelope {
    fn set(&mut self, sr: f32, adsr: Adsr) {
        self.attack = coef(sr, adsr.attack / 3.0);
        self.decay = coef(sr, adsr.decay / 3.0);
        self.release = coef(sr, adsr.release / 3.0);
        self.sustain = adsr.sustain.clamp(0.0, 1.0);
    }

    fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    fn tick(&mut self) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.level += (1.3 - self.level) * self.attack;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => self.level += (self.sustain - self.level) * self.decay,
            Stage::Release => {
                self.level -= self.level * self.release;
                if self.level < SILENCE {
                    self.level = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.level
    }
}

/// PolyBLEP residual at phase `t` for an increment `dt`.
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

fn oscillator(wave: Wave, phase: f32, dt: f32) -> f32 {
    match wave {
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => {
            let naive = if phase < 0.5 { 1.0 } else { -1.0 };
            naive + poly_blep(phase, dt) - poly_blep((phase + 0.5) % 1.0, dt)
        }
    }
}

fn advance(phase: &mut f32, dt: f32) {
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Voice {
    note: f32,
    owner: u8,
    /// Note-on order, to steal the oldest.
    age: u64,
    velocity: f32,
    phases: [f32; 2],
    incs: [f32; 2],
    amp: Envelope,
    filter_env: Envelope,
    filter: Svf,
    control: usize,
    /// Fading out before a new note takes the voice (stealing).
    stealing: Option<(f32, f32, u8)>,
    steal_gain: f32,
}

impl Voice {
    fn sounding(&self) -> bool {
        self.amp.stage != Stage::Idle
    }

    fn released(&self) -> bool {
        self.amp.stage == Stage::Release
    }
}

#[derive(Debug, Clone)]
pub struct PolySynth {
    sample_rate: f32,
    voices: [Voice; VOICES],
    patch: SynthPatch,
    /// Fader 0..=1, 0.5 = the patch cutoff.
    brightness: Smoothed,
    counter: u64,
    steal_coef: f32,
}

impl PolySynth {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let mut synth = Self {
            sample_rate: sr,
            voices: [Voice::default(); VOICES],
            patch: SynthPatch::default(),
            brightness: Smoothed::new(0.5, sample_rate, 0.02),
            counter: 0,
            steal_coef: 1.0 - coef(sr, STEAL_S),
        };
        synth.set_patch(SynthPatch::default());
        synth
    }

    /// Sound of the next notes (and of the envelopes of the notes sounding).
    pub fn set_patch(&mut self, patch: SynthPatch) {
        self.patch = patch;
        let sr = self.sample_rate;
        for v in &mut self.voices {
            v.amp.set(sr, patch.amp);
            v.filter_env.set(sr, patch.filter);
        }
    }

    pub fn set_brightness(&mut self, value: f32) {
        if value.is_finite() {
            self.brightness.set(value.clamp(0.0, 1.0));
        }
    }

    /// Voices sounding (attack, decay or release).
    #[must_use]
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.sounding()).count()
    }

    /// Plays `note` (MIDI) for `owner` (a pad), at `velocity` 0..=1.
    pub fn note_on(&mut self, note: f32, velocity: f32, owner: u8) {
        self.counter += 1;
        let age = self.counter;
        // A free voice, else the oldest released, else the oldest.
        let pick = self
            .voices
            .iter()
            .position(|v| !v.sounding() && v.stealing.is_none())
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.released())
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
            })
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
            });
        let Some(voice) = pick.and_then(|i| self.voices.get_mut(i)) else {
            return;
        };
        voice.age = age;
        if voice.sounding() {
            // Steal: fade the old note out, then start the new one.
            voice.stealing = Some((note, velocity.clamp(0.0, 1.0), owner));
            voice.steal_gain = 1.0;
            voice.owner = owner;
            return;
        }
        start(voice, self.sample_rate, &self.patch, note, velocity, owner);
    }

    /// Releases the notes of `owner`.
    pub fn release(&mut self, owner: u8) {
        for v in &mut self.voices {
            if let Some((_, _, o)) = v.stealing
                && o == owner
            {
                v.stealing = None;
                v.amp.gate_off();
                v.filter_env.gate_off();
            } else if v.owner == owner && v.sounding() {
                v.amp.gate_off();
                v.filter_env.gate_off();
            }
        }
    }

    /// Releases everything (transport stop, mute).
    pub fn release_all(&mut self) {
        for v in &mut self.voices {
            v.stealing = None;
            v.amp.gate_off();
            v.filter_env.gate_off();
        }
    }

    /// Adds every sounding voice into `out`.
    pub fn render_add(&mut self, out: &mut [f32]) {
        let bright = self.brightness.skip(out.len());
        let base = self.patch.cutoff * ((bright - 0.5) * 2.0 * BRIGHTNESS_OCTAVES).exp2();
        let sr = self.sample_rate;
        let patch = self.patch;
        let steal_coef = self.steal_coef;
        for v in &mut self.voices {
            if !v.sounding() && v.stealing.is_none() {
                continue;
            }
            for sample in out.iter_mut() {
                if let Some((note, velocity, owner)) = v.stealing {
                    v.steal_gain *= steal_coef;
                    if v.steal_gain < SILENCE {
                        v.stealing = None;
                        v.amp = Envelope {
                            stage: Stage::Idle,
                            level: 0.0,
                            ..v.amp
                        };
                        start(v, sr, &patch, note, velocity, owner);
                    }
                }
                if v.control == 0 {
                    let env = v.filter_env.level * patch.env_octaves;
                    v.filter.set(sr, base * env.exp2(), patch.resonance);
                    v.control = CONTROL;
                }
                v.control -= 1;
                let amp = v.amp.tick();
                v.filter_env.tick();
                let [p1, p2] = &mut v.phases;
                let [d1, d2] = v.incs;
                let x = oscillator(patch.wave1, *p1, d1) + oscillator(patch.wave2, *p2, d2);
                advance(p1, d1);
                advance(p2, d2);
                let gain = if v.stealing.is_some() {
                    v.steal_gain
                } else {
                    1.0
                };
                let y = v.filter.process(x * 0.5).lp;
                *sample += y * amp * v.velocity * patch.level * MIX * gain;
                if !v.sounding() && v.stealing.is_none() {
                    v.filter.reset();
                    break;
                }
            }
        }
        for sample in out.iter_mut() {
            *sample = soft_clip(*sample);
        }
    }
}

/// Starts `note` on a free voice.
fn start(v: &mut Voice, sr: f32, patch: &SynthPatch, note: f32, velocity: f32, owner: u8) {
    let hz = 440.0 * ((note - 69.0) / 12.0).exp2();
    let half = patch.detune / 2.0 / 1_200.0;
    v.note = note;
    v.owner = owner;
    v.velocity = velocity.clamp(0.0, 1.0);
    v.incs = [hz * (-half).exp2() / sr, hz * half.exp2() / sr];
    v.phases = [0.0, 0.37];
    v.filter.reset();
    v.control = 0;
    v.amp.level = 0.0;
    v.filter_env.level = 0.0;
    v.amp.gate_on();
    v.filter_env.gate_on();
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn render(s: &mut PolySynth, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        for chunk in out.chunks_mut(128) {
            s.render_add(chunk);
        }
        out
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn a_chord_sounds_then_releases_to_silence() {
        let mut s = PolySynth::new(SR);
        for note in [60.0, 64.0, 67.0] {
            s.note_on(note, 1.0, 0);
        }
        assert_eq!(s.active_voices(), 3);
        let held = render(&mut s, 24_000);
        assert!(held.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        assert!(rms(&held[12_000..]) > 0.02);
        s.release(0);
        render(&mut s, 96_000);
        assert_eq!(s.active_voices(), 0);
        assert!(render(&mut s, 128).iter().all(|&x| x == 0.0));
    }

    #[test]
    fn releasing_a_pad_keeps_the_notes_of_another() {
        let mut s = PolySynth::new(SR);
        s.note_on(60.0, 1.0, 0);
        s.note_on(60.0, 1.0, 1); // same note, other pad
        render(&mut s, 4_800);
        s.release(0);
        render(&mut s, 96_000);
        assert_eq!(s.active_voices(), 1, "pad 1 still holds its C");
    }

    #[test]
    fn steals_the_oldest_voice_without_a_click() {
        let mut s = PolySynth::new(SR);
        s.set_patch(SynthPatch {
            amp: Adsr {
                attack: 0.001,
                decay: 0.1,
                sustain: 1.0,
                release: 0.5,
            },
            ..SynthPatch::default()
        });
        for i in 0..VOICES {
            s.note_on(48.0 + i as f32, 1.0, 0);
        }
        let before = render(&mut s, 4_800);
        s.note_on(90.0, 1.0, 1);
        let after = render(&mut s, 4_800);
        assert_eq!(s.active_voices(), VOICES);
        let step = |x: &[f32]| {
            x.windows(2)
                .fold(0.0_f32, |m, w| m.max((w[1] - w[0]).abs()))
        };
        assert!(step(&after[..400]) <= step(&before[2_400..]) * 1.5);
        assert!(
            s.voices.iter().any(|v| v.note == 90.0),
            "the new note got a voice"
        );
    }

    #[test]
    fn brightness_opens_the_filter() {
        let level = |b: f32| {
            let mut s = PolySynth::new(SR);
            s.set_brightness(b);
            render(&mut s, 2_000); // let the smoothing settle
            s.note_on(72.0, 1.0, 0);
            let out = render(&mut s, 24_000);
            // High-frequency energy: first difference.
            let diff: Vec<f32> = out.windows(2).map(|w| w[1] - w[0]).collect();
            rms(&diff[12_000..])
        };
        assert!(level(1.0) > 2.0 * level(0.0));
    }
}
