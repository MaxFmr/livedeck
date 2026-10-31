//! One-pole parameter smoothing: removes the "zipper" steps of 7- or 14-bit controller values.
//!
//! Runs on: audio thread.

#[derive(Debug, Clone, Copy)]
pub struct Smoothed {
    value: f32,
    target: f32,
    coef: f32,
}

/// Below this distance the value snaps to the target (no endless denormal tail).
const SNAP: f32 = 1.0e-5;

impl Smoothed {
    /// `time_s` is the time constant (≈ 63 % of the way); 10 ms suits gains.
    #[must_use]
    pub fn new(initial: f32, sample_rate: u32, time_s: f32) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            value: initial,
            target: initial,
            coef: 1.0 - (-1.0 / (time_s.max(1.0e-4) * sr)).exp(),
        }
    }

    pub fn set(&mut self, target: f32) {
        if target.is_finite() {
            self.target = target;
        }
    }

    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Advances one sample.
    pub fn tick(&mut self) -> f32 {
        let delta = self.target - self.value;
        let step = delta * self.coef;
        // Snap when close, when the step no longer changes an f32 near the target, and never
        // cross the target because of rounding.
        if delta.abs() < SNAP || step.abs() >= delta.abs() || self.value + step == self.value {
            self.value = self.target;
        } else {
            self.value += step;
        }
        self.value
    }

    /// Advances `n` samples at once (block-rate parameters).
    pub fn skip(&mut self, n: usize) -> f32 {
        let delta = self.target - self.value;
        if delta.abs() < SNAP {
            self.value = self.target;
        } else {
            let remaining = (1.0 - self.coef).powi(i32::try_from(n).unwrap_or(i32::MAX));
            self.value = self.target - delta * remaining;
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_without_overshoot() {
        let mut s = Smoothed::new(0.0, 48_000, 0.01);
        s.set(1.0);
        let mut last = 0.0;
        for _ in 0..48_000 {
            let v = s.tick();
            assert!(v >= last && v <= 1.0);
            last = v;
        }
        assert_eq!(last, 1.0);
    }

    #[test]
    fn skip_matches_per_sample_steps() {
        let mut a = Smoothed::new(0.0, 48_000, 0.01);
        let mut b = a;
        a.set(1.0);
        b.set(1.0);
        for _ in 0..128 {
            a.tick();
        }
        assert!((a.skip(0) - b.skip(128)).abs() < 1.0e-4);
    }
}
