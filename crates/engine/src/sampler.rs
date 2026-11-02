//! Sample playback for the drum kit: one sample per voice, played at any rate with linear
//! interpolation (sample rate conversion and tuning in one), cut short by the voice's decay.
//!
//! The samples are decoded on the control thread (`io::samples`) and handed over with the engine,
//! before the stream starts: nothing is loaded or freed on the audio thread.
//!
//! Runs on: audio thread (trigger, render).

use crate::drums::{DrumVoice, VoiceParams};
use crate::dsp::SILENCE;

/// Retriggered or choked notes fade out over this long instead of stopping dead (clicks).
const RETRIGGER_FADE_S: f32 = 0.002;
const CHOKE_FADE_S: f32 = 0.01;

/// Decoded samples, mono, in `DrumVoice::ALL` order; an empty one leaves its voice to the synth.
#[derive(Debug, Clone, Default)]
pub struct SampleBank {
    pub sample_rate: u32,
    pub voices: [Vec<f32>; 6],
}

impl SampleBank {
    /// The sample of `voice`, if it has one.
    #[must_use]
    pub fn sample(&self, voice: DrumVoice) -> Option<&[f32]> {
        let i = DrumVoice::ALL.iter().position(|&v| v == voice)?;
        self.voices
            .get(i)
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }
}

/// One playing instance of a sample.
#[derive(Debug, Clone, Copy, Default)]
struct Playing {
    pos: f64,
    rate: f64,
    gain: f32,
    /// Decay envelope and its per-sample multiplier.
    env: f32,
    coef: f32,
    /// Fade-out multiplier per sample (1 = not fading) and current fade level.
    fade_coef: f32,
    fade: f32,
    active: bool,
}

impl Playing {
    fn next(&mut self, data: &[f32]) -> f32 {
        if !self.active {
            return 0.0;
        }
        let i = self.pos as usize;
        let (Some(&a), b) = (data.get(i), data.get(i + 1).copied().unwrap_or(0.0)) else {
            self.active = false;
            return 0.0;
        };
        let frac = (self.pos - i as f64) as f32;
        let out = (a + (b - a) * frac) * self.gain * self.env * self.fade;
        self.pos += self.rate;
        self.env *= self.coef;
        self.fade *= self.fade_coef;
        if self.env * self.fade * self.gain < SILENCE * 0.1 {
            self.active = false;
        }
        out
    }

    fn fade_out(&mut self, coef: f32) {
        self.fade_coef = self.fade_coef.min(coef);
    }
}

/// Main note and the fading tail of the previous one, per voice.
#[derive(Debug, Clone, Copy, Default)]
struct Slot {
    main: Playing,
    tail: Playing,
}

#[derive(Debug, Clone)]
pub struct SamplePlayer {
    bank: SampleBank,
    out_rate: f32,
    slots: [Slot; 6],
    retrigger_coef: f32,
    choke_coef: f32,
}

/// Per-sample multiplier reaching -60 dB after `seconds`.
fn fade_coef(sample_rate: f32, seconds: f32) -> f32 {
    (-6.907_755 / (seconds.max(1.0e-4) * sample_rate)).exp()
}

impl SamplePlayer {
    /// Takes ownership of decoded samples (control thread).
    #[must_use]
    pub fn new(bank: SampleBank, out_rate: u32) -> Self {
        let sr = out_rate.max(1) as f32;
        Self {
            bank,
            out_rate: sr,
            slots: [Slot::default(); 6],
            retrigger_coef: fade_coef(sr, RETRIGGER_FADE_S),
            choke_coef: fade_coef(sr, CHOKE_FADE_S),
        }
    }

    #[must_use]
    pub fn has(&self, voice: DrumVoice) -> bool {
        self.bank.sample(voice).is_some()
    }

    /// Starts the sample of `voice` (if any); returns false when it has none.
    pub fn trigger(&mut self, voice: DrumVoice, params: VoiceParams, velocity: f32) -> bool {
        let Some(len) = self.bank.sample(voice).map(<[f32]>::len) else {
            return false;
        };
        let Some(i) = DrumVoice::ALL.iter().position(|&v| v == voice) else {
            return false;
        };
        let rate = f64::from(self.bank.sample_rate) / f64::from(self.out_rate)
            * f64::from((params.tune / 12.0).exp2());
        // A decay below 1 shortens the sample: -60 dB at that fraction of its length.
        let coef = if params.decay < 1.0 {
            let seconds = len as f32 / rate as f32 / self.out_rate * params.decay.max(0.05);
            fade_coef(self.out_rate, seconds)
        } else {
            1.0
        };
        if voice == DrumVoice::ClosedHat
            && let Some(open) = DrumVoice::ALL
                .iter()
                .position(|&v| v == DrumVoice::OpenHat)
                .and_then(|o| self.slots.get_mut(o))
        {
            open.main.fade_out(self.choke_coef);
            open.tail.fade_out(self.choke_coef);
        }
        let retrigger = self.retrigger_coef;
        let Some(slot) = self.slots.get_mut(i) else {
            return false;
        };
        if slot.main.active {
            slot.tail = slot.main;
            slot.tail.fade_out(retrigger);
        }
        slot.main = Playing {
            pos: 0.0,
            rate,
            gain: velocity.clamp(0.0, 1.0) * params.level,
            env: 1.0,
            coef,
            fade_coef: 1.0,
            fade: 1.0,
            active: true,
        };
        true
    }

    /// Adds every playing sample, scaled by `gain`, into `out`.
    pub fn render_add(&mut self, out: &mut [f32], gain: f32) {
        for (voice, slot) in DrumVoice::ALL.into_iter().zip(self.slots.iter_mut()) {
            if !slot.main.active && !slot.tail.active {
                continue;
            }
            let Some(data) = self.bank.sample(voice) else {
                continue;
            };
            for sample in out.iter_mut() {
                *sample += (slot.main.next(data) + slot.tail.next(data)) * gain;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn bank(rate: u32, kick: Vec<f32>) -> SampleBank {
        let mut bank = SampleBank {
            sample_rate: rate,
            ..SampleBank::default()
        };
        bank.voices[0] = kick;
        bank
    }

    fn render(p: &mut SamplePlayer, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        p.render_add(&mut out, 1.0);
        out
    }

    #[test]
    fn plays_the_sample_as_is_at_the_same_rate() {
        let data: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let mut p = SamplePlayer::new(bank(48_000, data.clone()), 48_000);
        assert!(p.trigger(DrumVoice::Kick, VoiceParams::default(), 1.0));
        assert!(
            !p.trigger(DrumVoice::Snare, VoiceParams::default(), 1.0),
            "no sample"
        );
        let out = render(&mut p, 120);
        for i in 0..99 {
            assert!((out[i] - data[i]).abs() < 1.0e-6, "{i}");
        }
        assert!(out[101..].iter().all(|&s| s == 0.0), "stops at the end");
    }

    #[test]
    fn converts_the_rate_and_tunes() {
        // 44.1 kHz sample on a 48 kHz output: 441 samples last 480.
        let mut p = SamplePlayer::new(bank(44_100, vec![1.0; 441]), 48_000);
        p.trigger(DrumVoice::Kick, VoiceParams::default(), 1.0);
        let out = render(&mut p, 600);
        let len = out.iter().filter(|s| s.abs() > 0.0).count();
        assert!((479..=481).contains(&len), "{len}");
        // An octave up: half as long.
        let mut p = SamplePlayer::new(bank(48_000, vec![1.0; 480]), 48_000);
        let up = VoiceParams {
            tune: 12.0,
            ..VoiceParams::default()
        };
        p.trigger(DrumVoice::Kick, up, 1.0);
        let len = render(&mut p, 600).iter().filter(|s| s.abs() > 0.0).count();
        assert!((239..=241).contains(&len), "{len}");
    }

    #[test]
    fn short_decay_fades_and_retrigger_does_not_click() {
        let mut p = SamplePlayer::new(bank(48_000, vec![0.5; 48_000]), 48_000);
        let short = VoiceParams {
            decay: 0.5,
            ..VoiceParams::default()
        };
        p.trigger(DrumVoice::Kick, short, 1.0);
        let out = render(&mut p, 48_000);
        assert!(out[23_000].abs() < 1.0e-3, "-60 dB at half the length");
        p.trigger(DrumVoice::Kick, VoiceParams::default(), 1.0);
        render(&mut p, 1_000);
        p.trigger(DrumVoice::Kick, VoiceParams::default(), 0.5);
        let out = render(&mut p, 200);
        // The old note is not cut: it fades out under the new one in a few ms.
        assert!(out[0] > 0.7, "{}", out[0]);
        assert!(
            out.windows(2).all(|w| w[1] <= w[0] + 1.0e-6),
            "falls smoothly"
        );
        assert!((out[199] - 0.25).abs() < 0.01);
    }
}
