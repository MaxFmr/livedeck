//! Master safety limiter and NaN guard: nothing above the ceiling ever reaches the DAC.
//!
//! Runs on: audio thread. Instant attack, smooth release, hard clip as a last resort.
//! A look-ahead version can replace it later; the guarantees stay the same.

#[derive(Debug, Clone, Copy)]
pub struct Limiter {
    ceiling: f32,
    gain: f32,
    release: f32,
    /// Non-finite samples replaced by silence since start.
    pub nan_count: u32,
}

/// -0.3 dBFS.
pub const CEILING: f32 = 0.966_051;
const RELEASE_SECONDS: f32 = 0.08;

impl Limiter {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            ceiling: CEILING,
            gain: 1.0,
            release: 1.0 - (-1.0 / (RELEASE_SECONDS * sr)).exp(),
            nan_count: 0,
        }
    }

    /// Processes a mono buffer in place.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            self.guard(sample);
            self.follow(sample.abs());
            *sample = (*sample * self.gain).clamp(-self.ceiling, self.ceiling);
        }
    }

    /// Processes a stereo pair in place, with one gain for both channels (the image stays put).
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            self.guard(l);
            self.guard(r);
            self.follow(l.abs().max(r.abs()));
            *l = (*l * self.gain).clamp(-self.ceiling, self.ceiling);
            *r = (*r * self.gain).clamp(-self.ceiling, self.ceiling);
        }
    }

    fn guard(&mut self, sample: &mut f32) {
        if !sample.is_finite() {
            *sample = 0.0;
            self.nan_count = self.nan_count.saturating_add(1);
        }
    }

    fn follow(&mut self, peak: f32) {
        let target = if peak * self.gain > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if target < self.gain {
            self.gain = target;
        } else {
            self.gain += (target - self.gain) * self.release;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_exceeds_ceiling_and_removes_nan() {
        let mut limiter = Limiter::new(48_000);
        let mut buf = [0.5, 4.0, -12.0, f32::NAN, f32::INFINITY, 0.1];
        limiter.process(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= CEILING));
        assert_eq!(limiter.nan_count, 2);
    }

    #[test]
    fn stereo_shares_one_gain() {
        let mut limiter = Limiter::new(48_000);
        let mut left = [2.0_f32; 8];
        let mut right = [0.5_f32; 8];
        limiter.process_stereo(&mut left, &mut right);
        assert!(left.iter().all(|s| s.abs() <= CEILING));
        // The quiet side is turned down as much as the loud one.
        assert!((left[0] / right[0] - 4.0).abs() < 1e-4);
    }

    #[test]
    fn quiet_signal_passes_unchanged() {
        let mut limiter = Limiter::new(48_000);
        let mut buf = [0.25_f32; 64];
        limiter.process(&mut buf);
        assert!(buf.iter().all(|s| (s - 0.25).abs() < 1e-6));
    }
}
