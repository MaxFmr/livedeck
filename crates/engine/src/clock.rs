//! Musical clock: the sample counter is the source of truth, beats are derived from it.
//!
//! Runs on: audio thread.

/// Transport and tempo. Beat `n` starts at sample `round(n * samples_per_beat)`, computed from the
/// beat index each time so rounding never accumulates.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    sample_rate: f64,
    bpm: f64,
    playing: bool,
    /// Samples elapsed since the transport started.
    position: u64,
    /// Index of the next beat to be reached.
    next_beat: u64,
}

pub const MIN_BPM: f64 = 60.0;
pub const MAX_BPM: f64 = 200.0;

impl Clock {
    #[must_use]
    pub fn new(sample_rate: u32, bpm: f64) -> Self {
        Self {
            sample_rate: f64::from(sample_rate.max(1)),
            bpm: bpm.clamp(MIN_BPM, MAX_BPM),
            playing: false,
            position: 0,
            next_beat: 0,
        }
    }

    #[must_use]
    pub fn samples_per_beat(&self) -> f64 {
        self.sample_rate * 60.0 / self.bpm
    }

    #[must_use]
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Current position in beats (for display).
    #[must_use]
    pub fn beats(&self) -> f64 {
        self.position as f64 / self.samples_per_beat()
    }

    /// Starts from the top: the first beat falls on the next processed sample.
    pub fn start(&mut self) {
        self.playing = true;
        self.position = 0;
        self.next_beat = 0;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    fn beat_sample(&self, beat: u64) -> u64 {
        (beat as f64 * self.samples_per_beat()).round() as u64
    }

    /// Offset (in samples, within the next `frames`) and index of the next beat, if it falls inside.
    #[must_use]
    pub fn next_beat_within(&self, frames: usize) -> Option<(usize, u64)> {
        if !self.playing {
            return None;
        }
        let at = self.beat_sample(self.next_beat);
        let offset = usize::try_from(at.saturating_sub(self.position)).ok()?;
        (offset < frames).then_some((offset, self.next_beat))
    }

    /// Marks the beat returned by `next_beat_within` as handled, once its sample is reached.
    pub fn consume_beat(&mut self) {
        self.next_beat += 1;
    }

    /// Moves the clock forward by `frames` samples, consuming beats passed on the way.
    pub fn advance(&mut self, frames: usize) {
        if !self.playing {
            return;
        }
        self.position += frames as u64;
        while self.beat_sample(self.next_beat) < self.position {
            self.next_beat += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_land_on_exact_samples() {
        let mut clock = Clock::new(48_000, 120.0);
        clock.start();
        assert_eq!(clock.next_beat_within(128), Some((0, 0)));
        clock.advance(128);
        // 120 BPM at 48 kHz: one beat every 24 000 samples.
        assert_eq!(clock.next_beat_within(23_872), None);
        assert_eq!(clock.next_beat_within(23_873), Some((23_872, 1)));
    }

    #[test]
    fn no_drift_with_fractional_beat_length() {
        // 48 000 * 60 / 133 is not an integer: beat 1000 must still be at round(1000 * spb).
        let mut clock = Clock::new(48_000, 133.0);
        clock.start();
        let spb = 48_000.0 * 60.0 / 133.0;
        for _ in 0..(1000 * 22) {
            clock.advance(1000);
        }
        let expected_next = (clock.position() as f64 / spb).ceil() as u64;
        let (offset, beat) = clock.next_beat_within(30_000).unwrap_or_default();
        assert_eq!(beat, expected_next);
        assert_eq!(
            clock.position() + offset as u64,
            (expected_next as f64 * spb).round() as u64
        );
    }

    #[test]
    fn stopped_clock_does_not_move() {
        let mut clock = Clock::new(48_000, 120.0);
        clock.advance(1000);
        assert_eq!(clock.position(), 0);
        assert_eq!(clock.next_beat_within(100_000), None);
    }

    #[test]
    fn tempo_is_clamped() {
        assert_eq!(Clock::new(48_000, 10.0).bpm(), MIN_BPM);
        assert_eq!(Clock::new(48_000, 500.0).bpm(), MAX_BPM);
    }
}
