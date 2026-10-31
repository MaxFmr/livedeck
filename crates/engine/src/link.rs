//! Lock-free link between the audio thread and the control thread.
//!
//! Control → audio: `Command` through an SPSC ring buffer (one producer: the control thread).
//! Audio → control: `EngineEvent` through another ring buffer, and the latest `EngineSnapshot`
//! through a triple buffer (the reader never blocks the writer).
//!
//! Runs on: `AudioEngine` on the audio thread, `EngineHandle` on the control thread.

use rtrb::{Consumer, Producer, RingBuffer};
use triple_buffer::{Input, Output, triple_buffer};

use crate::{BEATS_PER_BAR, Engine};

/// Everything the audio thread can be asked to do. Small and `Copy`: no heap, no drop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Start,
    Stop,
    /// Absolute tempo, clamped to the clock range.
    SetTempo(f64),
    Metronome(bool),
}

const _: () = assert!(core::mem::size_of::<Command>() <= 32);

/// Things the audio thread reports, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineEvent {
    /// The last block played into a new beat (1-based bar and beat).
    Beat { bar: u64, beat: u8 },
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
    }
}

impl AudioEngine {
    /// Audio callback body: applies pending commands, renders, publishes state.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        // Bounded even if the control thread keeps pushing while we drain.
        for _ in 0..COMMAND_CAPACITY {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            self.apply(command);
        }
        self.engine.process(out, channels);
        self.report_beat();
        self.snapshot
            .write(snapshot_of(&self.engine, self.applied, self.dropped_events));
    }

    fn apply(&mut self, command: Command) {
        self.applied += 1;
        match command {
            Command::Start => {
                self.engine.start();
                self.last_beat = None;
            }
            Command::Stop => self.engine.stop(),
            Command::SetTempo(bpm) => self.engine.set_tempo(bpm),
            Command::Metronome(on) => self.engine.set_metronome(on),
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
        while let Some(EngineEvent::Beat { bar, beat }) = handle.next_event() {
            beats.push((bar, beat));
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
    fn metronome_can_be_muted() {
        let (mut audio, mut handle) = pair();
        handle.send(Command::Metronome(false));
        handle.send(Command::Start);
        let mut buf = [0.0_f32; 256];
        audio.process(&mut buf, 2);
        assert!(buf.iter().all(|&s| s == 0.0));
        assert!(!handle.snapshot().metronome);
    }
}
