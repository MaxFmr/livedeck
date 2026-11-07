//! The generative phrase of deck B (R3, runs on: audio thread).
//!
//! One bar of melody, never a bank: it is drawn at random in the key (Generate), varied a little
//! (Mutate), kept as it is (Freeze) or erased (Clear). Unfrozen, it varies by itself every few
//! bars. Chord pads held while REC is held are recorded into it (overdub). Generate, Mutate and
//! Clear take effect at the next bar. Notes are scale degrees, so the phrase follows the key,
//! the scale, the transposition and the chord octave.
//!
//! Everything is fixed-size: no allocation, no locking.

use crate::bass::Key;
use crate::chords::{Voicing, chord_notes, pad_degree};
use crate::poly::PolySynth;
use crate::sequencer::STEPS;

/// Events a phrase can hold.
pub const MAX_EVENTS: usize = 32;
/// Synth owners of the phrase voices: `OWNER + voice`.
pub const OWNER: u8 = 20;
/// Phrase notes sounding at once.
const VOICES: usize = 8;
/// Octave of the phrase notes (that of the old phrases), moved by the chord octave.
const OCTAVE: i8 = 2;
const VELOCITY: f32 = 0.75;
const ACCENT: f32 = 0.95;
/// Unfrozen, the phrase mutates by itself every this many bars.
pub const EVOLVE_BARS: u8 = 4;
/// Chord pads.
const PADS: usize = 8;

/// One event: a note (scale degree, 0-based, may go past the octave) or a chord (pad 0..8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Event {
    /// Step in the bar, 0..16.
    pub start: u8,
    /// Length in steps, ≥ 1.
    pub len: u8,
    pub degree: i8,
    pub chord: bool,
    pub accent: bool,
}

/// A whole phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notes {
    pub events: [Event; MAX_EVENTS],
    pub count: u8,
}

impl Default for Notes {
    fn default() -> Self {
        Self {
            events: [Event::default(); MAX_EVENTS],
            count: 0,
        }
    }
}

impl Notes {
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.events.iter().take(usize::from(self.count))
    }

    fn push(&mut self, event: Event) -> bool {
        match self.events.get_mut(usize::from(self.count)) {
            Some(slot) => {
                *slot = event;
                self.count += 1;
                true
            }
            None => false,
        }
    }

    fn starts_at(&self, step: u8) -> bool {
        self.iter().any(|e| e.start == step && !e.chord)
    }
}

/// What waits for the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pending {
    #[default]
    None,
    Generate,
    Mutate,
    Clear,
}

impl Pending {
    #[must_use]
    pub fn index(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Generate,
            2 => Self::Mutate,
            3 => Self::Clear,
            _ => Self::None,
        }
    }
}

/// A chord pad held while recording.
#[derive(Debug, Clone, Copy)]
struct Open {
    /// Steps from the bar the press fell in (not wrapped), and the event it became.
    from: i32,
    event: u8,
}

#[derive(Debug, Clone)]
pub struct Phrase {
    notes: Notes,
    /// Ready to replace `notes` at the bar.
    next: Notes,
    pending: Pending,
    frozen: bool,
    /// Generation density, 0..=1 (fader Tempo B).
    density: f32,
    bars: u8,
    rng: u32,
    recording: bool,
    open: [Option<Open>; PADS],
    /// Steps left per voice (0 = free).
    voices: [u8; VOICES],
}

impl Default for Phrase {
    fn default() -> Self {
        Self::new()
    }
}

impl Phrase {
    #[must_use]
    pub fn new() -> Self {
        Self {
            notes: Notes::default(),
            next: Notes::default(),
            pending: Pending::None,
            frozen: true,
            density: 0.5,
            bars: 0,
            rng: 0x1234_5678,
            recording: false,
            open: [None; PADS],
            voices: [0; VOICES],
        }
    }

    #[must_use]
    pub fn notes(&self) -> &Notes {
        &self.notes
    }

    #[must_use]
    pub fn pending(&self) -> Pending {
        self.pending
    }

    #[must_use]
    pub fn frozen(&self) -> bool {
        self.frozen
    }

    #[must_use]
    pub fn recording(&self) -> bool {
        self.recording
    }

    #[must_use]
    pub fn density(&self) -> f32 {
        self.density
    }

    pub fn set_density(&mut self, density: f32) {
        if density.is_finite() {
            self.density = density.clamp(0.0, 1.0);
        }
    }

    /// Generate: a new phrase at the bar (now if stopped); it then moves by itself.
    pub fn generate(&mut self, playing: bool) {
        let next = self.generated();
        self.frozen = false;
        self.queue(Pending::Generate, next, playing);
    }

    /// Mutate: a few notes of the phrase change, at the bar (now if stopped).
    pub fn mutate(&mut self, playing: bool) {
        let next = self.mutated(self.notes);
        self.queue(Pending::Mutate, next, playing);
    }

    /// Clear: silence at the bar (now if stopped).
    pub fn clear(&mut self, playing: bool) {
        self.queue(Pending::Clear, Notes::default(), playing);
    }

    /// Freeze on / off: frozen, the phrase no longer moves by itself.
    pub fn set_frozen(&mut self, frozen: bool) {
        self.frozen = frozen;
        self.bars = 0;
    }

    fn queue(&mut self, kind: Pending, next: Notes, playing: bool) {
        if playing {
            self.next = next;
            self.pending = kind;
        } else {
            self.notes = next;
            self.pending = Pending::None;
        }
    }

    /// REC held (true) or released: chord pads played meanwhile are added to the phrase.
    pub fn set_recording(&mut self, on: bool) {
        self.recording = on;
        if !on {
            self.open = [None; PADS];
        }
    }

    /// Chord pad `pad` pressed or released at `beats`: recorded while REC is held.
    pub fn chord(&mut self, pad: u8, on: bool, beats: f64) {
        if !self.recording {
            return;
        }
        let at = (beats * 4.0).round() as i32;
        let Some(open) = self.open.get_mut(usize::from(pad)) else {
            return;
        };
        if on {
            let start = at.rem_euclid(STEPS as i32) as u8;
            let event = Event {
                start,
                len: 1,
                degree: pad as i8,
                chord: true,
                accent: false,
            };
            let index = self.notes.count;
            if self.notes.push(event) {
                *open = Some(Open {
                    from: at,
                    event: index,
                });
            }
        } else if let Some(o) = open.take()
            && let Some(e) = self.notes.events.get_mut(usize::from(o.event))
        {
            e.len = (at - o.from).clamp(1, STEPS as i32) as u8;
        }
    }

    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next_rand() % n.max(1)
    }

    /// A phrase drawn in the key: 3 to 12 notes by density, a random walk on the degrees, the
    /// offbeats favoured, lengths 1 to 3 steps.
    fn generated(&mut self) -> Notes {
        let mut notes = Notes::default();
        let target = 3 + (self.density * 9.0).round() as u32;
        let mut degree: i32 = self.below(5) as i32;
        let mut tries = 0;
        while u32::from(notes.count) < target && tries < 64 {
            tries += 1;
            let step = self.below(STEPS as u32) as u8;
            let offbeat = step % 2 == 1 || step % 4 == 2;
            if notes.starts_at(step) || (!offbeat && self.below(3) == 0) {
                continue;
            }
            degree = (degree + self.below(5) as i32 - 2).clamp(0, 7);
            let len = 1 + self.below(3) as u8;
            let accent = self.below(6) == 0;
            notes.push(Event {
                start: step,
                len,
                degree: degree as i8,
                chord: false,
                accent,
            });
        }
        notes
    }

    /// Two notes moved: a step or two in pitch, or to a free step.
    fn mutated(&mut self, mut notes: Notes) -> Notes {
        if notes.count == 0 {
            return self.generated();
        }
        for _ in 0..2 {
            let i = self.below(u32::from(notes.count)) as usize;
            let roll = self.below(3);
            let new_step = self.below(STEPS as u32) as u8;
            let free = !notes.starts_at(new_step);
            let Some(e) = notes.events.get_mut(i) else {
                continue;
            };
            if e.chord {
                continue;
            }
            match roll {
                0 if free => e.start = new_step,
                1 => e.degree = (e.degree + 1).min(9),
                _ => e.degree = (e.degree - 1).max(0),
            }
        }
        notes
    }

    /// Step `step` of the bar (0..16) just fired by the sequencer: what changes at the bar, then
    /// the events of this step on the melody synth.
    pub fn fire_step(
        &mut self,
        step: usize,
        synth: &mut PolySynth,
        (key, transpose, voicing): (Key, i8, Voicing),
    ) {
        for (voice, left) in self.voices.iter_mut().enumerate() {
            if *left > 0 {
                *left -= 1;
                if *left == 0 {
                    synth.release(OWNER + voice as u8);
                }
            }
        }
        if step == 0 {
            self.on_bar();
        }
        let octave = OCTAVE + voicing.octave;
        for i in 0..usize::from(self.notes.count) {
            let Some(e) = self.notes.events.get(i).copied() else {
                continue;
            };
            if usize::from(e.start) != step {
                continue;
            }
            let voice = (0..VOICES)
                .min_by_key(|&v| self.voices.get(v).copied().unwrap_or(0))
                .unwrap_or(0);
            let owner = OWNER + voice as u8;
            synth.release(owner);
            let velocity = if e.accent { ACCENT } else { VELOCITY };
            if e.chord {
                let degree = pad_degree(e.degree as u8).unwrap_or(1);
                let (notes, count) = chord_notes(key, degree, voicing, transpose);
                for &note in notes.iter().take(count) {
                    synth.note_on(note, velocity, owner);
                }
            } else {
                let midi = key.note_at(i32::from(e.degree) + i32::from(transpose), octave);
                synth.note_on(midi, velocity, owner);
            }
            if let Some(left) = self.voices.get_mut(voice) {
                *left = e.len.max(1);
            }
        }
    }

    fn on_bar(&mut self) {
        match self.pending {
            Pending::None => {
                if !self.frozen && self.notes.count > 0 {
                    self.bars += 1;
                    if self.bars >= EVOLVE_BARS {
                        self.bars = 0;
                        self.notes = self.mutated(self.notes);
                    }
                }
            }
            _ => {
                self.notes = self.next;
                self.pending = Pending::None;
                self.bars = 0;
            }
        }
    }

    /// Transport stop: voices are released by the engine; what waited applies now.
    pub fn stop(&mut self) {
        self.voices = [0; VOICES];
        if self.pending != Pending::None {
            self.notes = self.next;
            self.pending = Pending::None;
        }
        self.set_recording(false);
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::bass::Scale;
    use crate::chords::Extension;

    const PLAY: (Key, i8, Voicing) = (
        Key {
            tonic: 9,
            scale: Scale::Minor,
        },
        0,
        Voicing {
            inversion: 0,
            octave: 0,
            extension: Extension::Triad,
        },
    );

    fn bars(p: &mut Phrase, synth: &mut PolySynth, n: usize) {
        for s in 0..n * STEPS {
            p.fire_step(s % STEPS, synth, PLAY);
        }
    }

    #[test]
    fn density_sets_how_many_notes_are_drawn() {
        let mut p = Phrase::new();
        p.set_density(0.0);
        let sparse = p.generated().count;
        p.set_density(1.0);
        let dense = p.generated().count;
        assert!(sparse <= 4 && dense >= 9, "{sparse} {dense}");
        let n = p.generated();
        assert!(
            n.iter()
                .all(|e| e.start < 16 && e.len >= 1 && (0..=7).contains(&e.degree))
        );
    }

    #[test]
    fn generate_and_clear_wait_for_the_bar() {
        let mut p = Phrase::new();
        let mut synth = PolySynth::new(48_000);
        let before = *p.notes();
        bars(&mut p, &mut synth, 1);
        p.generate(true);
        assert_eq!(p.pending(), Pending::Generate);
        assert_eq!(*p.notes(), before);
        bars(&mut p, &mut synth, 1);
        assert_eq!(p.pending(), Pending::None);
        assert!(!p.frozen(), "a generated phrase moves");
        p.clear(true);
        bars(&mut p, &mut synth, 1);
        assert_eq!(p.notes().count, 0);
    }

    #[test]
    fn unfrozen_it_varies_every_few_bars_frozen_it_holds() {
        let mut p = Phrase::new();
        let mut synth = PolySynth::new(48_000);
        assert_eq!(p.notes().count, 0, "silent at start");
        p.generate(false);
        p.set_frozen(true);
        let first = *p.notes();
        bars(&mut p, &mut synth, 9);
        assert_eq!(*p.notes(), first, "frozen");
        p.set_frozen(false);
        bars(&mut p, &mut synth, usize::from(EVOLVE_BARS) + 1);
        assert_ne!(*p.notes(), first);
    }

    #[test]
    fn chords_played_while_rec_is_held_are_added() {
        let mut p = Phrase::new();
        p.clear(false);
        p.chord(0, true, 0.0);
        assert_eq!(p.notes().count, 0, "not recording");
        p.set_recording(true);
        p.chord(2, true, 1.0);
        p.chord(2, false, 1.75);
        p.set_recording(false);
        assert_eq!(
            p.notes().events[0],
            Event {
                start: 4,
                len: 3,
                degree: 2,
                chord: true,
                accent: false
            }
        );
        let mut synth = PolySynth::new(48_000);
        for s in 0..5 {
            p.fire_step(s, &mut synth, PLAY);
        }
        assert!(synth.active_voices() >= 3, "a chord sounds");
    }
}
