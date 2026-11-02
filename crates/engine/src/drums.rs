//! Synthetic drum kit: one monophonic instance per voice, like a drum machine (a retrigger restarts
//! the voice). Default sound: « hybrid » — long round kick (808-like), dry snare and metallic hats
//! (909-like). Recipes: docs/rust/dsp-cookbook.md § Batterie.
//!
//! Runs on: audio thread. Triggering computes filter coefficients (`tan`, `exp`); rendering is
//! plain arithmetic.

use core::f32::consts::TAU;

use crate::dsp::{Decay, Noise, Svf, soft_clip};
use crate::sampler::{SampleBank, SamplePlayer};

/// Gain of the sample kit: the TR-909 samples sounded 0.7–4.4 dB below the synth depending on the
/// style, too quiet when switching (test of 015) → +3 dB.
const SAMPLE_GAIN: f32 = 1.41;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrumVoice {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
    Clap,
    Perc,
}

impl DrumVoice {
    pub const ALL: [Self; 6] = [
        Self::Kick,
        Self::Snare,
        Self::ClosedHat,
        Self::OpenHat,
        Self::Clap,
        Self::Perc,
    ];

    /// Name used in files (`config/mapping.toml`, presets).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Kick => "kick",
            Self::Snare => "snare",
            Self::ClosedHat => "closed_hat",
            Self::OpenHat => "open_hat",
            Self::Clap => "clap",
            Self::Perc => "perc",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == name)
    }
}

/// Sound-shaping knobs of one voice; presets (014) will set them per style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceParams {
    /// Semitones from the default pitch.
    pub tune: f32,
    /// Multiplies the default decay times.
    pub decay: f32,
    pub level: f32,
}

impl Default for VoiceParams {
    fn default() -> Self {
        Self {
            tune: 0.0,
            decay: 1.0,
            level: 1.0,
        }
    }
}

fn ratio(semitones: f32) -> f32 {
    (semitones / 12.0).exp2()
}

#[derive(Debug, Clone, Copy, Default)]
struct Kick {
    phase: f32,
    base: f32,
    sweep: f32,
    pitch: Decay,
    amp: Decay,
    click: Decay,
}

impl Kick {
    fn trigger(&mut self, sr: f32, p: VoiceParams, vel: f32) {
        self.phase = 0.0;
        self.base = 48.0 * ratio(p.tune);
        self.sweep = 110.0 * ratio(p.tune);
        self.pitch.trigger(1.0, sr, 0.09);
        self.amp.trigger(vel * p.level, sr, 0.55 * p.decay);
        self.click.trigger(0.25 * vel * p.level, sr, 0.004);
    }

    fn next(&mut self, sr: f32, noise: &mut Noise) -> f32 {
        if !self.amp.active() {
            return 0.0;
        }
        let freq = self.base + self.sweep * self.pitch.tick();
        self.phase += freq / sr;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let body = (self.phase * TAU).sin() * self.amp.tick();
        soft_clip(1.5 * body + noise.sample() * self.click.tick()) * 0.95
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Snare {
    phases: [f32; 2],
    freqs: [f32; 2],
    body: Decay,
    snap: Decay,
    filter: Svf,
}

impl Snare {
    fn trigger(&mut self, sr: f32, p: VoiceParams, vel: f32) {
        let r = ratio(p.tune);
        self.phases = [0.0, 0.25];
        self.freqs = [185.0 * r, 330.0 * r];
        self.body.trigger(0.6 * vel * p.level, sr, 0.09 * p.decay);
        self.snap.trigger(0.7 * vel * p.level, sr, 0.17 * p.decay);
        self.filter.set(sr, 1_800.0, 0.8);
    }

    fn next(&mut self, sr: f32, noise: &mut Noise) -> f32 {
        if !self.body.active() && !self.snap.active() {
            return 0.0;
        }
        let mut tone = 0.0;
        for (phase, freq) in self.phases.iter_mut().zip(self.freqs) {
            *phase += freq / sr;
            if *phase >= 1.0 {
                *phase -= 1.0;
            }
            tone += (*phase * TAU).sin();
        }
        let snap = self.filter.process(noise.sample()).hp;
        0.5 * tone * self.body.tick() + snap * self.snap.tick()
    }
}

/// Inharmonic square bank of the classic analogue hats, scaled by `tune`.
const HAT_FREQS: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

#[derive(Debug, Clone, Copy, Default)]
struct Hat {
    phases: [f32; 6],
    freqs: [f32; 6],
    amp: Decay,
    filter: Svf,
}

impl Hat {
    fn trigger(&mut self, sr: f32, p: VoiceParams, vel: f32, seconds: f32) {
        let r = 1.7 * ratio(p.tune);
        for (f, base) in self.freqs.iter_mut().zip(HAT_FREQS) {
            *f = base * r;
        }
        self.amp
            .trigger(0.55 * vel * p.level, sr, seconds * p.decay);
        self.filter.set(sr, 7_500.0, 0.9);
    }

    fn choke(&mut self, sr: f32) {
        self.amp.set_time(sr, 0.01);
    }

    fn next(&mut self, sr: f32, noise: &mut Noise) -> f32 {
        if !self.amp.active() {
            return 0.0;
        }
        let mut metal = 0.0;
        for (phase, freq) in self.phases.iter_mut().zip(self.freqs) {
            *phase += freq / sr;
            if *phase >= 1.0 {
                *phase -= 1.0;
            }
            metal += if *phase < 0.5 { 1.0 } else { -1.0 };
        }
        let source = metal / 6.0 + 0.4 * noise.sample();
        self.filter.process(source).hp * self.amp.tick()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Clap {
    /// Samples since the trigger; `u32::MAX` when idle.
    t: u32,
    burst_gap: u32,
    burst: Decay,
    tail: Decay,
    vel: f32,
    tail_seconds: f32,
    filter: Svf,
}

impl Clap {
    fn trigger(&mut self, sr: f32, p: VoiceParams, vel: f32) {
        self.t = 0;
        self.burst_gap = (0.011 * sr) as u32;
        self.vel = vel * p.level;
        self.tail_seconds = 0.2 * p.decay;
        self.burst.trigger(self.vel, sr, 0.012);
        self.tail.level = 0.0;
        self.filter.set(sr, 1_150.0 * ratio(p.tune), 1.8);
    }

    fn next(&mut self, sr: f32, noise: &mut Noise) -> f32 {
        if self.t == u32::MAX {
            return 0.0;
        }
        // Three hand claps 11 ms apart, then the room tail from the third one.
        if self.t == self.burst_gap || self.t == 2 * self.burst_gap {
            self.burst.trigger(self.vel, sr, 0.012);
        }
        if self.t == 2 * self.burst_gap {
            self.tail.trigger(0.7 * self.vel, sr, self.tail_seconds);
        }
        self.t += 1;
        let env = self.burst.tick().max(self.tail.tick());
        if self.t > 2 * self.burst_gap && !self.burst.active() && !self.tail.active() {
            self.t = u32::MAX;
        }
        2.0 * self.filter.process(noise.sample()).bp * env
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Perc {
    phase: f32,
    base: f32,
    pitch: Decay,
    amp: Decay,
}

impl Perc {
    fn trigger(&mut self, sr: f32, p: VoiceParams, vel: f32) {
        self.phase = 0.0;
        self.base = 220.0 * ratio(p.tune);
        self.pitch.trigger(0.6, sr, 0.06);
        self.amp.trigger(0.7 * vel * p.level, sr, 0.25 * p.decay);
    }

    fn next(&mut self, sr: f32) -> f32 {
        if !self.amp.active() {
            return 0.0;
        }
        self.phase += self.base * (1.0 + self.pitch.tick()) / sr;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        (self.phase * TAU).sin() * self.amp.tick()
    }
}

/// The whole kit. Everything is inline: building it allocates nothing either.
#[derive(Debug, Clone)]
pub struct DrumKit {
    sample_rate: f32,
    params: [VoiceParams; 6],
    noise: Noise,
    kick: Kick,
    snare: Snare,
    closed_hat: Hat,
    open_hat: Hat,
    clap: Clap,
    perc: Perc,
    /// Sample kit (015), loaded before the stream starts; played instead of the synth when on.
    samples: Option<SamplePlayer>,
    use_samples: bool,
}

fn slot(voice: DrumVoice) -> usize {
    match voice {
        DrumVoice::Kick => 0,
        DrumVoice::Snare => 1,
        DrumVoice::ClosedHat => 2,
        DrumVoice::OpenHat => 3,
        DrumVoice::Clap => 4,
        DrumVoice::Perc => 5,
    }
}

impl DrumKit {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate.max(1) as f32,
            params: [VoiceParams::default(); 6],
            noise: Noise::new(0x2545_F491),
            kick: Kick::default(),
            snare: Snare::default(),
            closed_hat: Hat::default(),
            open_hat: Hat::default(),
            clap: Clap {
                t: u32::MAX,
                ..Clap::default()
            },
            perc: Perc::default(),
            samples: None,
            use_samples: false,
        }
    }

    /// Hands over a decoded sample kit (control thread, before the stream starts).
    pub fn set_samples(&mut self, bank: SampleBank) {
        self.samples = Some(SamplePlayer::new(bank, self.sample_rate as u32));
    }

    #[must_use]
    pub fn has_samples(&self) -> bool {
        self.samples.is_some()
    }

    /// Next hits on the samples (voices without a sample stay synthetic) or the synth. Notes
    /// already sounding finish on their own.
    pub fn set_use_samples(&mut self, on: bool) {
        self.use_samples = on && self.samples.is_some();
    }

    #[must_use]
    pub fn use_samples(&self) -> bool {
        self.use_samples
    }

    pub fn set_params(&mut self, voice: DrumVoice, params: VoiceParams) {
        if let Some(p) = self.params.get_mut(slot(voice)) {
            *p = params;
        }
    }

    /// Starts `voice` at the next rendered sample; `velocity` 0..=1. The closed hat chokes the open one.
    pub fn trigger(&mut self, voice: DrumVoice, velocity: f32) {
        let vel = if velocity.is_finite() {
            velocity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let sr = self.sample_rate;
        let p = self.params.get(slot(voice)).copied().unwrap_or_default();
        if self.use_samples
            && let Some(samples) = self.samples.as_mut()
            && samples.trigger(voice, p, vel)
        {
            if voice == DrumVoice::ClosedHat {
                // The synthetic open hat may still ring from before the switch.
                self.open_hat.choke(sr);
            }
            return;
        }
        match voice {
            DrumVoice::Kick => self.kick.trigger(sr, p, vel),
            DrumVoice::Snare => self.snare.trigger(sr, p, vel),
            DrumVoice::ClosedHat => {
                self.open_hat.choke(sr);
                self.closed_hat.trigger(sr, p, vel, 0.045);
            }
            DrumVoice::OpenHat => self.open_hat.trigger(sr, p, vel, 0.32),
            DrumVoice::Clap => self.clap.trigger(sr, p, vel),
            DrumVoice::Perc => self.perc.trigger(sr, p, vel),
        }
    }

    /// Adds the kit, scaled by `gain`, into a mono buffer.
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        let sr = self.sample_rate;
        for sample in out.iter_mut() {
            let n = &mut self.noise;
            let mix = self.kick.next(sr, n)
                + self.snare.next(sr, n)
                + self.closed_hat.next(sr, n)
                + self.open_hat.next(sr, n)
                + self.clap.next(sr, n)
                + self.perc.next(sr);
            *sample += mix * gain;
        }
        if let Some(samples) = self.samples.as_mut() {
            samples.render_add(out, gain * SAMPLE_GAIN);
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn render(kit: &mut DrumKit, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames];
        for chunk in out.chunks_mut(128) {
            kit.render_add(chunk, 1.0);
        }
        out
    }

    fn peak(x: &[f32]) -> f32 {
        x.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn idle_kit_is_silent() {
        let mut kit = DrumKit::new(SR);
        assert!(render(&mut kit, 4_800).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn every_voice_sounds_then_stops() {
        for voice in DrumVoice::ALL {
            let mut kit = DrumKit::new(SR);
            kit.trigger(voice, 1.0);
            let out = render(&mut kit, SR as usize * 2);
            let p = peak(&out[..SR as usize / 2]);
            assert!(p > 0.1, "{voice:?} too quiet: {p}");
            assert!(p <= 1.5, "{voice:?} too loud: {p}");
            assert!(out.iter().all(|s| s.is_finite()), "{voice:?}");
            assert!(
                peak(&out[SR as usize * 3 / 2..]) == 0.0,
                "{voice:?} never ends"
            );
        }
    }

    #[test]
    fn closed_hat_chokes_open_hat() {
        let mut open = DrumKit::new(SR);
        open.trigger(DrumVoice::OpenHat, 1.0);
        render(&mut open, 2_400);
        let mut choked = open.clone();
        choked.trigger(DrumVoice::ClosedHat, 0.0); // silent closed hat: only the choke remains
        let tail = |kit: &mut DrumKit| {
            render(kit, 4_800);
            peak(&render(kit, 2_400))
        };
        assert!(tail(&mut open) > 0.01);
        assert!(tail(&mut choked) < 1.0e-3);
    }

    #[test]
    fn velocity_scales_the_level() {
        let loud = {
            let mut kit = DrumKit::new(SR);
            kit.trigger(DrumVoice::Perc, 1.0);
            peak(&render(&mut kit, 4_800))
        };
        let soft = {
            let mut kit = DrumKit::new(SR);
            kit.trigger(DrumVoice::Perc, 0.25);
            peak(&render(&mut kit, 4_800))
        };
        assert!((soft / loud - 0.25).abs() < 0.02);
    }

    #[test]
    fn names_round_trip() {
        for voice in DrumVoice::ALL {
            assert_eq!(DrumVoice::from_name(voice.name()), Some(voice));
        }
        assert_eq!(DrumVoice::from_name("cowbell"), None);
    }
}
