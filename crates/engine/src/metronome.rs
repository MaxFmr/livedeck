//! Metronome click: short sine burst with exponential decay, accented on the downbeat.
//!
//! Runs on: audio thread.

use core::f32::consts::TAU;

#[derive(Debug, Clone, Copy)]
pub struct Click {
    sample_rate: f32,
    phase: f32,
    increment: f32,
    envelope: f32,
    /// Per-sample multiplier giving a ~30 ms decay to -60 dB.
    decay: f32,
    pub gain: f32,
}

const DECAY_SECONDS: f32 = 0.03;
/// Below this the click is considered finished (also avoids denormals).
const SILENCE: f32 = 1.0e-4;

impl Click {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self::with_decay(sample_rate, DECAY_SECONDS)
    }

    /// A burst that fades to -60 dB in `decay_seconds`.
    #[must_use]
    pub fn with_decay(sample_rate: u32, decay_seconds: f32) -> Self {
        let sample_rate = sample_rate.max(1) as f32;
        Self {
            sample_rate,
            phase: 0.0,
            increment: 0.0,
            envelope: 0.0,
            // exp(ln(1e-3) / (t * sr)) reaches -60 dB after t seconds.
            decay: (-6.907_755 / (decay_seconds.max(1.0e-3) * sample_rate)).exp(),
            gain: 0.5,
        }
    }

    /// Starts a click; `accent` is the first beat of the bar (higher pitch).
    pub fn trigger(&mut self, accent: bool) {
        self.trigger_at(if accent { 1_600.0 } else { 1_000.0 });
    }

    /// Starts a burst at `freq` Hz.
    pub fn trigger_at(&mut self, freq: f32) {
        self.increment = freq / self.sample_rate;
        self.phase = 0.0;
        self.envelope = 1.0;
    }

    /// Adds the click into a mono buffer.
    pub fn render_add(&mut self, out: &mut [f32]) {
        if self.envelope < SILENCE {
            self.envelope = 0.0;
            return;
        }
        for sample in out.iter_mut() {
            *sample += (self.phase * TAU).sin() * self.envelope * self.gain;
            self.phase += self.increment;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.envelope *= self.decay;
        }
    }
}
