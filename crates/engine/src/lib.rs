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

pub mod clock;
pub mod drums;
pub mod dsp;
pub mod limiter;
pub mod link;
pub mod metronome;
pub mod sequencer;
pub mod smooth;

use clock::Clock;
use drums::{DrumKit, DrumVoice};
use limiter::Limiter;
use metronome::Click;
use sequencer::Sequencer;
use smooth::Smoothed;

/// Continuous parameters, normalized 0..=1 (curves are applied by the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    MasterGain,
    MetronomeLevel,
    DrumLevel,
}

/// Beats per bar (4/4 only for now).
pub const BEATS_PER_BAR: u64 = 4;

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

pub struct Engine {
    clock: Clock,
    click: Click,
    drums: DrumKit,
    sequencer: Sequencer,
    limiter: Limiter,
    metronome: bool,
    master_gain: Smoothed,
    metronome_level: Smoothed,
    drum_level: Smoothed,
    /// Mono mix bus, allocated once; blocks larger than this are processed in slices.
    mono: Vec<f32>,
}

impl Engine {
    /// Allocates everything the audio thread will ever need. Call before starting the stream.
    #[must_use]
    pub fn new(sample_rate: u32, max_block: usize, bpm: f64) -> Self {
        Self {
            clock: Clock::new(sample_rate, bpm),
            click: Click::new(sample_rate),
            drums: DrumKit::new(sample_rate),
            sequencer: Sequencer::new(),
            limiter: Limiter::new(sample_rate),
            metronome: true,
            master_gain: Smoothed::new(1.0, sample_rate, 0.01),
            metronome_level: Smoothed::new(gain_curve(0.7), sample_rate, 0.01),
            drum_level: Smoothed::new(gain_curve(0.8), sample_rate, 0.01),
            mono: vec![0.0; max_block.max(64)],
        }
    }

    pub fn start(&mut self) {
        self.clock.start();
        self.sequencer.reset();
    }

    pub fn stop(&mut self) {
        self.clock.stop();
    }

    pub fn set_tempo(&mut self, bpm: f64) {
        self.clock.set_bpm(bpm);
    }

    pub fn set_metronome(&mut self, on: bool) {
        self.metronome = on;
    }

    /// Sets a normalized (0..=1) parameter; it glides there in ~10 ms.
    pub fn set_param(&mut self, param: Param, value: f32) {
        let value = gain_curve(value);
        match param {
            Param::MasterGain => self.master_gain.set(value),
            Param::MetronomeLevel => self.metronome_level.set(value),
            Param::DrumLevel => self.drum_level.set(value),
        }
    }

    /// Plays a drum voice from the next rendered sample, playing or not.
    pub fn trigger_drum(&mut self, voice: DrumVoice, velocity: f32) {
        self.drums.trigger(voice, velocity);
    }

    pub fn drums_mut(&mut self) -> &mut DrumKit {
        &mut self.drums
    }

    /// Queues drum pattern `index` (0-based) for the next bar (now if stopped).
    pub fn queue_pattern(&mut self, index: u8) {
        self.sequencer.queue(index, self.clock.is_playing());
    }

    /// Drum sequencer on/off (the bar position keeps running).
    pub fn set_sequencer(&mut self, on: bool) {
        self.sequencer.set_enabled(on);
    }

    /// Swing of the odd 16ths, 0.5 (straight) ..= 0.75.
    pub fn set_swing(&mut self, swing: f32) {
        self.sequencer.set_swing(swing);
    }

    #[must_use]
    pub fn sequencer(&self) -> &Sequencer {
        &self.sequencer
    }

    #[must_use]
    pub fn metronome_enabled(&self) -> bool {
        self.metronome
    }

    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Non-finite samples caught by the master guard.
    #[must_use]
    pub fn nan_count(&self) -> u32 {
        self.limiter.nan_count
    }

    /// Renders into an interleaved buffer: the mix goes to channels 0 and 1, the others get silence.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let mut frames_out = out.chunks_exact_mut(channels);
        loop {
            let remaining = frames_out.len();
            if remaining == 0 {
                break;
            }
            let n = remaining.min(self.mono.len());
            let Some(mono) = self.mono.get_mut(..n) else {
                break;
            };
            mono.fill(0.0);
            self.click.gain = self.metronome_level.skip(n);
            let drum_gain = self.drum_level.skip(n);
            render_mono(
                Voices {
                    clock: &mut self.clock,
                    click: &mut self.click,
                    metronome: self.metronome,
                    sequencer: &mut self.sequencer,
                    drums: &mut self.drums,
                    drum_gain,
                },
                mono,
            );
            for sample in mono.iter_mut() {
                *sample *= self.master_gain.tick();
            }
            self.limiter.process(mono);
            for (frame, &sample) in frames_out.by_ref().take(n).zip(mono.iter()) {
                for (ch, slot) in frame.iter_mut().enumerate() {
                    *slot = if ch < 2 { sample } else { 0.0 };
                }
            }
        }
    }
}

/// Perceptual fader law: 0..=1 → amplitude (x², so the middle of the travel is about -12 dB).
fn gain_curve(x: f32) -> f32 {
    let x = if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    };
    x * x
}

/// What one slice renders (borrowed from the engine, which also owns the mix bus).
struct Voices<'a> {
    clock: &'a mut Clock,
    click: &'a mut Click,
    metronome: bool,
    sequencer: &'a mut Sequencer,
    drums: &'a mut DrumKit,
    drum_gain: f32,
}

/// Renders one slice, splitting it at beats and sequencer steps so every click and drum hit starts
/// on its exact sample.
fn render_mono(v: Voices<'_>, mono: &mut [f32]) {
    let mut rest = mono;
    while !rest.is_empty() {
        let len = rest.len();
        let beat = v.clock.next_beat_within(len);
        let step = v.sequencer.next_step_within(v.clock, len);
        let beat_at = beat.map_or(len, |(offset, _)| offset);
        let step_at = step.unwrap_or(len);
        // Both offsets are < len by construction; clamped anyway so the split can never panic.
        let segment_len = beat_at.min(step_at).min(len);
        let (segment, tail) = rest.split_at_mut(segment_len);
        v.click.render_add(segment);
        v.drums.render_add(segment, v.drum_gain);
        v.clock.advance(segment_len);
        if let Some((_, beat)) = beat
            && beat_at == segment_len
        {
            if v.metronome {
                v.click.trigger(beat % BEATS_PER_BAR == 0);
            }
            v.clock.consume_beat();
        }
        if step_at == segment_len && step.is_some() {
            v.sequencer.fire(v.drums);
        }
        rest = tail;
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)] // tests may panic: that is how they fail
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn render(engine: &mut Engine, frames: usize, channels: usize, block: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * channels];
        for chunk in out.chunks_mut(block * channels) {
            engine.process(chunk, channels);
        }
        out
    }

    /// Indices of samples where a click starts (silence before, sound after).
    fn onsets(mono: &[f32]) -> Vec<usize> {
        mono.windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] == 0.0 && w[1] != 0.0)
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn block_budget_matches_reference_buffer() {
        // 128 frames @ 48 kHz is the reference buffer of the project.
        assert!((block_budget_us(128, 48_000) - 2_666.666).abs() < 0.01);
        assert_eq!(block_budget_us(128, 0), 0.0);
    }

    #[test]
    fn clicks_fall_on_beats_whatever_the_block_size() {
        for block in [64, 128, 333, 512] {
            let mut engine = Engine::new(SR, 256, 120.0);
            engine.set_sequencer(false);
            engine.start();
            let out = render(&mut engine, 3 * 24_000 + 100, 1, block);
            // An onset at i means sample i is silent and i + 1 sounds: a click starting on beat
            // sample b begins with sin(0) = 0, so it is reported at b.
            assert_eq!(
                onsets(&out),
                vec![0, 24_000, 48_000, 72_000],
                "block {block}"
            );
        }
    }

    #[test]
    fn output_is_finite_and_below_ceiling() {
        let mut engine = Engine::new(SR, 128, 180.0);
        engine.start();
        let out = render(&mut engine, SR as usize * 2, 2, 128);
        assert!(
            out.iter()
                .all(|s| s.is_finite() && s.abs() <= limiter::CEILING)
        );
        assert!(
            out.iter().any(|s| s.abs() > 0.1),
            "metronome should be audible"
        );
        assert_eq!(engine.nan_count(), 0);
    }

    #[test]
    fn stopped_engine_is_silent() {
        let mut engine = Engine::new(SR, 128, 120.0);
        let out = render(&mut engine, 10_000, 2, 128);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn drums_sound_at_block_start_even_when_stopped() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.trigger_drum(DrumVoice::Kick, 1.0);
        let out = render(&mut engine, 128, 1, 128);
        assert!(out[0] != 0.0, "the kick click starts on the first sample");
        assert!(out.iter().any(|s| s.abs() > 0.3));
    }

    #[test]
    fn mix_goes_to_first_two_channels_only() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.start();
        let out = render(&mut engine, 2_000, 4, 128);
        for frame in out.as_chunks::<4>().0 {
            assert_eq!(frame[0], frame[1]);
            assert_eq!(frame[2], 0.0);
            assert_eq!(frame[3], 0.0);
        }
    }
}
