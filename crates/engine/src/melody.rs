//! Melody deck on the clock: the arpeggiator (plays the notes of the chords sounding, one at a
//! time, on its own grid) and the phrases (one-bar motifs in scale degrees, brought by the style,
//! looped on the sequencer steps).
//!
//! Both move with the chord transposition and stay in the key. A phrase starts and stops at the
//! bar; Shift plays its variation (second half a third higher, an answer to the first).
//!
//! Runs on: audio thread.

use crate::bass::{BassPattern, Key};
use crate::chords::ChordPads;
use crate::clock::Clock;
use crate::poly::PolySynth;
use crate::sequencer::{PATTERNS, STEPS, SwingGrid, step_beats};

/// Synth owners of the arpeggio and phrase notes (chord pads use 0..8).
pub const ARP_OWNER: u8 = 8;
pub const PHRASE_OWNER: u8 = 9;
pub const MAX_ARP_OCTAVES: u8 = 4;
/// Part of a tick the arpeggio note is held.
const ARP_GATE: f64 = 0.5;
const ARP_VELOCITY: f32 = 0.75;
/// Phrases sit an octave above the chords (A minor: A4).
const PHRASE_OCTAVE: i8 = 2;
const PHRASE_VELOCITY: f32 = 0.7;
const PHRASE_ACCENT: f32 = 0.95;
/// Degrees the variation lifts the second half of a phrase by (a third).
const VARIATION_LIFT: i32 = 2;
/// Notes the arpeggio can draw from (8 chords of 4 notes, without duplicates).
const ARP_NOTES: usize = 32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArpMode {
    #[default]
    Off,
    Up,
    UpDown,
    Random,
}

impl ArpMode {
    pub const ALL: [Self; 4] = [Self::Off, Self::Up, Self::UpDown, Self::Random];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Up => "montant",
            Self::UpDown => "aller-retour",
            Self::Random => "aléatoire",
        }
    }

    #[must_use]
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&m| m == self).unwrap_or(0) as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(usize::from(index))
            .copied()
            .unwrap_or_default()
    }
}

/// Note value of the arpeggio, slowest first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArpRate {
    Quarter,
    Eighth,
    EighthTriplet,
    #[default]
    Sixteenth,
    SixteenthTriplet,
    ThirtySecond,
}

impl ArpRate {
    pub const ALL: [Self; 6] = [
        Self::Quarter,
        Self::Eighth,
        Self::EighthTriplet,
        Self::Sixteenth,
        Self::SixteenthTriplet,
        Self::ThirtySecond,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Quarter => "1/4",
            Self::Eighth => "1/8",
            Self::EighthTriplet => "1/8 triolet",
            Self::Sixteenth => "1/16",
            Self::SixteenthTriplet => "1/16 triolet",
            Self::ThirtySecond => "1/32",
        }
    }

    #[must_use]
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&r| r == self).unwrap_or(0) as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(usize::from(index))
            .copied()
            .unwrap_or_default()
    }

    /// Ticks per beat.
    #[must_use]
    pub fn per_beat(self) -> u64 {
        match self {
            Self::Quarter => 1,
            Self::Eighth => 2,
            Self::EighthTriplet => 3,
            Self::Sixteenth => 4,
            Self::SixteenthTriplet => 6,
            Self::ThirtySecond => 8,
        }
    }

    /// Beat of `tick`: 8ths, 16ths and 32nds follow the swing of the drums (a 32nd sits half-way
    /// between two swung 16ths), quarters fall on the beat, triplets stay straight.
    #[must_use]
    pub fn tick_beats(self, tick: u64, swing: f32, grid: SwingGrid) -> f64 {
        match self {
            Self::Quarter => tick as f64,
            Self::Eighth => step_beats(tick * 2, swing, grid),
            Self::Sixteenth => step_beats(tick, swing, grid),
            Self::ThirtySecond => {
                let from = step_beats(tick / 2, swing, grid);
                if tick.is_multiple_of(2) {
                    from
                } else {
                    (from + step_beats(tick / 2 + 1, swing, grid)) / 2.0
                }
            }
            Self::EighthTriplet | Self::SixteenthTriplet => tick as f64 / self.per_beat() as f64,
        }
    }
}

/// Settings of the arpeggiator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArpSettings {
    pub mode: ArpMode,
    pub rate: ArpRate,
    /// Octaves spanned, 1..=`MAX_ARP_OCTAVES`.
    pub octaves: u8,
}

impl Default for ArpSettings {
    fn default() -> Self {
        Self {
            mode: ArpMode::Off,
            rate: ArpRate::Sixteenth,
            octaves: 1,
        }
    }
}

/// A phrase and whether its variation plays.
pub type PhraseChoice = (u8, bool);

/// Arpeggiator and phrase player.
#[derive(Debug, Clone)]
pub struct Melody {
    arp: ArpSettings,
    /// Next arpeggio tick, counted from the transport start on the grid of `arp.rate`.
    next_tick: u64,
    /// Notes played since the arpeggio started (position in its up / up-down cycle).
    played: usize,
    last_pick: usize,
    rng: u32,
    notes: [f32; ARP_NOTES],
    arp_off_at: Option<u64>,
    /// Last notes started by the arpeggio and the phrase (MIDI).
    last_arp_note: f32,
    last_phrase_note: f32,
    phrase: Option<PhraseChoice>,
    /// Phrase change waiting for the bar (`Some(None)` = stop).
    pending_phrase: Option<Option<PhraseChoice>>,
    phrase_off_at: Option<u64>,
}

impl Default for Melody {
    fn default() -> Self {
        Self::new()
    }
}

impl Melody {
    #[must_use]
    pub fn new() -> Self {
        Self {
            arp: ArpSettings::default(),
            next_tick: 0,
            played: 0,
            last_pick: usize::MAX,
            rng: 0x2545_F491,
            notes: [0.0; ARP_NOTES],
            arp_off_at: None,
            last_arp_note: 0.0,
            last_phrase_note: 0.0,
            phrase: None,
            pending_phrase: None,
            phrase_off_at: None,
        }
    }

    #[must_use]
    pub fn arp(&self) -> ArpSettings {
        self.arp
    }

    /// Whether the arpeggio is the one playing the chords (on, and the transport runs).
    #[must_use]
    pub fn arp_active(&self, clock: &Clock) -> bool {
        self.arp.mode != ArpMode::Off && clock.is_playing()
    }

    /// New arpeggiator settings; the grid restarts on the next tick of the new rate.
    pub fn set_arp(&mut self, settings: ArpSettings, clock: &Clock, swing: f32, grid: SwingGrid) {
        let settings = ArpSettings {
            octaves: settings.octaves.clamp(1, MAX_ARP_OCTAVES),
            ..settings
        };
        if settings.mode != self.arp.mode {
            self.played = 0;
        }
        self.arp = settings;
        self.resync(clock, swing, grid);
    }

    /// Next tick at or after the clock position.
    fn resync(&mut self, clock: &Clock, swing: f32, grid: SwingGrid) {
        if !clock.is_playing() {
            return;
        }
        let rate = self.arp.rate;
        let mut tick = (clock.beats() * rate.per_beat() as f64).floor().max(0.0) as u64;
        while clock.sample_at(rate.tick_beats(tick, swing, grid)) < clock.position() {
            tick += 1;
        }
        self.next_tick = tick;
    }

    /// Phrase pad pressed: queues it for the bar (now if stopped); pressing what plays (or waits)
    /// again stops it.
    pub fn press_phrase(&mut self, index: u8, variation: bool, playing: bool) {
        if usize::from(index) >= PATTERNS {
            return;
        }
        let pressed = Some((index, variation));
        let wanted = self.pending_phrase.unwrap_or(self.phrase);
        let next = if wanted == pressed { None } else { pressed };
        if playing {
            self.pending_phrase = (next != self.phrase).then_some(next);
        } else {
            self.phrase = next;
            self.pending_phrase = None;
        }
    }

    #[must_use]
    pub fn phrase(&self) -> Option<PhraseChoice> {
        self.phrase
    }

    #[must_use]
    pub fn pending_phrase(&self) -> Option<Option<PhraseChoice>> {
        self.pending_phrase
    }

    /// Transport start: the arpeggio grid and the bar start over, a waiting phrase starts.
    pub fn reset(&mut self) {
        self.next_tick = 0;
        self.played = 0;
        self.arp_off_at = None;
        self.phrase_off_at = None;
        if let Some(p) = self.pending_phrase.take() {
            self.phrase = p;
        }
    }

    /// Transport stop (the synth is released by the engine).
    pub fn stop(&mut self) {
        self.arp_off_at = None;
        self.phrase_off_at = None;
    }

    fn next_tick_sample(&self, clock: &Clock, swing: f32, grid: SwingGrid) -> u64 {
        clock.sample_at(self.arp.rate.tick_beats(self.next_tick, swing, grid))
    }

    /// Offset of the next arpeggio tick or note end inside the next `frames` samples.
    #[must_use]
    pub fn next_event_within(
        &self,
        clock: &Clock,
        swing: f32,
        grid: SwingGrid,
        frames: usize,
    ) -> Option<usize> {
        if !clock.is_playing() {
            return None;
        }
        let tick =
            (self.arp.mode != ArpMode::Off).then(|| self.next_tick_sample(clock, swing, grid));
        [tick, self.arp_off_at, self.phrase_off_at]
            .into_iter()
            .flatten()
            .filter_map(|at| usize::try_from(at.saturating_sub(clock.position())).ok())
            .filter(|&offset| offset < frames)
            .min()
    }

    /// Handles what `next_event_within` announced, once its sample is reached: note ends, then
    /// the arpeggio tick.
    pub fn on_time(
        &mut self,
        synth: &mut PolySynth,
        chords: &ChordPads,
        key: Key,
        clock: &Clock,
        (swing, grid): (f32, SwingGrid),
    ) {
        let now = clock.position();
        if self.arp_off_at.is_some_and(|at| at <= now) {
            synth.release(ARP_OWNER);
            self.arp_off_at = None;
        }
        if self.phrase_off_at.is_some_and(|at| at <= now) {
            synth.release(PHRASE_OWNER);
            self.phrase_off_at = None;
        }
        if self.arp.mode == ArpMode::Off || self.next_tick_sample(clock, swing, grid) > now {
            return;
        }
        let from = self.next_tick_sample(clock, swing, grid);
        self.next_tick += 1;
        let to = self.next_tick_sample(clock, swing, grid);
        let count = chords.notes(key, &mut self.notes);
        if count == 0 {
            // Nothing held: the next chord starts its arpeggio from the bottom.
            self.played = 0;
            return;
        }
        let total = count * usize::from(self.arp.octaves.max(1));
        let pick = match self.arp.mode {
            ArpMode::Off => return,
            ArpMode::Up => self.played % total,
            ArpMode::UpDown => {
                let period = (2 * total).saturating_sub(2).max(1);
                let p = self.played % period;
                if p < total { p } else { period - p }
            }
            ArpMode::Random => self.random_pick(total),
        };
        self.played = self.played.wrapping_add(1);
        let Some(&base) = self.notes.get(pick % count) else {
            return;
        };
        let note = base + 12.0 * (pick / count) as f32;
        synth.release(ARP_OWNER);
        synth.note_on(note, ARP_VELOCITY, ARP_OWNER);
        self.last_arp_note = note;
        let gate = (to.saturating_sub(from) as f64 * ARP_GATE).max(1.0) as u64;
        self.arp_off_at = Some(now + gate);
    }

    /// A random note of `total`, never the same twice in a row (when there is a choice).
    fn random_pick(&mut self, total: usize) -> usize {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        let mut pick = x as usize % total;
        if total > 1 && pick == self.last_pick {
            pick = (pick + 1) % total;
        }
        self.last_pick = pick;
        pick
    }

    /// Plays step `step` (0..16) of the bar, just fired by the sequencer. `gate_of(len, held)`
    /// gives the gate of a note of `len` steps from that step.
    pub fn fire_step(
        &mut self,
        step: usize,
        synth: &mut PolySynth,
        (key, transpose): (Key, i8),
        phrases: Option<&[BassPattern; PATTERNS]>,
        clock: &Clock,
        gate_of: impl Fn(u8, bool) -> u64,
    ) {
        if step == 0
            && let Some(p) = self.pending_phrase.take()
        {
            if p.is_none() {
                synth.release(PHRASE_OWNER);
                self.phrase_off_at = None;
            }
            self.phrase = p;
        }
        let Some((index, variation)) = self.phrase else {
            return;
        };
        let Some(pattern) = phrases.and_then(|p| p.get(usize::from(index))) else {
            return;
        };
        let Some(note) = pattern.steps.get(step).copied().flatten() else {
            return;
        };
        let lift = if variation && step >= STEPS / 2 {
            VARIATION_LIFT
        } else {
            0
        };
        let degree = i32::from(note.degree.max(1)) - 1 + i32::from(transpose) + lift;
        let velocity = if note.accent {
            PHRASE_ACCENT
        } else {
            PHRASE_VELOCITY
        };
        let midi = key.note_at(degree, PHRASE_OCTAVE + note.octave);
        synth.release(PHRASE_OWNER);
        synth.note_on(midi, velocity, PHRASE_OWNER);
        self.last_phrase_note = midi;
        let held = pattern.held_into_glide(step, note.len);
        self.phrase_off_at = Some(clock.position() + gate_of(note.len, held));
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::bass::Scale;

    const SR: u32 = 48_000;
    const A_MINOR: Key = Key {
        tonic: 9,
        scale: Scale::Minor,
    };

    #[test]
    fn swung_grids_follow_the_drums() {
        let grid = SwingGrid::Eighth;
        assert_eq!(ArpRate::Quarter.tick_beats(3, 0.6, grid), 3.0);
        assert!((ArpRate::Eighth.tick_beats(1, 0.6, grid) - 0.6).abs() < 1e-6);
        assert!((ArpRate::Sixteenth.tick_beats(2, 0.6, grid) - 0.6).abs() < 1e-6);
        // Half-way between steps 0 (beat 0) and 1 (0.3 at 60 % on the 8th grid).
        assert!((ArpRate::ThirtySecond.tick_beats(1, 0.6, grid) - 0.15).abs() < 1e-6);
        assert!((ArpRate::EighthTriplet.tick_beats(2, 0.6, grid) - 2.0 / 3.0).abs() < 1e-9);
    }

    /// Runs the clock for `frames`, returning the arpeggio notes started (sample, MIDI note).
    fn run_arp(
        melody: &mut Melody,
        chords: &ChordPads,
        clock: &mut Clock,
        frames: usize,
    ) -> Vec<(u64, f32)> {
        let mut synth = PolySynth::new(SR);
        let mut started = Vec::new();
        let mut left = frames;
        while left > 0 {
            let Some(offset) = melody.next_event_within(clock, 0.5, SwingGrid::Eighth, left) else {
                clock.advance(left);
                break;
            };
            clock.advance(offset);
            left -= offset;
            let before = melody.played;
            melody.on_time(&mut synth, chords, A_MINOR, clock, (0.5, SwingGrid::Eighth));
            if melody.played != before {
                started.push((clock.position(), melody.last_arp_note));
            }
        }
        started
    }

    fn a_minor_held() -> ChordPads {
        let mut synth = PolySynth::new(SR);
        let mut chords = ChordPads::default();
        chords.set_arp(&mut synth, A_MINOR, true);
        chords.pad(&mut synth, A_MINOR, 0, true);
        chords
    }

    #[test]
    fn up_arpeggio_climbs_the_chord_on_16ths() {
        let mut clock = Clock::new(SR, 120.0);
        clock.start();
        let mut melody = Melody::new();
        let up = ArpSettings {
            mode: ArpMode::Up,
            rate: ArpRate::Sixteenth,
            octaves: 2,
        };
        melody.set_arp(up, &clock, 0.5, SwingGrid::Eighth);
        let chords = a_minor_held();
        // 120 BPM: a 16th every 6 000 samples.
        let notes = run_arp(&mut melody, &chords, &mut clock, 7 * 6_000 + 1);
        let at: Vec<u64> = notes.iter().map(|n| n.0).collect();
        assert_eq!(at, (0..8).map(|k| k * 6_000).collect::<Vec<_>>());
        let pitches: Vec<f32> = notes.iter().map(|n| n.1).collect();
        assert_eq!(
            pitches,
            vec![57.0, 60.0, 64.0, 69.0, 72.0, 76.0, 57.0, 60.0]
        );
    }

    #[test]
    fn up_down_turns_without_repeating_the_ends() {
        let mut clock = Clock::new(SR, 120.0);
        clock.start();
        let mut melody = Melody::new();
        let settings = ArpSettings {
            mode: ArpMode::UpDown,
            rate: ArpRate::Sixteenth,
            octaves: 1,
        };
        melody.set_arp(settings, &clock, 0.5, SwingGrid::Eighth);
        let chords = a_minor_held();
        let notes = run_arp(&mut melody, &chords, &mut clock, 5 * 6_000 + 1);
        let pitches: Vec<f32> = notes.iter().map(|n| n.1).collect();
        assert_eq!(pitches, vec![57.0, 60.0, 64.0, 60.0, 57.0, 60.0]);
    }

    #[test]
    fn arp_notes_end_half_way_and_nothing_plays_without_a_chord() {
        let mut clock = Clock::new(SR, 120.0);
        clock.start();
        let mut melody = Melody::new();
        let settings = ArpSettings {
            mode: ArpMode::Random,
            ..ArpSettings::default()
        };
        melody.set_arp(settings, &clock, 0.5, SwingGrid::Eighth);
        let mut synth = PolySynth::new(SR);
        melody.on_time(
            &mut synth,
            &a_minor_held(),
            A_MINOR,
            &clock,
            (0.5, SwingGrid::Eighth),
        );
        assert_eq!(melody.arp_off_at, Some(3_000));
        assert_eq!(synth.active_voices(), 1);
        clock.advance(6_000);
        melody.on_time(
            &mut synth,
            &ChordPads::default(),
            A_MINOR,
            &clock,
            (0.5, SwingGrid::Eighth),
        );
        assert_eq!(melody.played, 0, "no chord: the cycle starts over");
    }

    #[test]
    fn changing_the_rate_mid_bar_waits_for_its_next_tick() {
        let mut clock = Clock::new(SR, 120.0);
        clock.start();
        clock.advance(10_000);
        let mut melody = Melody::new();
        let settings = ArpSettings {
            mode: ArpMode::Up,
            rate: ArpRate::Eighth,
            octaves: 1,
        };
        melody.set_arp(settings, &clock, 0.5, SwingGrid::Eighth);
        // 8ths every 12 000 samples: the next one is at 12 000.
        assert_eq!(
            melody.next_event_within(&clock, 0.5, SwingGrid::Eighth, 48_000),
            Some(2_000)
        );
    }

    fn phrases() -> [BassPattern; PATTERNS] {
        let p = BassPattern::parse("1 . 3 . 5 . . . 1 . 3 . 5 . . .");
        [p; PATTERNS]
    }

    /// Notes a phrase starts over one bar from step 0 (step, MIDI note).
    fn bar(melody: &mut Melody, transpose: i8) -> Vec<(usize, f32)> {
        let clock = Clock::new(SR, 120.0);
        let mut synth = PolySynth::new(SR);
        let mut notes = Vec::new();
        for step in 0..STEPS {
            melody.phrase_off_at = None;
            melody.fire_step(
                step,
                &mut synth,
                (A_MINOR, transpose),
                Some(&phrases()),
                &clock,
                |_, _| 100,
            );
            if melody.phrase_off_at.is_some() {
                notes.push((step, melody.last_phrase_note));
            }
        }
        notes
    }

    #[test]
    fn phrases_start_and_stop_at_the_bar() {
        let mut melody = Melody::new();
        melody.press_phrase(0, false, true);
        assert_eq!(melody.phrase(), None);
        assert_eq!(melody.pending_phrase(), Some(Some((0, false))));
        let notes = bar(&mut melody, 0);
        assert_eq!(melody.phrase(), Some((0, false)));
        assert_eq!(
            notes,
            vec![
                (0, 69.0),
                (2, 72.0),
                (4, 76.0),
                (8, 69.0),
                (10, 72.0),
                (12, 76.0)
            ]
        );
        // Pressed again: stops at the next bar.
        melody.press_phrase(0, false, true);
        assert_eq!(melody.pending_phrase(), Some(None));
        assert!(bar(&mut melody, 0).is_empty());
        assert_eq!(melody.phrase(), None);
    }

    #[test]
    fn variation_answers_a_third_higher_and_transposition_moves_all() {
        let mut melody = Melody::new();
        melody.press_phrase(1, true, false);
        let notes = bar(&mut melody, 0);
        // Second half: C E G instead of A C E.
        assert_eq!(notes[3..], [(8, 72.0), (10, 76.0), (12, 79.0)]);
        let notes = bar(&mut melody, 1);
        assert_eq!(notes[0], (0, 71.0)); // B: one degree up
        // Pressing the variation of another phrase switches to it, stopped: right away.
        melody.press_phrase(2, false, false);
        assert_eq!(melody.phrase(), Some((2, false)));
    }
}
