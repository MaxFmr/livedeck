//! Small DSP building blocks shared by the voices: noise, state-variable filter, envelopes,
//! soft clipping. All state is inline; nothing allocates.
//!
//! Runs on: audio thread.

use core::f32::consts::PI;

/// Below this level an envelope is considered finished (also keeps clear of denormals).
pub const SILENCE: f32 = 1.0e-4;

/// White noise from a 32-bit xorshift (no `rand` in the engine).
#[derive(Debug, Clone, Copy)]
pub struct Noise {
    state: u32,
}

impl Noise {
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    /// Next sample in [-1, 1].
    pub fn sample(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Simultaneous outputs of the state-variable filter.
#[derive(Debug, Clone, Copy)]
pub struct SvfOut {
    pub lp: f32,
    pub bp: f32,
    pub hp: f32,
}

/// Trapezoidal (TPT) state-variable filter, Cytomic/Simper form: stable under modulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Svf {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// Sets cutoff `fc` (Hz) and resonance `q`; `tan` runs here, not per sample.
    pub fn set(&mut self, sample_rate: f32, fc: f32, q: f32) {
        let fc = fc.clamp(10.0, sample_rate * 0.49);
        let g = (PI * fc / sample_rate).tan();
        self.k = 1.0 / q.max(0.1);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }

    pub fn reset(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }

    pub fn process(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut {
            lp: v2,
            bp: v1,
            hp: x - self.k * v1 - v2,
        }
    }
}

/// Exponential decay: starts at the trigger level and falls to -60 dB in the given time.
#[derive(Debug, Clone, Copy, Default)]
pub struct Decay {
    pub level: f32,
    coef: f32,
}

impl Decay {
    /// Per-sample multiplier reaching -60 dB after `seconds`.
    #[must_use]
    pub fn coef_for(sample_rate: f32, seconds: f32) -> f32 {
        (-6.907_755 / (seconds.max(1.0e-3) * sample_rate)).exp()
    }

    pub fn trigger(&mut self, level: f32, sample_rate: f32, seconds: f32) {
        self.level = level;
        self.coef = Self::coef_for(sample_rate, seconds);
    }

    /// Shortens the remaining decay (e.g. an open hat choked by a closed one).
    pub fn set_time(&mut self, sample_rate: f32, seconds: f32) {
        self.coef = Self::coef_for(sample_rate, seconds);
    }

    pub fn active(&self) -> bool {
        self.level >= SILENCE
    }

    /// Current level, then advances one sample.
    pub fn tick(&mut self) -> f32 {
        let out = self.level;
        self.level *= self.coef;
        if self.level < SILENCE {
            self.level = 0.0;
        }
        out
    }
}

/// Soft clip, Padé approximation of `tanh` (exact enough below |x| = 3, clamped beyond).
#[must_use]
pub fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_stays_in_range_and_is_not_constant() {
        let mut n = Noise::new(1);
        let samples: Vec<f32> = (0..10_000).map(|_| n.sample()).collect();
        assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05);
    }

    #[test]
    fn svf_lowpass_passes_dc_and_highpass_blocks_it() {
        let mut f = Svf::default();
        f.set(48_000.0, 1_000.0, 0.707);
        let mut out = f.process(1.0);
        for _ in 0..48_000 {
            out = f.process(1.0);
        }
        assert!((out.lp - 1.0).abs() < 1.0e-3);
        assert!(out.hp.abs() < 1.0e-3);
    }

    #[test]
    fn decay_reaches_silence_in_time() {
        let mut d = Decay::default();
        d.trigger(1.0, 48_000.0, 0.1);
        for _ in 0..4_800 {
            d.tick();
        }
        assert!(d.level < 1.1e-3 && d.level > 0.9e-3);
        for _ in 0..48_000 {
            d.tick();
        }
        assert!(!d.active());
        assert_eq!(d.level, 0.0);
    }

    #[test]
    fn soft_clip_is_bounded_and_close_to_tanh() {
        for i in -60..=60 {
            let x = i as f32 / 10.0;
            assert!(soft_clip(x).abs() <= 1.0);
            if x.abs() <= 1.5 {
                assert!((soft_clip(x) - x.tanh()).abs() < 0.03, "{x}");
            }
        }
    }
}
