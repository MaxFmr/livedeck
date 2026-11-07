//! Melody loops recorded on the fly (runs on: audio thread).
//!
//! The note pads of deck B play scale degrees, its chord pads chords; REC (or a loop slot pressed
//! while empty) records both from the next bar, quantized to the sixteenth, for `bars` bars, then
//! plays them in a loop. One loop plays at a time; starting, switching and stopping happen at the
//! bar. Notes and chords are stored as scale degrees, so a loop follows the key, the scale, the
//! transposition (and the voicing, for chords).
//!
//! Everything is fixed-size: no allocation, no locking.

use crate::bass::Key;
use crate::chords::{Voicing, chord_notes, pad_degree};
use crate::poly::PolySynth;
use crate::sequencer::STEPS;

/// Loop slots (the four Sampler pads of deck B).
pub const SLOTS: usize = 4;
/// Notes a loop can hold.
pub const MAX_NOTES: usize = 64;
/// Loop lengths offered, in bars.
pub const LENGTHS: [u8; 3] = [1, 2, 4];
/// Longest loop, in steps.
const MAX_STEPS: u16 = 4 * STEPS as u16;
/// Note pads (degrees 1 to 8, the last an octave up).
pub const NOTE_PADS: usize = 8;
/// Synth owners: live notes `NOTE_OWNER + pad`, loop notes `LOOP_OWNER + voice`.
pub const NOTE_OWNER: u8 = 10;
pub const LOOP_OWNER: u8 = 20;
/// Loop notes sounding at once.
const LOOP_VOICES: usize = 8;
const NOTE_OCTAVE: i8 = 2;
const NOTE_VELOCITY: f32 = 0.8;

/// A recorded note: start step in the loop, scale degree (0-based) or chord pad, length in steps.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LoopNote {
    pub start: u16,
    pub degree: i8,
    pub len: u16,
    /// `degree` is a chord pad (0..8), played with the voicing of the moment.
    pub chord: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    notes: [LoopNote; MAX_NOTES],
    count: usize,
    /// Length in bars (set when recording starts).
    bars: u8,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            notes: [LoopNote::default(); MAX_NOTES],
            count: 0,
            bars: 2,
        }
    }
}

impl Slot {
    fn steps(&self) -> u16 {
        u16::from(self.bars) * STEPS as u16
    }
}

/// What a slot is doing, for the display and the LEDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopState {
    #[default]
    Empty,
    /// Recording starts at the next bar.
    Armed,
    Recording,
    /// Recorded, silent.
    Stopped,
    /// Starts at the next bar.
    Waiting,
    Playing,
    /// Stops at the next bar.
    Stopping,
}

impl LoopState {
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Armed,
            2 => Self::Recording,
            3 => Self::Stopped,
            4 => Self::Waiting,
            5 => Self::Playing,
            6 => Self::Stopping,
            _ => Self::Empty,
        }
    }
}

/// A note pad held while recording: where it started (steps from the recording start, not
/// wrapped) and the note it became.
#[derive(Debug, Clone, Copy)]
struct Open {
    pad: u8,
    from: i32,
    note: usize,
}

#[derive(Debug, Clone)]
pub struct Looper {
    slots: [Slot; SLOTS],
    states: [LoopState; SLOTS],
    /// Length of the next recording, in bars.
    bars: u8,
    /// Slot recording or armed, and the beat its recording starts on.
    recording: Option<(usize, f64)>,
    /// Slot playing (or stopping) and its position, in steps.
    playing: Option<usize>,
    pos: u16,
    /// Pads held while recording: note pads, then chord pads.
    open: [Option<Open>; 2 * NOTE_PADS],
    /// Loop notes sounding: steps left per voice.
    voices: [u16; LOOP_VOICES],
}

impl Default for Looper {
    fn default() -> Self {
        Self::new()
    }
}

impl Looper {
    #[must_use]
    pub fn new() -> Self {
        Self {
            slots: [Slot::default(); SLOTS],
            states: [LoopState::Empty; SLOTS],
            bars: 2,
            recording: None,
            playing: None,
            pos: 0,
            open: [None; 2 * NOTE_PADS],
            voices: [0; LOOP_VOICES],
        }
    }

    #[must_use]
    pub fn states(&self) -> [LoopState; SLOTS] {
        self.states
    }

    /// Length of the next recording, in bars.
    #[must_use]
    pub fn bars(&self) -> u8 {
        self.bars
    }

    /// Notes recorded in each slot.
    #[must_use]
    pub fn note_counts(&self) -> [u8; SLOTS] {
        self.slots.map(|s| u8::try_from(s.count).unwrap_or(u8::MAX))
    }

    /// Bars of each slot (what it was recorded with).
    #[must_use]
    pub fn slot_bars(&self) -> [u8; SLOTS] {
        self.slots.map(|s| s.bars)
    }

    /// Steps of the loop playing (or being recorded) and how far it is, for the display.
    #[must_use]
    pub fn progress(&self, beats: f64) -> Option<(u16, u16)> {
        if let Some((slot, start)) = self.recording
            && self.states.get(slot) == Some(&LoopState::Recording)
        {
            let steps = self.slots.get(slot).map_or(0, Slot::steps);
            let done = ((beats - start) * 4.0).clamp(0.0, f64::from(steps)) as u16;
            return Some((done, steps));
        }
        let slot = self.playing?;
        let steps = self.slots.get(slot)?.steps();
        Some((self.pos, steps))
    }

    /// Next recording length: one step through `LENGTHS` (±).
    pub fn nudge_bars(&mut self, delta: i8) {
        let at = LENGTHS.iter().position(|&b| b == self.bars).unwrap_or(1) as i16;
        let next = (at + i16::from(delta)).clamp(0, LENGTHS.len() as i16 - 1);
        self.bars = LENGTHS
            .get(usize::try_from(next).unwrap_or(1))
            .copied()
            .unwrap_or(2);
    }

    /// Loop pad `slot` pressed. `beats`: clock position; `playing`: the transport runs.
    pub fn press(&mut self, slot: usize, beats: f64, playing: bool) {
        let Some(&state) = self.states.get(slot) else {
            return;
        };
        let next_bar = if playing {
            (beats / BAR).floor() * BAR + BAR
        } else {
            0.0
        };
        match state {
            LoopState::Empty => {
                // A recording armed elsewhere is given up.
                self.cancel_recording();
                if let Some(s) = self.slots.get_mut(slot) {
                    *s = Slot {
                        bars: self.bars,
                        ..Slot::default()
                    };
                }
                self.set_state(slot, LoopState::Armed);
                self.recording = Some((slot, next_bar));
            }
            LoopState::Armed | LoopState::Recording => {
                // Pressed again: the recording is dropped.
                self.cancel_recording();
            }
            LoopState::Stopped => {
                for other in 0..SLOTS {
                    if self.states.get(other) == Some(&LoopState::Waiting) {
                        self.set_state(other, LoopState::Stopped);
                    }
                }
                if let Some(p) = self.playing {
                    self.set_state(p, LoopState::Stopping);
                }
                if playing {
                    self.set_state(slot, LoopState::Waiting);
                } else {
                    self.start_playing(slot);
                }
            }
            LoopState::Waiting => {
                self.set_state(slot, LoopState::Stopped);
                if let Some(p) = self.playing {
                    self.set_state(p, LoopState::Playing);
                }
            }
            LoopState::Playing => {
                if playing {
                    self.set_state(slot, LoopState::Stopping);
                } else {
                    self.set_state(slot, LoopState::Stopped);
                    self.playing = None;
                }
            }
            LoopState::Stopping => self.set_state(slot, LoopState::Playing),
        }
    }

    /// REC: arms the first empty slot (from the next bar); while a recording is armed or runs,
    /// drops it. Returns the slot armed.
    pub fn rec(&mut self, beats: f64, playing: bool) -> Option<usize> {
        if self.recording.is_some() {
            self.cancel_recording();
            return None;
        }
        let slot = (0..SLOTS).find(|&s| self.states.get(s) == Some(&LoopState::Empty))?;
        self.press(slot, beats, playing);
        Some(slot)
    }

    /// Chord pad `pad` pressed or released (it sounds through the chord pads): recorded when a
    /// recording runs.
    pub fn chord(&mut self, pad: u8, on: bool, beats: f64) {
        if usize::from(pad) < NOTE_PADS {
            self.record(usize::from(pad), true, on, beats);
        }
    }

    /// Erases slot `slot` (stops it first).
    pub fn clear(&mut self, slot: usize, synth: &mut PolySynth) {
        if self.recording.is_some_and(|(s, _)| s == slot) {
            self.cancel_recording();
        }
        if self.playing == Some(slot) {
            self.playing = None;
            self.release_all(synth);
        }
        if let Some(s) = self.slots.get_mut(slot) {
            s.count = 0;
        }
        self.set_state(slot, LoopState::Empty);
    }

    fn cancel_recording(&mut self) {
        if let Some((slot, _)) = self.recording.take() {
            self.set_state(slot, LoopState::Empty);
        }
        self.open = [None; 2 * NOTE_PADS];
    }

    fn set_state(&mut self, slot: usize, state: LoopState) {
        if let Some(s) = self.states.get_mut(slot) {
            *s = state;
        }
    }

    fn start_playing(&mut self, slot: usize) {
        self.playing = Some(slot);
        self.pos = 0;
        self.set_state(slot, LoopState::Playing);
    }

    /// Note pad `pad` (0..8) pressed or released: sounds at once, and is recorded when a
    /// recording runs (or starts within half a step).
    pub fn note(
        &mut self,
        pad: u8,
        on: bool,
        synth: &mut PolySynth,
        (key, transpose): (Key, i8),
        beats: f64,
    ) {
        let p = usize::from(pad);
        if p >= NOTE_PADS {
            return;
        }
        let owner = NOTE_OWNER + pad;
        synth.release(owner);
        if on {
            let midi = key.note_at(i32::from(pad) + i32::from(transpose), NOTE_OCTAVE);
            synth.note_on(midi, NOTE_VELOCITY, owner);
        }
        self.record(p, false, on, beats);
    }

    fn record(&mut self, pad: usize, chord: bool, on: bool, beats: f64) {
        let key = pad + if chord { NOTE_PADS } else { 0 };
        let Some((slot, start)) = self.recording else {
            return;
        };
        let at = ((beats - start) * 4.0).round() as i32;
        let Some(s) = self.slots.get_mut(slot) else {
            return;
        };
        let steps = i32::from(s.steps());
        if on {
            // Armed: only a note played within half a step of the start counts.
            if at < 0 || at >= steps {
                return;
            }
            if s.count >= MAX_NOTES {
                return;
            }
            let note = s.count;
            if let Some(n) = s.notes.get_mut(note) {
                *n = LoopNote {
                    start: u16::try_from(at).unwrap_or(0),
                    degree: pad as i8,
                    len: 1,
                    chord,
                };
            }
            s.count += 1;
            if let Some(o) = self.open.get_mut(key) {
                *o = Some(Open {
                    pad: pad as u8,
                    from: at,
                    note,
                });
            }
        } else if let Some(open) = self.open.get_mut(key).and_then(Option::take)
            && let Some(n) = s.notes.get_mut(open.note)
            && usize::from(open.pad) == pad
        {
            n.len = u16::try_from((at - open.from).clamp(1, steps)).unwrap_or(1);
        }
    }

    /// Step `step` of the bar (0..16) just fired by the sequencer, at `beats`: bar changes, then
    /// the loop notes of this step.
    pub fn fire_step(
        &mut self,
        step: usize,
        beats: f64,
        synth: &mut PolySynth,
        (key, transpose, voicing): (Key, i8, Voicing),
    ) {
        // Loop notes end on the step their length reaches.
        for (voice, left) in self.voices.iter_mut().enumerate() {
            if *left > 0 {
                *left -= 1;
                if *left == 0 {
                    synth.release(LOOP_OWNER + voice as u8);
                }
            }
        }
        if step == 0 {
            self.on_bar(beats, synth);
        }
        let Some(slot) = self.playing else {
            return;
        };
        let Some(s) = self.slots.get(slot) else {
            return;
        };
        let steps = s.steps().max(1);
        for n in s.notes.iter().take(s.count) {
            if n.start != self.pos {
                continue;
            }
            // A free voice, else the one closest to its end.
            let voice = (0..LOOP_VOICES)
                .min_by_key(|&v| self.voices.get(v).copied().unwrap_or(0))
                .unwrap_or(0);
            let owner = LOOP_OWNER + voice as u8;
            synth.release(owner);
            if n.chord {
                let degree = pad_degree(n.degree as u8).unwrap_or(1);
                let (notes, count) = chord_notes(key, degree, voicing, transpose);
                for &note in notes.iter().take(count) {
                    synth.note_on(note, NOTE_VELOCITY, owner);
                }
            } else {
                let midi = key.note_at(i32::from(n.degree) + i32::from(transpose), NOTE_OCTAVE);
                synth.note_on(midi, NOTE_VELOCITY, owner);
            }
            if let Some(left) = self.voices.get_mut(voice) {
                *left = n.len.clamp(1, MAX_STEPS);
            }
        }
        self.pos = (self.pos + 1) % steps;
    }

    fn on_bar(&mut self, beats: f64, synth: &mut PolySynth) {
        const EPS: f64 = 0.01;
        if let Some((slot, start)) = self.recording {
            let state = self.states.get(slot).copied();
            let steps = self.slots.get(slot).map_or(0, Slot::steps);
            if state == Some(LoopState::Armed) && beats >= start - EPS {
                self.set_state(slot, LoopState::Recording);
            } else if state == Some(LoopState::Recording)
                && beats >= start + f64::from(steps) / 4.0 - EPS
            {
                self.recording = None;
                if self.slots.get(slot).is_some_and(|s| s.count == 0) {
                    // Nothing played: the slot stays free.
                    self.set_state(slot, LoopState::Empty);
                    return;
                }
                // Recorded: it plays from here, in place of the loop playing.
                if let Some(p) = self.playing
                    && p != slot
                {
                    self.set_state(p, LoopState::Stopped);
                }
                self.release_all(synth);
                self.start_playing(slot);
                return;
            }
        }
        let waiting = (0..SLOTS).find(|&s| self.states.get(s) == Some(&LoopState::Waiting));
        if let Some(p) = self.playing
            && self.states.get(p) == Some(&LoopState::Stopping)
        {
            self.set_state(p, LoopState::Stopped);
            self.playing = None;
            self.release_all(synth);
        }
        if let Some(slot) = waiting {
            self.release_all(synth);
            self.start_playing(slot);
        }
    }

    fn release_all(&mut self, synth: &mut PolySynth) {
        for (voice, left) in self.voices.iter_mut().enumerate() {
            synth.release(LOOP_OWNER + voice as u8);
            *left = 0;
        }
    }

    /// Transport start: the loop playing starts over; an armed recording starts with it.
    pub fn reset(&mut self) {
        self.pos = 0;
        if let Some((slot, _)) = self.recording {
            self.recording = Some((slot, 0.0));
        }
    }

    /// Transport stop: live and loop notes are released by the engine; a recording is dropped.
    pub fn stop(&mut self) {
        if self.states.contains(&LoopState::Recording) {
            self.cancel_recording();
        }
        self.voices = [0; LOOP_VOICES];
        for slot in 0..SLOTS {
            match self.states.get(slot) {
                Some(LoopState::Waiting) => self.set_state(slot, LoopState::Stopped),
                Some(LoopState::Stopping) => {
                    self.set_state(slot, LoopState::Stopped);
                    self.playing = None;
                }
                _ => {}
            }
        }
    }
}

const BAR: f64 = crate::BEATS_PER_BAR as f64;

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::bass::Scale;

    const KEY: (Key, i8) = (
        Key {
            tonic: 9,
            scale: Scale::Minor,
        },
        0,
    );
    const PLAY: (Key, i8, Voicing) = (
        KEY.0,
        0,
        Voicing {
            inversion: 0,
            octave: 0,
            extension: crate::chords::Extension::Triad,
        },
    );

    /// Fires every step from `from` to `to` (in steps, straight sixteenths).
    fn run(l: &mut Looper, synth: &mut PolySynth, from: u32, to: u32) {
        for s in from..to {
            l.fire_step(s as usize % STEPS, f64::from(s) / 4.0, synth, PLAY);
        }
    }

    #[test]
    fn a_loop_records_from_the_next_bar_then_plays() {
        let mut l = Looper::new();
        let mut synth = PolySynth::new(48_000);
        l.nudge_bars(-1); // 1 bar
        run(&mut l, &mut synth, 0, 5);
        l.press(0, 1.2, true);
        assert_eq!(l.states()[0], LoopState::Armed);
        // Played a little before the bar: counts as step 0.
        l.note(2, true, &mut synth, KEY, 3.95);
        run(&mut l, &mut synth, 5, 17);
        assert_eq!(l.states()[0], LoopState::Recording);
        l.note(2, false, &mut synth, KEY, 4.5); // 2 steps
        l.note(4, true, &mut synth, KEY, 6.02); // step 8
        l.note(4, false, &mut synth, KEY, 6.27);
        run(&mut l, &mut synth, 17, 33);
        assert_eq!(l.states()[0], LoopState::Playing);
        let s = &l.slots[0];
        assert_eq!(s.count, 2);
        assert_eq!(
            s.notes[0],
            LoopNote {
                start: 0,
                degree: 2,
                len: 2,
                chord: false
            }
        );
        assert_eq!(
            s.notes[1],
            LoopNote {
                start: 8,
                degree: 4,
                len: 1,
                chord: false
            }
        );
        // Step 32 played the loop's step 0: its note sounds.
        assert!(synth.active_voices() > 0);
    }

    #[test]
    fn loops_start_switch_and_stop_at_the_bar() {
        let mut l = Looper::new();
        let mut synth = PolySynth::new(48_000);
        l.states = [
            LoopState::Stopped,
            LoopState::Stopped,
            LoopState::Empty,
            LoopState::Empty,
        ];
        l.press(0, 1.0, true);
        assert_eq!(l.states()[0], LoopState::Waiting);
        run(&mut l, &mut synth, 0, 17);
        assert_eq!(l.states()[0], LoopState::Playing);
        l.press(1, 5.0, true);
        assert_eq!(l.states()[0], LoopState::Stopping);
        run(&mut l, &mut synth, 17, 33);
        assert_eq!(&l.states()[..2], &[LoopState::Stopped, LoopState::Playing]);
        l.press(1, 9.0, true);
        run(&mut l, &mut synth, 33, 49);
        assert_eq!(l.states()[1], LoopState::Stopped);
        l.clear(1, &mut synth);
        assert_eq!(l.states()[1], LoopState::Empty);
    }

    #[test]
    fn rec_arms_the_first_empty_slot_and_records_chords() {
        let mut l = Looper::new();
        let mut synth = PolySynth::new(48_000);
        l.states[0] = LoopState::Stopped;
        assert_eq!(l.rec(0.0, false), Some(1));
        l.reset();
        run(&mut l, &mut synth, 0, 1);
        assert_eq!(l.states()[1], LoopState::Recording);
        l.chord(3, true, 0.25);
        l.chord(3, false, 2.0);
        run(&mut l, &mut synth, 1, 33);
        assert_eq!(l.states()[1], LoopState::Playing);
        assert_eq!(
            l.slots[1].notes[0],
            LoopNote {
                start: 1,
                degree: 3,
                len: 7,
                chord: true
            }
        );
        // A chord sounds several voices.
        run(&mut l, &mut synth, 33, 34);
        assert!(synth.active_voices() >= 3);
        // REC again while nothing records: the next empty slot.
        assert_eq!(l.rec(9.0, true), Some(2));
        assert_eq!(l.rec(9.5, true), None, "pressed again: dropped");
        assert_eq!(l.states()[2], LoopState::Empty);
    }

    #[test]
    fn a_silent_recording_leaves_the_slot_empty() {
        let mut l = Looper::new();
        let mut synth = PolySynth::new(48_000);
        l.press(1, 0.0, false);
        l.reset();
        run(&mut l, &mut synth, 0, 33);
        assert_eq!(l.states()[1], LoopState::Empty);
    }

    #[test]
    fn lengths_scroll_between_one_and_four_bars() {
        let mut l = Looper::new();
        assert_eq!(l.bars(), 2);
        l.nudge_bars(5);
        assert_eq!(l.bars(), 4);
        l.nudge_bars(-1);
        l.nudge_bars(-1);
        assert_eq!(l.bars(), 1);
    }
}
