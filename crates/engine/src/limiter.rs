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
            if !sample.is_finite() {
                *sample = 0.0;
                self.nan_count = self.nan_count.saturating_add(1);
            }
            let peak = sample.abs();
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
            *sample = (*sample * self.gain).clamp(-self.ceiling, self.ceiling);
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
    fn quiet_signal_passes_unchanged() {
        let mut limiter = Limiter::new(48_000);
        let mut buf = [0.25_f32; 64];
        limiter.process(&mut buf);
        assert!(buf.iter().all(|s| (s - 0.25).abs() < 1e-6));
    }
}
