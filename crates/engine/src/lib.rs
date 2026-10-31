//! LiveDeck audio engine: clock, sequencers, voices and mixing.
//!
//! Runs on: audio thread (everything reachable from `process`/`render`).
//! Pure Rust, no I/O, no dependency on cpal/midir/egui, so it can be tested offline.
//! Real-time rules: docs/rust/realtime-audio.md §1.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

/// Time budget of one audio block, in microseconds.
///
/// Used to express callback load as a fraction of the block (see `/perf-check`).
#[must_use]
pub fn block_budget_us(frames: u32, sample_rate: u32) -> f64 {
    if sample_rate == 0 {
        return 0.0;
    }
    f64::from(frames) * 1_000_000.0 / f64::from(sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_budget_matches_reference_buffer() {
        // 128 frames @ 48 kHz is the reference buffer of the project.
        assert!((block_budget_us(128, 48_000) - 2_666.666).abs() < 0.01);
        assert_eq!(block_budget_us(128, 0), 0.0);
    }
}
