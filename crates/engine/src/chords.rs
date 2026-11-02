//! Chords of the melody deck: scale degrees stacked in thirds (always in the key), voiced by an
//! inversion, an octave and an extension, played on the poly synth by the pads that hold them.
//!
//! Pads 0–3 play degrees I, IV, V, vi; pads 4–7 (Shift) ii, iii, vii and I an octave up. With latch
//! on, a chord keeps sounding after its pad is released, until another one is played.
//!
//! Runs on: audio thread.

use crate::bass::Key;
use crate::poly::PolySynth;

/// Chord pads: 4 plain + 4 with Shift.
pub const PADS: usize = 8;
/// Degree played by each pad.
const DEGREES: [u8; PADS] = [1, 4, 5, 6, 2, 3, 7, 8];
/// Octave of the chords above the bass register (A minor: A3, C4, E4).
const OCTAVE: i8 = 1;
pub const MAX_INVERSION: i8 = 3;
pub const MAX_OCTAVE: i8 = 2;
const VELOCITY: f32 = 0.8;

/// What is stacked on the degree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Extension {
    #[default]
    Triad,
    Seventh,
    Sus4,
    Add9,
}

impl Extension {
    pub const ALL: [Self; 4] = [Self::Triad, Self::Seventh, Self::Sus4, Self::Add9];

    /// Scale steps above the degree.
    #[must_use]
    pub fn steps(self) -> &'static [u8] {
        match self {
            Self::Triad => &[0, 2, 4],
            Self::Seventh => &[0, 2, 4, 6],
            Self::Sus4 => &[0, 3, 4],
            Self::Add9 => &[0, 2, 4, 8],
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Triad => "triade",
            Self::Seventh => "7e",
            Self::Sus4 => "sus4",
            Self::Add9 => "add9",
        }
    }

    #[must_use]
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&e| e == self).unwrap_or(0) as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(usize::from(index) % Self::ALL.len())
            .copied()
            .unwrap_or_default()
    }
}

/// How a chord is laid out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Voicing {
    /// Positive: lowest notes moved up an octave, one per step; negative: highest moved down.
    pub inversion: i8,
    /// Whole chord, in octaves.
    pub octave: i8,
    pub extension: Extension,
}

/// MIDI notes of `degree` in `key` with `voicing`, lowest first; returns how many are used.
#[must_use]
pub fn chord_notes(key: Key, degree: u8, voicing: Voicing) -> ([f32; 4], usize) {
    let mut notes = [0.0_f32; 4];
    let steps = voicing.extension.steps();
    let octave = OCTAVE + voicing.octave.clamp(-MAX_OCTAVE, MAX_OCTAVE);
    for (note, &step) in notes.iter_mut().zip(steps) {
        *note = key.note(degree + step, octave);
    }
    let count = steps.len().min(notes.len());
    let Some(used) = notes.get_mut(..count) else {
        return (notes, 0);
    };
    let inversion = voicing.inversion.clamp(-MAX_INVERSION, MAX_INVERSION);
    for _ in 0..inversion.unsigned_abs() {
        used.sort_by(f32::total_cmp);
        if inversion > 0 {
            if let Some(low) = used.first_mut() {
                *low += 12.0;
            }
        } else if let Some(high) = used.last_mut() {
            *high -= 12.0;
        }
    }
    used.sort_by(f32::total_cmp);
    (notes, count)
}

/// Which chord pads are held and sounding, the latch and the voicing.
#[derive(Debug, Clone, Default)]
pub struct ChordPads {
    held: [bool; PADS],
    sounding: [bool; PADS],
    latch: bool,
    voicing: Voicing,
}

impl ChordPads {
    fn play(&mut self, synth: &mut PolySynth, key: Key, pad: u8) {
        let Some(&degree) = DEGREES.get(usize::from(pad)) else {
            return;
        };
        synth.release(pad);
        let (notes, count) = chord_notes(key, degree, self.voicing);
        for &note in notes.iter().take(count) {
            synth.note_on(note, VELOCITY, pad);
        }
        if let Some(s) = self.sounding.get_mut(usize::from(pad)) {
            *s = true;
        }
    }

    fn stop(&mut self, synth: &mut PolySynth, pad: usize) {
        synth.release(pad as u8);
        if let Some(s) = self.sounding.get_mut(pad) {
            *s = false;
        }
    }

    /// Pad `pad` (0..8) pressed or released.
    pub fn pad(&mut self, synth: &mut PolySynth, key: Key, pad: u8, on: bool) {
        let p = usize::from(pad);
        if p >= PADS {
            return;
        }
        if let Some(h) = self.held.get_mut(p) {
            *h = on;
        }
        if on {
            if self.latch {
                // A new chord replaces the latched ones that are no longer held.
                for other in 0..PADS {
                    if other != p && !self.held.get(other).copied().unwrap_or(false) {
                        self.stop(synth, other);
                    }
                }
            }
            self.play(synth, key, pad);
        } else if !self.latch {
            self.stop(synth, p);
        }
    }

    /// Latch on: released chords keep sounding; off: those no longer held stop.
    pub fn set_latch(&mut self, synth: &mut PolySynth, on: bool) {
        self.latch = on;
        if !on {
            for pad in 0..PADS {
                if !self.held.get(pad).copied().unwrap_or(false) {
                    self.stop(synth, pad);
                }
            }
        }
    }

    /// New voicing, applied to the chords sounding right away.
    pub fn set_voicing(&mut self, synth: &mut PolySynth, key: Key, voicing: Voicing) {
        self.voicing = voicing;
        for pad in 0..PADS as u8 {
            if self
                .sounding
                .get(usize::from(pad))
                .copied()
                .unwrap_or(false)
            {
                self.play(synth, key, pad);
            }
        }
    }

    /// Everything stops (transport stop).
    pub fn release_all(&mut self, synth: &mut PolySynth) {
        synth.release_all();
        self.sounding = [false; PADS];
    }

    #[must_use]
    pub fn latch(&self) -> bool {
        self.latch
    }

    #[must_use]
    pub fn voicing(&self) -> Voicing {
        self.voicing
    }

    /// Bit `i` set = chord pad `i` sounding.
    #[must_use]
    pub fn sounding(&self) -> u8 {
        self.sounding
            .iter()
            .enumerate()
            .fold(0, |m, (i, &s)| if s { m | 1 << i } else { m })
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::bass::Scale;

    const A_MINOR: Key = Key {
        tonic: 9,
        scale: Scale::Minor,
    };

    fn notes(degree: u8, voicing: Voicing) -> Vec<f32> {
        let (n, c) = chord_notes(A_MINOR, degree, voicing);
        n[..c].to_vec()
    }

    #[test]
    fn triads_stay_in_the_key() {
        assert_eq!(notes(1, Voicing::default()), vec![57.0, 60.0, 64.0]); // A C E
        assert_eq!(notes(4, Voicing::default()), vec![62.0, 65.0, 69.0]); // D F A
        assert_eq!(notes(7, Voicing::default()), vec![67.0, 71.0, 74.0]); // G B D
        assert_eq!(notes(8, Voicing::default()), vec![69.0, 72.0, 76.0]); // A an octave up
        let major = Key {
            tonic: 0,
            scale: Scale::Major,
        };
        let (n, c) = chord_notes(major, 5, Voicing::default());
        assert_eq!(&n[..c], &[67.0, 71.0, 74.0]); // G B D (C sits at C3, chords an octave up)
    }

    #[test]
    fn inversions_octaves_and_extensions() {
        let v = |inversion, octave, extension| Voicing {
            inversion,
            octave,
            extension,
        };
        assert_eq!(notes(1, v(1, 0, Extension::Triad)), vec![60.0, 64.0, 69.0]);
        assert_eq!(notes(1, v(2, 0, Extension::Triad)), vec![64.0, 69.0, 72.0]);
        assert_eq!(notes(1, v(-1, 0, Extension::Triad)), vec![52.0, 57.0, 60.0]);
        assert_eq!(notes(1, v(0, 1, Extension::Triad)), vec![69.0, 72.0, 76.0]);
        assert_eq!(
            notes(1, v(0, 0, Extension::Seventh)),
            vec![57.0, 60.0, 64.0, 67.0]
        );
        assert_eq!(notes(1, v(0, 0, Extension::Sus4)), vec![57.0, 62.0, 64.0]);
        assert_eq!(
            notes(1, v(0, 0, Extension::Add9)),
            vec![57.0, 60.0, 64.0, 71.0]
        );
    }

    #[test]
    fn latch_keeps_the_chord_until_the_next_one() {
        let mut synth = PolySynth::new(48_000);
        let mut pads = ChordPads::default();
        pads.set_latch(&mut synth, true);
        pads.pad(&mut synth, A_MINOR, 0, true);
        pads.pad(&mut synth, A_MINOR, 0, false);
        assert_eq!(pads.sounding(), 0b1, "latched");
        pads.pad(&mut synth, A_MINOR, 2, true);
        assert_eq!(pads.sounding(), 0b100, "replaced");
        pads.set_latch(&mut synth, false);
        assert_eq!(pads.sounding(), 0b100, "still held");
        pads.pad(&mut synth, A_MINOR, 2, false);
        assert_eq!(pads.sounding(), 0);
    }

    #[test]
    fn voicing_change_replays_the_chords_sounding() {
        let mut synth = PolySynth::new(48_000);
        let mut pads = ChordPads::default();
        pads.pad(&mut synth, A_MINOR, 0, true);
        assert_eq!(synth.active_voices(), 3);
        let seventh = Voicing {
            extension: Extension::Seventh,
            ..Voicing::default()
        };
        pads.set_voicing(&mut synth, A_MINOR, seventh);
        // The old triad is releasing, the new 7th chord sounds.
        assert_eq!(synth.active_voices(), 7);
    }
}
