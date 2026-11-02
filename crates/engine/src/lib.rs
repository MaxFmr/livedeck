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

pub mod bass;
pub mod clock;
pub mod drums;
pub mod dsp;
pub mod limiter;
pub mod link;
pub mod metronome;
pub mod repeat;
pub mod sequencer;
pub mod smooth;
pub mod style;

use bass::BassVoice;
use clock::Clock;
use drums::{DrumKit, DrumVoice};
use limiter::Limiter;
use metronome::Click;
use repeat::{BeatRepeat, Division};
use sequencer::Sequencer;
use smooth::Smoothed;

/// Continuous parameters, normalized 0..=1 (curves are applied by the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    MasterGain,
    MetronomeLevel,
    /// Level of the rhythm deck (drums and bass).
    DrumLevel,
    /// Base cutoff of the bass filter (linear knob, exponential in Hz).
    BassCutoff,
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
    bass: BassVoice,
    sequencer: Sequencer,
    limiter: Limiter,
    metronome: bool,
    master_gain: Smoothed,
    metronome_level: Smoothed,
    drum_level: Smoothed,
    repeat: BeatRepeat,
    /// Clock position of the last drum hit (sequencer or pad) since the transport started.
    last_hit: Option<u64>,
    /// Length of a roll waiting for the next hit (no hit in the current division at the press).
    roll_pending: Option<usize>,
    /// Mono mix bus, allocated once; blocks larger than this are processed in slices.
    mono: Vec<f32>,
    /// Drum bus (before the drum level), same size as `mono`.
    drum_bus: Vec<f32>,
    /// Bass bus (kept out of the beat-repeat), same size as `mono`.
    bass_bus: Vec<f32>,
}

impl Engine {
    /// Allocates everything the audio thread will ever need. Call before starting the stream.
    #[must_use]
    pub fn new(sample_rate: u32, max_block: usize, bpm: f64) -> Self {
        Self {
            clock: Clock::new(sample_rate, bpm),
            click: Click::new(sample_rate),
            drums: DrumKit::new(sample_rate),
            bass: BassVoice::new(sample_rate),
            sequencer: Sequencer::new(),
            limiter: Limiter::new(sample_rate),
            metronome: true,
            master_gain: Smoothed::new(1.0, sample_rate, 0.01),
            metronome_level: Smoothed::new(gain_curve(0.7), sample_rate, 0.01),
            drum_level: Smoothed::new(gain_curve(0.8), sample_rate, 0.01),
            // One beat at the slowest tempo: the longest division repeated.
            repeat: BeatRepeat::new(
                (f64::from(sample_rate) * 60.0 / clock::MIN_BPM).ceil() as usize
            ),
            last_hit: None,
            roll_pending: None,
            mono: vec![0.0; max_block.max(64)],
            drum_bus: vec![0.0; max_block.max(64)],
            bass_bus: vec![0.0; max_block.max(64)],
        }
    }

    pub fn start(&mut self) {
        self.clock.start();
        self.sequencer.reset();
        self.repeat.release();
        self.last_hit = None;
        self.roll_pending = None;
    }

    pub fn stop(&mut self) {
        self.clock.stop();
        self.bass.note_off();
        self.repeat.release();
        self.roll_pending = None;
    }

    pub fn set_tempo(&mut self, bpm: f64) {
        self.clock.set_bpm(bpm);
    }

    pub fn set_metronome(&mut self, on: bool) {
        self.metronome = on;
    }

    /// Sets a normalized (0..=1) parameter; it glides there in ~10 ms.
    pub fn set_param(&mut self, param: Param, value: f32) {
        match param {
            Param::MasterGain => self.master_gain.set(gain_curve(value)),
            Param::MetronomeLevel => self.metronome_level.set(gain_curve(value)),
            Param::DrumLevel => self.drum_level.set(gain_curve(value)),
            Param::BassCutoff => self.bass.set_cutoff(value),
        }
    }

    /// Plays a drum voice from the next rendered sample, playing or not.
    pub fn trigger_drum(&mut self, voice: DrumVoice, velocity: f32) {
        self.drums.trigger(voice, velocity);
        if self.clock.is_playing() {
            let now = self.clock.position();
            self.last_hit = Some(now);
            if let Some(len) = self.roll_pending.take() {
                self.repeat.engage(now, len, now + len as u64);
            }
        }
    }

    pub fn drums_mut(&mut self) -> &mut DrumKit {
        &mut self.drums
    }

    /// Queues drum pattern `index` (0-based) for the next bar (now if stopped).
    pub fn queue_pattern(&mut self, index: u8) {
        self.sequencer.queue(index, self.clock.is_playing());
    }

    /// Style `index` (see `style::presets`) for the next bar, or now if stopped.
    pub fn queue_style(&mut self, index: u8) {
        self.sequencer.queue_style(index);
        if !self.clock.is_playing() {
            self.sequencer.apply_style(&mut self.drums, &mut self.bass);
        }
    }

    /// Bass line `index` (0-based) alone for the next bar (now if stopped); `queue_pattern` also
    /// brings the line of the same index.
    pub fn queue_bass_line(&mut self, index: u8) {
        let playing = self.clock.is_playing();
        self.sequencer.bass_mut().queue(index, playing);
    }

    /// Bass tonic (0 = C … 11 = B) for the next bar (now if stopped).
    pub fn queue_tonic(&mut self, tonic: u8) {
        let playing = self.clock.is_playing();
        self.sequencer.bass_mut().queue_tonic(tonic, playing);
    }

    /// Bass mute: closes the note sounding from the next step.
    pub fn set_bass_mute(&mut self, muted: bool) {
        self.sequencer.bass_mut().set_muted(muted);
    }

    /// Fill `index` (0-based) from the next beat to the end of the bar; ignored when stopped.
    pub fn queue_fill(&mut self, index: u8) {
        self.sequencer.queue_fill(index, self.clock.is_playing());
    }

    /// Muted drum voices (bit `i` = `DrumVoice::ALL[i]`), from the next step.
    pub fn set_mutes(&mut self, mutes: u8) {
        self.sequencer.set_mutes(mutes);
    }

    /// Drum density, 0 (kicks on the beat) ..= 1 (every step).
    pub fn set_density(&mut self, density: f32) {
        self.sequencer.set_density(density);
    }

    /// A Loop pad is pressed: a shorter division than the loop heard subdivides it; otherwise a
    /// new slice of `division` of the groove, from the last hit, loops from its next boundary.
    /// `None` (every pad released) goes back to the groove. Ignored when stopped.
    pub fn beat_repeat(&mut self, division: Option<Division>) {
        self.roll_pending = None;
        let Some(division) = division.filter(|_| self.clock.is_playing()) else {
            self.repeat.release();
            return;
        };
        let len = (division.beats() * self.clock.samples_per_beat())
            .round()
            .max(1.0) as u64;
        // Shorter than the loop heard: subdivide it (roll what is heard).
        if self.repeat.subdivide(len as usize) {
            return;
        }
        let now = self.clock.position();
        // Roll the last hit if it falls within one division before the press: the roll follows
        // what is being played. An older hit would replay the past (heard in the 012 test, at low
        // density): then the roll waits for the next hit instead.
        match self.last_hit.filter(|&hit| now.saturating_sub(hit) < len) {
            Some(hit) => self.repeat.engage(hit, len as usize, hit + len),
            None => {
                self.repeat.cancel_pending();
                self.roll_pending = Some(len as usize);
            }
        }
    }

    /// The Loop pad of `released` is let go while the one of `back` is still held: back to the
    /// loop of `back` (in phase if it is kept, else a new slice).
    pub fn beat_repeat_back(&mut self, released: Division, back: Division) {
        self.roll_pending = None;
        if !self.clock.is_playing() {
            self.repeat.release();
            return;
        }
        let len = |d: Division| (d.beats() * self.clock.samples_per_beat()).round() as usize;
        if !self.repeat.go_back(len(released), len(back)) {
            self.beat_repeat(Some(back));
        }
    }

    #[must_use]
    pub fn beat_repeat_engaged(&self) -> bool {
        self.repeat.engaged() || self.roll_pending.is_some()
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
            let (Some(mono), Some(bus), Some(bass)) = (
                self.mono.get_mut(..n),
                self.drum_bus.get_mut(..n),
                self.bass_bus.get_mut(..n),
            ) else {
                break;
            };
            mono.fill(0.0);
            bus.fill(0.0);
            bass.fill(0.0);
            self.click.gain = self.metronome_level.skip(n);
            let drum_gain = self.drum_level.skip(n);
            let start = self.clock.position();
            render_mono(
                Voices {
                    clock: &mut self.clock,
                    click: &mut self.click,
                    metronome: self.metronome,
                    sequencer: &mut self.sequencer,
                    drums: &mut self.drums,
                    bass: &mut self.bass,
                    last_hit: &mut self.last_hit,
                    roll_pending: &mut self.roll_pending,
                    repeat: &mut self.repeat,
                },
                Buses {
                    mono,
                    drums: bus,
                    bass,
                },
            );
            self.repeat.process(bus, start);
            for ((sample, &drum), &bass) in mono.iter_mut().zip(bus.iter()).zip(bass.iter()) {
                *sample += (drum + bass) * drum_gain;
            }
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
    bass: &'a mut BassVoice,
    last_hit: &'a mut Option<u64>,
    roll_pending: &'a mut Option<usize>,
    repeat: &'a mut BeatRepeat,
}

/// Buffers of one slice: click, drums, bass (same length).
struct Buses<'a> {
    mono: &'a mut [f32],
    drums: &'a mut [f32],
    bass: &'a mut [f32],
}

/// Renders one slice (click into `mono`, drums and bass into their buses), splitting it at beats
/// and sequencer steps so every click, drum hit and bass note starts on its exact sample.
fn render_mono(v: Voices<'_>, buses: Buses<'_>) {
    let mut rest = buses.mono;
    let mut bus_rest = buses.drums;
    let mut bass_rest = buses.bass;
    while !rest.is_empty() {
        let len = rest.len();
        let beat = v.clock.next_beat_within(len);
        let step = v.sequencer.next_step_within(v.clock, len);
        let beat_at = beat.map_or(len, |(offset, _)| offset);
        let step_at = step.unwrap_or(len);
        // Both offsets are < len by construction; clamped anyway so the split can never panic.
        let segment_len = beat_at.min(step_at).min(len);
        let (segment, tail) = rest.split_at_mut(segment_len);
        let (bus_segment, bus_tail) = bus_rest.split_at_mut(segment_len.min(bus_rest.len()));
        let (bass_segment, bass_tail) = bass_rest.split_at_mut(segment_len.min(bass_rest.len()));
        v.click.render_add(segment);
        v.drums.render_add(bus_segment, 1.0);
        v.bass.render_add(bass_segment);
        v.clock.advance(segment_len);
        if let Some((_, beat)) = beat
            && beat_at == segment_len
        {
            if v.metronome {
                v.click.trigger(beat % BEATS_PER_BAR == 0);
            }
            v.clock.consume_beat();
        }
        if step_at == segment_len
            && step.is_some()
            && v.sequencer.fire(v.drums, v.bass, v.clock) != 0
        {
            let hit = v.clock.position();
            *v.last_hit = Some(hit);
            if let Some(len) = v.roll_pending.take() {
                v.repeat.engage(hit, len, hit + len as u64);
            }
        }
        rest = tail;
        bus_rest = bus_tail;
        bass_rest = bass_tail;
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
    fn roll_waits_for_the_next_hit_when_the_last_one_is_old() {
        // Density 0 on pattern A: kicks on beats 1 and 3 only (samples 0 and 48 000).
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.set_metronome(false);
        engine.set_param(Param::MasterGain, 0.5);
        engine.set_density(0.0);
        engine.set_bass_mute(true);
        engine.start();
        render(&mut engine, 24_000, 1, 128);
        // Pressed on beat 2: the kick of beat 1 is a whole beat old, longer than a 1/16.
        engine.beat_repeat(Division::from_denominator(16));
        assert!(engine.beat_repeat_engaged());
        let out = render(&mut engine, 48_000, 1, 128);
        // Live until the kick of beat 3 (24 000 samples on), then that kick rolls every 6 000.
        assert!(
            out[..23_000].iter().all(|s| s.abs() < 0.05),
            "no old kick replayed"
        );
        let turn = |k: usize| &out[24_000 + k * 6_000 + 500..24_000 + (k + 1) * 6_000 - 1_000];
        assert!(turn(2).iter().any(|s| s.abs() > 0.1));
        let diff = turn(2)
            .iter()
            .zip(turn(3))
            .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(diff < 1.0e-3, "{diff}");
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
