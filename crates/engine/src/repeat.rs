//! Beat-repeat on the drum bus: while a Loop pad is held, a slice of one division is looped — in
//! the engine, the slice starting on the last hit played, so the roll always starts with a note
//! (slices cut on the grid looped tails, silences and half notes in the 012 test). The sequencer
//! keeps running underneath, so releasing the pad gives back the groove in place.
//!
//! The dry bus (the groove the sequencer keeps playing underneath) is kept in a history ring, so
//! the slice can start before the press: the first repetition comes at the next slice boundary
//! after the press, not one division later. Slices always come from the dry groove, like the
//! "slip" mode of DJ decks: from 1/16 to 1/4, the new loop is a beat of groove, not a beat of
//! the 1/16 stutter heard (012 test).
//!
//! Loops are kept in slots, one per division held: changing division records the new slice into
//! another slot while the current loop goes on, and the previous loops keep turning silently, in
//! phase. A shorter division subdivides what is heard instead, from the start of the current
//! turn: holding 1/4 on a snare then adding 1/16 rolls that snare faster, not the kick playing
//! underneath at that moment (012 test). Going
//! back to a division still held (release pad 3 while pad 1 is down) resumes its own loop at once,
//! instead of slicing the 1/16 roll into a new 1/4 loop (012 test). Releasing every pad forgets
//! them.
//!
//! Ramps hide the switches: live ↔ loop (the live signal fades out over the RAMP samples that end
//! on the first boundary, so a hit landing on it is never heard for a few ms), loop ↔ loop, and
//! both ends of every turn: ~4 ms in (it starts on a hit, whose attack clicks when rolled fast),
//! ~20 ms out with a raised cosine (a division often cuts through the body of the kick).
//!
//! Runs on: audio thread. All buffers are allocated once in `new`.

/// Length of the switch ramps, in samples (~10 ms at 48 kHz: at 2.7 ms, crossfading two kicks
/// out of phase still clicked).
const RAMP: usize = 480;
/// Fade-in of each turn, in samples (~4 ms): softens the attack replayed at every turn — the
/// kick's noise click, repeated 8 to 16 times a second, still clicked at 1.3 ms (012 test).
const FADE_IN: usize = 192;
/// Fade-out of each turn, in samples (~20 ms), at most a quarter of the turn.
const FADE_OUT: usize = 960;
/// Loop slots: one per Loop pad.
const SLOTS: usize = 4;

/// Note value repeated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Division {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl Division {
    #[must_use]
    pub fn beats(self) -> f64 {
        match self {
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
            Self::ThirtySecond => 0.125,
        }
    }

    /// 4, 8, 16 or 32 (the "1/n" written in the mapping file).
    #[must_use]
    pub fn from_denominator(n: u8) -> Option<Self> {
        match n {
            4 => Some(Self::Quarter),
            8 => Some(Self::Eighth),
            16 => Some(Self::Sixteenth),
            32 => Some(Self::ThirtySecond),
            _ => None,
        }
    }

    #[must_use]
    pub fn denominator(self) -> u8 {
        match self {
            Self::Quarter => 4,
            Self::Eighth => 8,
            Self::Sixteenth => 16,
            Self::ThirtySecond => 32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Capture {
    None,
    /// Slice of `len` samples starting at clock sample `at` (at or before now), recorded into
    /// `slot` and looped from clock sample `from` (at least `at + len`).
    Armed {
        at: u64,
        len: usize,
        from: u64,
        slot: usize,
        heard: bool,
    },
    Recording {
        len: usize,
        written: usize,
        from: u64,
        slot: usize,
        heard: bool,
    },
}

/// One recorded loop; `read` keeps turning even when it is not heard, so it can come back in
/// phase.
#[derive(Debug, Clone)]
struct Slot {
    buffer: Vec<f32>,
    len: usize,
    read: usize,
    /// Holds a loop of the current gesture (false once every pad is released).
    valid: bool,
    /// Last use, for picking the slot to overwrite.
    used: u64,
}

impl Slot {
    /// Current sample, with the fades of both ends of the turn.
    fn sample(&self) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        self.buffer.get(self.read).copied().unwrap_or(0.0) * BeatRepeat::seam(self.read, self.len)
    }
}

pub struct BeatRepeat {
    /// Dry bus of the last samples, indexed by clock position modulo its length.
    history: Vec<f32>,
    /// Output (what is heard) of the last samples, same indexing.
    heard: Vec<f32>,
    /// Clock position of the next sample to process.
    next_pos: u64,
    slots: [Slot; SLOTS],
    /// Slot heard (or fading out after a release).
    active: Option<usize>,
    capture: Capture,
    /// Previous slot fading out after a switch, with the samples left.
    fading: Option<(usize, usize)>,
    /// Whether the output should be the loop (the mix ramps towards it).
    looping: bool,
    /// 0 = live … 1 = loop.
    wet: f32,
    uses: u64,
}

impl BeatRepeat {
    /// `max_len`: longest slice in samples (one beat at the slowest tempo).
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        let len = max_len.max(RAMP * 2);
        let slot = Slot {
            buffer: vec![0.0; len],
            len: 0,
            read: 0,
            valid: false,
            used: 0,
        };
        Self {
            history: vec![0.0; len],
            heard: vec![0.0; len],
            next_pos: 0,
            slots: core::array::from_fn(|_| slot.clone()),
            active: None,
            capture: Capture::None,
            fading: None,
            looping: false,
            wet: 0.0,
            uses: 0,
        }
    }

    /// A pad is pressed: loops a new slice, the `len` samples starting at clock sample `at` (at
    /// or before now, within the history), from clock sample `from` on (raised to `at + len` if
    /// earlier). A loop already playing keeps going until then.
    pub fn engage(&mut self, at: u64, len: usize, from: u64) {
        self.arm(at, len, from, false);
    }

    /// A pad of a shorter division than the loop heard is pressed: loops `len` samples of what
    /// is heard from the start of the current turn, on the grid of that turn. Returns false when
    /// there is nothing to subdivide (the caller then slices the groove).
    pub fn subdivide(&mut self, len: usize) -> bool {
        let len = self.clamp(len);
        let Some(heard) = self.active.and_then(|a| self.slots.get(a)) else {
            return false;
        };
        if !self.looping || heard.len <= len {
            return false;
        }
        let at = self.next_pos.saturating_sub(heard.read as u64);
        let turns = (self.next_pos - at).div_ceil(len as u64).max(1);
        self.arm(at, len, at + turns * len as u64, true);
        true
    }

    fn arm(&mut self, at: u64, len: usize, from: u64, heard: bool) {
        let len = self.clamp(len);
        // A free slot first, else the least recently used one (never the one heard).
        let slot = (0..SLOTS)
            .filter(|&i| Some(i) != self.active)
            .min_by_key(|&i| self.slots.get(i).map_or((false, 0), |s| (s.valid, s.used)))
            .unwrap_or(0);
        if let Some(s) = self.slots.get_mut(slot) {
            s.valid = false;
            s.len = 0;
        }
        let from = from.max(at + len as u64);
        self.capture = Capture::Armed {
            at,
            len,
            from,
            slot,
            heard,
        };
    }

    /// Drops a slice not yet looping (a loop already heard goes on).
    pub fn cancel_pending(&mut self) {
        self.capture = Capture::None;
    }

    /// The pad looping `released` samples is let go while the pad looping `back` is still held:
    /// forgets the released loop and goes back to the held one — at once and in phase if it is
    /// kept. Returns false when there is no loop of `back` to go back to (the caller then slices
    /// a new one).
    pub fn go_back(&mut self, released: usize, back: usize) -> bool {
        let (released, back) = (self.clamp(released), self.clamp(back));
        if let Capture::Armed { len, .. } | Capture::Recording { len, .. } = self.capture
            && len == released
        {
            // Released before its roll began.
            self.capture = Capture::None;
        }
        let heard = self.active.and_then(|a| self.slots.get(a));
        let found = if self.looping && heard.is_some_and(|s| s.len == back) {
            true
        } else if let Some(slot) =
            (0..SLOTS).find(|&i| self.slots.get(i).is_some_and(|s| s.valid && s.len == back))
        {
            self.switch_to(slot);
            true
        } else {
            false
        };
        for slot in &mut self.slots {
            if slot.len == released {
                slot.valid = false;
            }
        }
        found
    }

    fn clamp(&self, len: usize) -> usize {
        len.clamp(FADE_IN * 2, self.history.len())
    }

    /// Back to the live signal (with a short ramp); the kept loops are forgotten.
    pub fn release(&mut self) {
        self.capture = Capture::None;
        self.looping = false;
        for slot in &mut self.slots {
            slot.valid = false;
        }
    }

    /// True while engaged (recording or looping).
    #[must_use]
    pub fn engaged(&self) -> bool {
        self.looping || self.capture != Capture::None
    }

    /// Samples kept in the history: how far back a slice may start.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Level of a loop sample at `read` in a turn of `len`: raised-cosine fades at both ends.
    fn seam(read: usize, len: usize) -> f32 {
        let smooth = |done: usize, over: usize| {
            if done >= over {
                1.0
            } else {
                0.5 - 0.5 * (core::f32::consts::PI * done as f32 / over as f32).cos()
            }
        };
        let from_end = len.saturating_sub(1) - read.min(len.saturating_sub(1));
        smooth(read, FADE_IN) * smooth(from_end, FADE_OUT.min(len / 4).max(1))
    }

    /// Makes `slot` the loop heard; the one heard before fades out.
    fn switch_to(&mut self, slot: usize) {
        if self.looping && self.active != Some(slot) {
            self.fading = self.active.map(|a| (a, RAMP));
        }
        self.uses += 1;
        if let Some(s) = self.slots.get_mut(slot) {
            s.valid = true;
            s.used = self.uses;
        }
        self.active = Some(slot);
        self.looping = true;
        self.capture = Capture::None;
    }

    /// Processes the drum bus in place; `start` is the clock position of `bus[0]`.
    pub fn process(&mut self, bus: &mut [f32], start: u64) {
        let size = self.history.len() as u64;
        let step = 1.0 / RAMP as f32;
        for (i, sample) in bus.iter_mut().enumerate() {
            let pos = start + i as u64;
            if let Capture::Armed {
                at,
                len,
                from,
                slot,
                heard,
            } = self.capture
                && pos >= at
            {
                // The slice began in the past: take it from the history.
                let done = usize::try_from(pos - at).unwrap_or(usize::MAX).min(len);
                let ring = if heard { &self.heard } else { &self.history };
                if let Some(s) = self.slots.get_mut(slot) {
                    for (k, dst) in s.buffer.iter_mut().take(done).enumerate() {
                        let src = ((at + k as u64) % size) as usize;
                        *dst = ring.get(src).copied().unwrap_or(0.0);
                    }
                }
                self.capture = Capture::Recording {
                    len,
                    written: done,
                    from,
                    slot,
                    heard,
                };
            }
            if let Capture::Recording {
                len,
                written,
                from,
                slot,
                ..
            } = self.capture
                && written >= len
                && pos >= from
            {
                if let Some(s) = self.slots.get_mut(slot) {
                    s.len = len;
                    s.read = 0;
                }
                self.switch_to(slot);
            }

            let live = *sample;
            // Before the first turn, the live signal fades out over the RAMP samples that end on
            // the boundary.
            let pre_roll = !self.looping
                && match self.capture {
                    Capture::Armed { from, .. } | Capture::Recording { from, .. } => {
                        pos + RAMP as u64 >= from
                    }
                    Capture::None => false,
                };
            let target = if self.looping || pre_roll { 1.0 } else { 0.0 };
            if self.wet < target {
                self.wet = (self.wet + step).min(1.0);
            } else if self.wet > target {
                self.wet = (self.wet - step).max(0.0);
            }
            let mut out = live;
            if pre_roll {
                out = live * (1.0 - self.wet);
            } else if self.wet > 0.0
                && let Some(active) = self.active
            {
                let mut looped = self.slots.get(active).map_or(0.0, Slot::sample);
                if let Some((slot, left)) = self.fading {
                    let g = left as f32 / RAMP as f32;
                    let old = self.slots.get(slot).map_or(0.0, Slot::sample);
                    looped = looped * (1.0 - g) + old * g;
                    self.fading = (left > 1).then_some((slot, left - 1));
                }
                out = live + (looped - live) * self.wet;
            }
            // Every loop keeps turning, heard or not.
            for s in &mut self.slots {
                if s.len > 0 {
                    s.read = (s.read + 1) % s.len;
                }
            }

            if let Capture::Recording {
                len,
                written,
                from,
                slot,
                heard,
            } = self.capture
                && written < len
            {
                if let Some(dst) = self
                    .slots
                    .get_mut(slot)
                    .and_then(|s| s.buffer.get_mut(written))
                {
                    *dst = if heard { out } else { live };
                }
                self.capture = Capture::Recording {
                    len,
                    written: written + 1,
                    from,
                    slot,
                    heard,
                };
            }
            let at = (pos % size) as usize;
            if let Some(h) = self.history.get_mut(at) {
                *h = live;
            }
            if let Some(h) = self.heard.get_mut(at) {
                *h = out;
            }
            *sample = out;
        }
        self.next_pos = start + bus.len() as u64;
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A ramp signal: every sample tells where it came from.
    fn ramp(start: u64, len: usize) -> Vec<f32> {
        (0..len).map(|i| (start + i as u64) as f32).collect()
    }

    /// Runs the ramp signal through `r` from `from` to `to` in blocks of 128.
    fn run(r: &mut BeatRepeat, from: u64, to: u64) -> Vec<f32> {
        let mut out = Vec::new();
        let mut pos = from;
        while pos < to {
            let n = (to - pos).min(128) as usize;
            let mut bus = ramp(pos, n);
            r.process(&mut bus, pos);
            out.extend(bus);
            pos += n as u64;
        }
        out
    }

    #[test]
    fn idle_passes_the_signal_untouched() {
        let mut r = BeatRepeat::new(1_000);
        assert_eq!(run(&mut r, 0, 256), ramp(0, 256));
    }

    #[test]
    fn repeats_the_division_of_the_press_from_the_next_boundary() {
        let mut r = BeatRepeat::new(40_000);
        assert_eq!(run(&mut r, 0, 12_000), ramp(0, 12_000));
        // Pressed at 12 000, inside the division 10 000..15 000: it repeats from 15 000.
        r.engage(10_000, 5_000, 0);
        let out = run(&mut r, 12_000, 26_000);
        assert_eq!(
            &out[..3_000 - RAMP],
            &ramp(12_000, 3_000 - RAMP)[..],
            "live until the pre-roll fade"
        );
        // Once the switch ramp is over, out[15 000 + k] plays sample 10 000 + k.
        assert_eq!(out[15_000 + 2_000 - 12_000], 12_000.0);
        assert_eq!(
            out[20_000 + 3_000 - 12_000],
            13_000.0,
            "second turn of the loop"
        );
    }

    #[test]
    fn rolls_an_old_slice_from_a_later_boundary() {
        let mut r = BeatRepeat::new(40_000);
        run(&mut r, 0, 17_000);
        // Last hit at 10 000, slices of 3 000: pressed at 17 000, the roll starts at 19 000.
        r.engage(10_000, 3_000, 19_000);
        let out = run(&mut r, 17_000, 26_000);
        assert_eq!(
            &out[..2_000 - RAMP],
            &ramp(17_000, 2_000 - RAMP)[..],
            "live until the pre-roll fade"
        );
        // Middle of each turn (clear of the fades).
        assert_eq!(out[19_000 + 1_500 - 17_000], 11_500.0);
        assert_eq!(out[22_000 + 1_500 - 17_000], 11_500.0, "second turn");
    }

    #[test]
    fn live_is_silent_on_the_boundary_where_the_loop_starts() {
        let mut r = BeatRepeat::new(40_000);
        run(&mut r, 0, 12_000);
        r.engage(10_000, 5_000, 0);
        let out = run(&mut r, 12_000, 16_000);
        // The live signal fades out over the RAMP samples before 15 000…
        assert_eq!(out[15_000 - RAMP - 1 - 12_000], (15_000 - RAMP - 1) as f32);
        assert!(out[15_000 - 1 - 12_000].abs() < 15_000.0 / RAMP as f32 * 1.01);
        // …so on the boundary only the loop (fading in from 10 000) is heard.
        assert!(out[15_000 - 12_000].abs() < 1.0);
        assert!(out[15_000 + 50 - 12_000] < 10_100.0);
    }

    #[test]
    fn release_ramps_back_to_live() {
        let mut r = BeatRepeat::new(40_000);
        r.engage(0, 3_000, 0);
        run(&mut r, 0, 10_000);
        r.release();
        let live = run(&mut r, 10_000, 10_600);
        assert_eq!(live[RAMP + 1], (10_000 + RAMP + 1) as f32);
        assert!(!r.engaged());
    }

    #[test]
    fn loop_seam_fades_to_zero() {
        let mut r = BeatRepeat::new(40_000);
        r.engage(0, 4_000, 0);
        let mut bus = vec![1.0; 20_000];
        r.process(&mut bus, 0);
        // Each turn fades in and out: silent at both ends, full level in the middle, the
        // fade-out longer than the fade-in.
        assert!(bus[4_000 + 3_999].abs() < 1.0e-6);
        assert!(bus[8_000].abs() < 1.0e-6);
        assert!((bus[8_000 + FADE_IN / 2] - 0.5).abs() < 0.01);
        assert_eq!(bus[8_000 + FADE_IN], 1.0);
        assert!((bus[8_000 + 3_999 - FADE_OUT / 2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn changing_division_never_falls_back_to_live() {
        let mut r = BeatRepeat::new(40_000);
        r.engage(0, 4_000, 0);
        run(&mut r, 0, 20_000);
        // New, shorter division from the boundary at 20 000: the first loop goes on while the
        // slice of dry groove is recorded, then the new loop (20 000 + k) takes over.
        r.engage(20_000, 2_000, 0);
        let out = run(&mut r, 20_000, 26_000);
        assert!(
            out[..2_000].iter().all(|&s| s < 4_000.0),
            "the live groove came back before the new loop"
        );
        assert_eq!(out[2_000 + RAMP + 500], (20_000 + RAMP + 500) as f32);
        assert_eq!(out[4_000 + 500], 20_500.0, "second turn");
        assert!(r.engaged());
    }

    #[test]
    fn going_back_resumes_the_kept_loop_in_phase() {
        let mut r = BeatRepeat::new(40_000);
        // Pad 1: loop of 0..4 000, heard from 4 000.
        r.engage(0, 4_000, 0);
        run(&mut r, 0, 20_000);
        // Pad 3: loop of 2 000 from 22 000 (dry groove 20 000..22 000).
        r.engage(20_000, 2_000, 0);
        run(&mut r, 20_000, 26_000);
        // Pad 3 released: loop 1 comes back at once, where it would be had it kept playing.
        assert!(r.go_back(2_000, 4_000));
        let out = run(&mut r, 26_000, 30_000);
        // At 26 000 + RAMP + k, loop 1 plays sample (26 000 + RAMP + k) mod 4 000.
        let at = RAMP + 500;
        assert_eq!(out[at], ((26_000 + at) % 4_000) as f32);
        // Pad 3 pressed again: a new slice of the groove (28 000..30 000), not its old loop.
        r.engage(28_000, 2_000, 0);
        let again = run(&mut r, 30_000, 33_000);
        assert_eq!(again[RAMP + 500], (28_000 + RAMP + 500) as f32);
    }

    #[test]
    fn from_short_to_long_loops_the_groove_not_the_stutter_and_back() {
        let mut r = BeatRepeat::new(40_000);
        // Pad 3 first: loop of 2 000 (dry 0..2 000).
        r.engage(0, 2_000, 0);
        run(&mut r, 0, 20_000);
        // Pad 1 added: a loop of 4 000 of the dry groove from 20 000, heard from 24 000.
        r.engage(20_000, 4_000, 0);
        let out = run(&mut r, 20_000, 30_000);
        assert_eq!(
            out[4_000 + RAMP + 1_000],
            (20_000 + RAMP + 1_000) as f32,
            "groove, not the 1/16 loop"
        );
        // Pad 1 released: the short loop comes back in phase (p mod 2 000).
        assert!(r.go_back(4_000, 2_000));
        let back = run(&mut r, 30_000, 31_500);
        let k = RAMP + 500;
        assert_eq!(back[k], ((30_000 + k) % 2_000) as f32);
    }

    #[test]
    fn going_back_before_the_new_roll_keeps_the_loop() {
        let mut r = BeatRepeat::new(40_000);
        r.engage(0, 4_000, 0);
        run(&mut r, 0, 20_000);
        // Pad 3 pressed, its roll would start at 22 000; released at 21 000.
        r.engage(18_000, 2_000, 22_000);
        run(&mut r, 20_000, 21_000);
        assert!(r.go_back(2_000, 4_000));
        assert!(r.engaged());
        let out = run(&mut r, 21_000, 25_000);
        // Loop 1 never stopped: sample p plays p mod 4 000.
        for k in [500, 1_500, 2_000] {
            assert_eq!(out[k], ((21_000 + k) % 4_000) as f32, "at {k}");
        }
    }

    #[test]
    fn a_shorter_division_subdivides_what_is_heard() {
        let mut r = BeatRepeat::new(40_000);
        // Pad 1: loop of the groove 0..4 000.
        r.engage(0, 4_000, 0);
        run(&mut r, 0, 21_000);
        // Pad 3 at 21 000, 1 000 into a turn (begun at 20 000): it rolls the first 1 000
        // samples of that turn (0..1 000), from the next 1 000 boundary of the turn, at once.
        assert!(r.subdivide(1_000));
        let out = run(&mut r, 21_000, 25_000);
        // Inner turns start at 21 000, 22 000…: sample 21 000 + k plays k mod 1 000.
        assert_eq!(out[1_500], 500.0);
        assert_eq!(out[2_000 + 300], 300.0, "rolled again two turns later");
        // A longer division does not subdivide.
        assert!(!r.subdivide(8_000));
    }

    #[test]
    fn denominators_round_trip() {
        for d in [4, 8, 16, 32] {
            assert_eq!(
                Division::from_denominator(d).map(Division::denominator),
                Some(d)
            );
        }
        assert_eq!(Division::from_denominator(3), None);
    }
}
