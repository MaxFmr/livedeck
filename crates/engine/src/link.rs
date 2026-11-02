//! Lock-free link between the audio thread and the control thread.
//!
//! Control → audio: `Command` through an SPSC ring buffer (one producer: the control thread).
//! Audio → control: `EngineEvent` through another ring buffer, and the latest `EngineSnapshot`
//! through a triple buffer (the reader never blocks the writer).
//!
//! Runs on: `AudioEngine` on the audio thread, `EngineHandle` on the control thread.

use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output, triple_buffer};

use crate::drums::DrumVoice;
use crate::repeat::Division;
use crate::{BEATS_PER_BAR, Engine, Param};

/// Everything the audio thread can be asked to do. Small and `Copy`: no heap, no drop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Start,
    Stop,
    /// Absolute tempo, clamped to the clock range.
    SetTempo(f64),
    Metronome(bool),
    /// Normalized 0..=1 value of a continuous parameter.
    SetParam(Param, f32),
    /// Drum pattern (0-based) for the next bar, or now if stopped.
    QueuePattern(u8),
    /// Style (see `style::presets`) for the next bar, or now if stopped.
    QueueStyle(u8),
    /// Drum source for the next bar (now if stopped): samples (true) or synth.
    UseSamples(bool),
    /// Chord pad (0..4 = I, IV, V, vi) pressed or released: played now.
    Chord {
        pad: u8,
        on: bool,
    },
    /// Bass line (0-based) alone for the next bar, or now if stopped.
    QueueBassLine(u8),
    /// Bass tonic (0 = C … 11 = B) for the next bar, or now if stopped.
    SetTonic(u8),
    BassMute(bool),
    /// Swing of the odd 16ths, 0.5..=0.75.
    SetSwing(f32),
    /// Drum sequencer on/off.
    Sequencer(bool),
    /// Muted drum voices, bit `i` = `DrumVoice::ALL[i]`.
    SetMutes(u8),
    /// Drum density 0..=1.
    SetDensity(f32),
    /// Drum fill (0-based) from the next beat to the end of the bar.
    Fill(u8),
    /// Beat-repeat of the drum bus: a Loop pad pressed; `None` = every pad released.
    BeatRepeat(Option<Division>),
    /// The Loop pad of `released` is let go, the one of `back` is still held.
    BeatRepeatBack {
        released: Division,
        back: Division,
    },
    /// Plays a drum voice now (velocity 0..=1). A non-zero `gesture_us` (host time of the
    /// gesture) comes back in `EngineEvent::Hit` to measure gesture → sound latency.
    Drum {
        voice: DrumVoice,
        velocity: f32,
        gesture_us: u64,
    },
}

const _: () = assert!(core::mem::size_of::<Command>() <= 32);

/// Things the audio thread reports, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineEvent {
    /// The last block played into a new beat (1-based bar and beat).
    Beat { bar: u64, beat: u8 },
    /// A `Command::Drum` with a gesture time started sounding at the first sample of the block
    /// stamped `time`.
    Hit { gesture_us: u64, time: BlockTime },
}

/// Host-time stamps of one audio block, in µs (0 when unknown, e.g. offline rendering). See
/// `io::audio::BlockStamp` for what each one means.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockTime {
    pub called_us: u64,
    pub callback_us: u64,
    pub playback_us: u64,
}

/// Latest engine state, written once per block.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EngineSnapshot {
    pub playing: bool,
    pub bpm: f64,
    /// Position in beats since start.
    pub beats: f64,
    pub metronome: bool,
    /// Non-finite samples caught by the master guard.
    pub nan_count: u32,
    /// Events dropped because the event queue was full.
    pub dropped_events: u32,
    /// Commands applied since start.
    pub commands: u64,
    /// Drum pattern playing (0-based) and the one waiting for the next bar.
    pub pattern: u8,
    pub pending_pattern: Option<u8>,
    pub swing: f32,
    /// Step (0..16) of the bar about to play.
    pub step: u8,
    pub mutes: u8,
    pub density: f32,
    /// Fill waiting or playing (0-based), and whether it has started.
    pub fill: Option<(u8, bool)>,
    pub beat_repeat: bool,
    /// Bass line playing (0-based) and the one waiting for the next bar.
    pub bass_line: u8,
    pub pending_bass_line: Option<u8>,
    pub bass_muted: bool,
    /// Tonic (0 = C … 11 = B) and the one waiting for the next bar.
    pub tonic: u8,
    pub pending_tonic: Option<u8>,
    /// Style playing and the one waiting for the next bar.
    pub style: u8,
    pub pending_style: Option<u8>,
    /// Drums played from samples, the source waiting for the bar, and whether a kit is loaded.
    pub samples: bool,
    pub pending_samples: Option<bool>,
    pub samples_loaded: bool,
    /// Melody synth voices sounding (out of `poly::VOICES`).
    pub synth_voices: u8,
}

pub const COMMAND_CAPACITY: usize = 256;
pub const EVENT_CAPACITY: usize = 256;

/// Audio-thread side: owns the engine.
pub struct AudioEngine {
    engine: Engine,
    commands: Consumer<Command>,
    events: Producer<EngineEvent>,
    snapshot: Input<EngineSnapshot>,
    last_beat: Option<u64>,
    applied: u64,
    dropped_events: u32,
}

/// Control-thread side.
pub struct EngineHandle {
    commands: Producer<Command>,
    events: Consumer<EngineEvent>,
    snapshot: Output<EngineSnapshot>,
}

/// Splits an engine (built and allocated on the control thread) into its two sides.
#[must_use]
pub fn connect(engine: Engine) -> (AudioEngine, EngineHandle) {
    let (cmd_tx, cmd_rx) = RingBuffer::new(COMMAND_CAPACITY);
    let (evt_tx, evt_rx) = RingBuffer::new(EVENT_CAPACITY);
    let initial = snapshot_of(&engine, 0, 0);
    let (snap_in, snap_out) = triple_buffer(&initial);
    (
        AudioEngine {
            engine,
            commands: cmd_rx,
            events: evt_tx,
            snapshot: snap_in,
            last_beat: None,
            applied: 0,
            dropped_events: 0,
        },
        EngineHandle {
            commands: cmd_tx,
            events: evt_rx,
            snapshot: snap_out,
        },
    )
}

fn snapshot_of(engine: &Engine, applied: u64, dropped_events: u32) -> EngineSnapshot {
    let clock = engine.clock();
    EngineSnapshot {
        playing: clock.is_playing(),
        bpm: clock.bpm(),
        beats: clock.beats(),
        metronome: engine.metronome_enabled(),
        nan_count: engine.nan_count(),
        dropped_events,
        commands: applied,
        pattern: engine.sequencer().current(),
        pending_pattern: engine.sequencer().pending(),
        swing: engine.sequencer().swing(),
        step: engine.sequencer().step_in_bar(),
        mutes: engine.sequencer().mutes(),
        density: engine.sequencer().density(),
        fill: engine.sequencer().fill(),
        beat_repeat: engine.beat_repeat_engaged(),
        bass_line: engine.sequencer().bass().current(),
        pending_bass_line: engine.sequencer().bass().pending(),
        bass_muted: engine.sequencer().bass().muted(),
        tonic: engine.sequencer().bass().key().tonic,
        pending_tonic: engine.sequencer().bass().pending_tonic(),
        style: engine.sequencer().style(),
        pending_style: engine.sequencer().pending_style(),
        samples: engine.drums().use_samples(),
        pending_samples: engine.sequencer().pending_samples(),
        samples_loaded: engine.drums().has_samples(),
        synth_voices: engine.synth().active_voices() as u8,
    }
}

impl AudioEngine {
    /// The engine, read-only (diagnostics from the audio callback).
    #[must_use]
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Audio callback body without timing information (offline rendering, tests).
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        self.process_at(out, channels, BlockTime::default());
    }

    /// Audio callback body: applies pending commands, renders, publishes state.
    pub fn process_at(&mut self, out: &mut [f32], channels: usize, time: BlockTime) {
        // Bounded even if the control thread keeps pushing while we drain.
        for _ in 0..COMMAND_CAPACITY {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            self.apply(command, time);
        }
        self.engine.process(out, channels);
        self.report_beat();
        self.snapshot
            .write(snapshot_of(&self.engine, self.applied, self.dropped_events));
    }

    fn apply(&mut self, command: Command, time: BlockTime) {
        self.applied += 1;
        match command {
            Command::Start => {
                self.engine.start();
                self.last_beat = None;
            }
            Command::Stop => self.engine.stop(),
            Command::SetTempo(bpm) => self.engine.set_tempo(bpm),
            Command::Metronome(on) => self.engine.set_metronome(on),
            Command::QueuePattern(index) => self.engine.queue_pattern(index),
            Command::QueueStyle(index) => self.engine.queue_style(index),
            Command::UseSamples(on) => self.engine.use_samples(on),
            Command::Chord { pad, on } => self.engine.chord(pad, on),
            Command::QueueBassLine(index) => self.engine.queue_bass_line(index),
            Command::SetTonic(tonic) => self.engine.queue_tonic(tonic),
            Command::BassMute(muted) => self.engine.set_bass_mute(muted),
            Command::SetSwing(swing) => self.engine.set_swing(swing),
            Command::Sequencer(on) => self.engine.set_sequencer(on),
            Command::SetMutes(mutes) => self.engine.set_mutes(mutes),
            Command::SetDensity(density) => self.engine.set_density(density),
            Command::Fill(index) => self.engine.queue_fill(index),
            Command::BeatRepeat(division) => self.engine.beat_repeat(division),
            Command::BeatRepeatBack { released, back } => {
                self.engine.beat_repeat_back(released, back);
            }
            Command::SetParam(param, value) => self.engine.set_param(param, value),
            Command::Drum {
                voice,
                velocity,
                gesture_us,
            } => {
                self.engine.trigger_drum(voice, velocity);
                if gesture_us != 0 {
                    self.push_event(EngineEvent::Hit { gesture_us, time });
                }
            }
        }
    }

    fn report_beat(&mut self) {
        let clock = self.engine.clock();
        if !clock.is_playing() {
            return;
        }
        let Some(beat) = clock.last_sample_beat() else {
            return;
        };
        if self.last_beat == Some(beat) {
            return;
        }
        self.last_beat = Some(beat);
        let event = EngineEvent::Beat {
            bar: beat / BEATS_PER_BAR + 1,
            beat: (beat % BEATS_PER_BAR) as u8 + 1,
        };
        self.push_event(event);
    }

    fn push_event(&mut self, event: EngineEvent) {
        if self.events.push(event).is_err() {
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
    }
}

impl EngineHandle {
    /// Queues a command for the next audio block. Returns false if the queue is full (the
    /// command is dropped: the control thread decides whether to retry).
    pub fn send(&mut self, command: Command) -> bool {
        self.commands.push(command).is_ok()
    }

    /// Next pending event, if any.
    pub fn next_event(&mut self) -> Option<EngineEvent> {
        self.events.pop().ok()
    }

    /// Latest published state.
    pub fn snapshot(&mut self) -> EngineSnapshot {
        *self.snapshot.read()
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pair() -> (AudioEngine, EngineHandle) {
        connect(Engine::new(48_000, 256, 120.0))
    }

    fn run(audio: &mut AudioEngine, blocks: usize) {
        let mut buf = [0.0_f32; 256];
        for _ in 0..blocks {
            audio.process(&mut buf, 2);
        }
    }

    #[test]
    fn commands_apply_on_next_block() {
        let (mut audio, mut handle) = pair();
        assert!(!handle.snapshot().playing);
        assert!(handle.send(Command::Start));
        assert!(handle.send(Command::SetTempo(90.0)));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert!(snap.playing);
        assert_eq!(snap.bpm, 90.0);
        assert_eq!(snap.commands, 2);
        handle.send(Command::Stop);
        run(&mut audio, 1);
        assert!(!handle.snapshot().playing);
    }

    #[test]
    fn beats_are_reported_once_each() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        // 120 BPM: 2 beats per second; 1 s = 375 blocks of 128 frames.
        run(&mut audio, 375);
        let mut beats = Vec::new();
        while let Some(event) = handle.next_event() {
            if let EngineEvent::Beat { bar, beat } = event {
                beats.push((bar, beat));
            }
        }
        assert_eq!(beats, vec![(1, 1), (1, 2)]);
    }

    #[test]
    fn full_queues_never_panic() {
        let (mut audio, mut handle) = pair();
        let accepted = (0..COMMAND_CAPACITY * 2)
            .filter(|_| handle.send(Command::Metronome(true)))
            .count();
        assert_eq!(accepted, COMMAND_CAPACITY);
        run(&mut audio, 1); // drains the command queue
        assert!(handle.send(Command::Start));
        // Nobody drains events: they must be dropped and counted, not block or panic.
        run(&mut audio, 375 * 300);
        assert!(handle.snapshot().dropped_events > 0);
    }

    #[test]
    fn master_gain_glides_to_silence() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        handle.send(Command::SetParam(Param::MasterGain, 0.0));
        let mut buf = [0.0_f32; 256];
        // ~10 ms time constant: after 100 ms the output is silent.
        run(&mut audio, 38);
        audio.process(&mut buf, 2);
        assert!(buf.iter().all(|s| s.abs() < 1.0e-4));
    }

    #[test]
    fn hit_reports_the_block_times() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Drum {
            voice: DrumVoice::Snare,
            velocity: 1.0,
            gesture_us: 1_000,
        });
        let mut buf = [0.0_f32; 256];
        let time = BlockTime {
            called_us: 1_500,
            callback_us: 4_000,
            playback_us: 9_000,
        };
        audio.process_at(&mut buf, 2, time);
        assert_eq!(
            handle.next_event(),
            Some(EngineEvent::Hit {
                gesture_us: 1_000,
                time
            })
        );
        assert!(buf.iter().any(|s| s.abs() > 0.1), "the hit is audible");
    }

    #[test]
    fn pattern_change_is_published_then_applied_at_the_bar() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        run(&mut audio, 10);
        handle.send(Command::QueuePattern(2));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert_eq!((snap.pattern, snap.pending_pattern), (0, Some(2)));
        // One bar at 120 BPM = 96 000 samples = 750 blocks of 128 frames.
        run(&mut audio, 750);
        let snap = handle.snapshot();
        assert_eq!((snap.pattern, snap.pending_pattern), (2, None));
    }

    #[test]
    fn fill_mutes_and_repeat_are_published() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        handle.send(Command::SetMutes(0b10));
        handle.send(Command::SetDensity(0.3));
        run(&mut audio, 10);
        handle.send(Command::Fill(1));
        handle.send(Command::BeatRepeat(Some(Division::Eighth)));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert_eq!((snap.mutes, snap.density), (0b10, 0.3));
        assert_eq!(snap.fill, Some((1, false)));
        assert!(snap.beat_repeat);
        handle.send(Command::BeatRepeat(None));
        run(&mut audio, 1);
        assert!(!handle.snapshot().beat_repeat);
    }

    #[test]
    fn bass_changes_are_published_then_applied_at_the_bar() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        run(&mut audio, 10);
        handle.send(Command::QueueBassLine(3));
        handle.send(Command::SetTonic(2));
        handle.send(Command::BassMute(true));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert_eq!((snap.bass_line, snap.pending_bass_line), (0, Some(3)));
        assert_eq!((snap.tonic, snap.pending_tonic), (9, Some(2)));
        assert!(snap.bass_muted);
        // 120 BPM: a bar is 96 000 samples, 750 blocks of 128.
        run(&mut audio, 750);
        let snap = handle.snapshot();
        assert_eq!((snap.bass_line, snap.pending_bass_line), (3, None));
        assert_eq!((snap.tonic, snap.pending_tonic), (2, None));
    }

    #[test]
    fn style_change_waits_for_the_bar_and_brings_its_swing() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        run(&mut audio, 10);
        handle.send(Command::QueueStyle(3));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert_eq!((snap.style, snap.pending_style), (0, Some(3)));
        run(&mut audio, 750);
        let snap = handle.snapshot();
        assert_eq!((snap.style, snap.pending_style), (3, None));
        assert_eq!(snap.swing, crate::style::TEXTS[3].swing);
        assert_eq!(snap.bpm, 120.0, "the tempo stays the player's");
    }

    #[test]
    fn drum_source_switches_at_the_bar_only_with_a_kit() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::UseSamples(true));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert!(
            !snap.samples_loaded && !snap.samples,
            "no kit: stays on the synth"
        );

        let mut engine = Engine::new(48_000, 1024, 120.0);
        let mut bank = crate::sampler::SampleBank {
            sample_rate: 48_000,
            ..Default::default()
        };
        bank.voices[0] = vec![0.5; 1_000];
        engine.set_sample_bank(bank);
        let (mut audio, mut handle) = connect(engine);
        handle.send(Command::Start);
        run(&mut audio, 10);
        handle.send(Command::UseSamples(true));
        run(&mut audio, 1);
        let snap = handle.snapshot();
        assert_eq!((snap.samples, snap.pending_samples), (false, Some(true)));
        run(&mut audio, 750);
        let snap = handle.snapshot();
        assert_eq!((snap.samples, snap.pending_samples), (true, None));
    }

    #[test]
    fn chord_pads_hold_their_triad_and_styles_bring_their_patch() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Start);
        handle.send(Command::Chord { pad: 0, on: true });
        handle.send(Command::Chord { pad: 3, on: true });
        run(&mut audio, 10);
        assert_eq!(handle.snapshot().synth_voices, 6);
        handle.send(Command::Chord { pad: 0, on: false });
        run(&mut audio, 1_000);
        assert_eq!(handle.snapshot().synth_voices, 3, "vi still held");
        handle.send(Command::Stop);
        run(&mut audio, 1_000);
        assert_eq!(handle.snapshot().synth_voices, 0);
    }

    #[test]
    fn beat_repeat_loops_the_drum_bus() {
        // Metronome and bass off: only the drums sound. Repeat 1/16 from the start, held for a bar.
        let (mut audio, mut handle) = pair();
        handle.send(Command::Metronome(false));
        handle.send(Command::BassMute(true));
        // Quiet enough for the limiter to stay out of the way.
        handle.send(Command::SetParam(Param::MasterGain, 0.5));
        handle.send(Command::Start);
        handle.send(Command::BeatRepeat(Some(Division::Sixteenth)));
        let mut out: Vec<f32> = Vec::new();
        let mut buf = [0.0_f32; 256];
        for _ in 0..750 {
            audio.process(&mut buf, 2);
            out.extend(buf.iter().step_by(2).copied());
        }
        // 1/16 at 120 BPM = 6 000 samples: turns 2 and 3 are identical (the kick of step 0).
        let turn = |k: usize| &out[k * 6_000 + 100..(k + 1) * 6_000 - 100];
        assert!(turn(2).iter().any(|s| s.abs() > 0.02));
        let diff = turn(2)
            .iter()
            .zip(turn(3))
            .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(diff < 1.0e-4, "{diff}");
    }

    #[test]
    fn metronome_can_be_muted() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Metronome(false));
        handle.send(Command::Sequencer(false));
        handle.send(Command::Start);
        let mut buf = [0.0_f32; 256];
        audio.process(&mut buf, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
        assert!(!handle.snapshot().metronome);
    }
}
