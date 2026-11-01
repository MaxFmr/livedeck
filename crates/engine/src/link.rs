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
    /// Swing of the odd 16ths, 0.5..=0.75.
    SetSwing(f32),
    /// Drum sequencer on/off.
    Sequencer(bool),
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
    }
}

impl AudioEngine {
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
            Command::SetSwing(swing) => self.engine.set_swing(swing),
            Command::Sequencer(on) => self.engine.set_sequencer(on),
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
