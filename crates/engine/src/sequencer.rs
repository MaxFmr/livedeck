//! 16-step drum sequencer: 4 patterns, per-step velocity and probability, swing, pattern changes
//! quantized to the bar.
//!
//! Step `n` (a 16th note, counted from the transport start) falls on beat `n / 4`, then the swing
//! pushes the offbeat 8ths late (or the odd 16ths, with `SwingGrid::Sixteenth`): at 50 % they are
//! straight, at 75 % the pair is a dotted note + its third. The engine splits its blocks on step
//! positions, so every hit lands on its exact sample.
//!
//! Runs on: audio thread.

use crate::clock::Clock;
use crate::drums::{DrumKit, DrumVoice};

pub const STEPS: usize = 16;
pub const PATTERNS: usize = 4;
const VOICES: usize = DrumVoice::ALL.len();
pub const MIN_SWING: f32 = 0.5;
pub const MAX_SWING: f32 = 0.75;

/// One cell of the grid; `velocity` 0 = no hit.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Step {
    pub velocity: f32,
    /// Chance (0..=1) the hit plays each time the step comes round.
    pub probability: f32,
}

/// Grid of every voice × 16 steps, indexed like `DrumVoice::ALL`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pattern {
    pub steps: [[Step; STEPS]; VOICES],
}

impl Pattern {
    /// Builds a pattern from text rows, one character per step:
    /// `X` accent (1.0), `x` normal (0.75), `o` ghost (0.4), `?` normal at 50 % chance, else off.
    #[must_use]
    pub fn from_rows(rows: &[(DrumVoice, &str)]) -> Self {
        let mut pattern = Self::default();
        for &(voice, text) in rows {
            let Some(row) = DrumVoice::ALL
                .iter()
                .position(|&v| v == voice)
                .and_then(|i| pattern.steps.get_mut(i))
            else {
                continue;
            };
            for (cell, ch) in row.iter_mut().zip(text.chars()) {
                *cell = match ch {
                    'X' => Step {
                        velocity: 1.0,
                        probability: 1.0,
                    },
                    'x' => Step {
                        velocity: 0.75,
                        probability: 1.0,
                    },
                    'o' => Step {
                        velocity: 0.4,
                        probability: 1.0,
                    },
                    '?' => Step {
                        velocity: 0.75,
                        probability: 0.5,
                    },
                    _ => Step::default(),
                };
            }
        }
        pattern
    }
}

/// Starting patterns of the hybrid kit: A sparse backbeat, B groove, C four-on-the-floor, D busy
/// break. Style presets (014) will replace them.
#[must_use]
pub fn default_patterns() -> [Pattern; PATTERNS] {
    use DrumVoice::{Clap, ClosedHat, Kick, OpenHat, Perc, Snare};
    [
        Pattern::from_rows(&[
            (Kick, "X.......X......."),
            (Snare, "....X.......X..."),
            (ClosedHat, "x.x.x.x.x.x.x.x."),
        ]),
        Pattern::from_rows(&[
            (Kick, "X.....x...X....."),
            (Snare, "....X.......X..o"),
            (ClosedHat, "x.xxx.x.x.xxx.x?"),
            (OpenHat, "..............x."),
            (Clap, "....x.......x..."),
            (Perc, "..o....o.....o.."),
        ]),
        Pattern::from_rows(&[
            (Kick, "X...X...X...X..."),
            (Clap, "....X.......X..."),
            (ClosedHat, "x...x...x...x..."),
            (OpenHat, "..x...x...x...x."),
            (Perc, "...o..o....o..o."),
        ]),
        Pattern::from_rows(&[
            (Kick, "X..x..X...x.X..."),
            (Snare, "....X..o.o..X.oX"),
            (ClosedHat, "xoxoxoxoxoxoxoxo"),
            (Clap, "............X..."),
            (Perc, "x.?..?.x.?..?.x."),
        ]),
    ]
}

/// Note value the swing pushes late: 8th offbeats (default, audible on 8th-note hats) or odd 16ths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SwingGrid {
    #[default]
    Eighth,
    Sixteenth,
}

#[derive(Debug, Clone)]
pub struct Sequencer {
    patterns: [Pattern; PATTERNS],
    current: u8,
    pending: Option<u8>,
    /// Index of the next step to play, counted from the transport start.
    next_step: u64,
    swing: f32,
    grid: SwingGrid,
    rng: u32,
    /// Off: steps still advance (the bar position stays right) but play nothing.
    enabled: bool,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequencer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: default_patterns(),
            current: 0,
            pending: None,
            next_step: 0,
            swing: 0.54,
            grid: SwingGrid::Eighth,
            rng: 0x9E37_79B9,
            enabled: true,
        }
    }

    /// Back to step 0 (transport start); a pending pattern starts right away.
    pub fn reset(&mut self) {
        self.next_step = 0;
        if let Some(p) = self.pending.take() {
            self.current = p;
        }
    }

    /// Queues pattern `index` (0-based) for the next bar, or switches now if the transport is stopped.
    pub fn queue(&mut self, index: u8, playing: bool) {
        if usize::from(index) >= PATTERNS {
            return;
        }
        if playing && index != self.current {
            self.pending = Some(index);
        } else {
            self.current = index;
            self.pending = None;
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_swing(&mut self, swing: f32) {
        if swing.is_finite() {
            self.swing = swing.clamp(MIN_SWING, MAX_SWING);
        }
    }

    #[must_use]
    pub fn swing(&self) -> f32 {
        self.swing
    }

    #[must_use]
    pub fn current(&self) -> u8 {
        self.current
    }

    #[must_use]
    pub fn pending(&self) -> Option<u8> {
        self.pending
    }

    /// Step (0..16) of the bar about to play.
    #[must_use]
    pub fn step_in_bar(&self) -> u8 {
        (self.next_step % STEPS as u64) as u8
    }

    pub fn set_swing_grid(&mut self, grid: SwingGrid) {
        self.grid = grid;
    }

    /// Beat of `step`: inside each swung pair (two 8ths or two 16ths), the second note moves from
    /// half-way to `swing`, and the 16ths in between follow linearly.
    fn step_beats(&self, step: u64) -> f64 {
        let pair = match self.grid {
            SwingGrid::Eighth => 1.0,
            SwingGrid::Sixteenth => 0.5,
        };
        let straight = step as f64 / 4.0;
        let start = (straight / pair).floor() * pair;
        let q = (straight - start) / pair;
        let s = f64::from(self.swing);
        let warped = if q < 0.5 {
            q * 2.0 * s
        } else {
            s + (q - 0.5) * 2.0 * (1.0 - s)
        };
        start + warped * pair
    }

    /// Offset of the next step inside the next `frames` samples, if the transport plays.
    #[must_use]
    pub fn next_step_within(&self, clock: &Clock, frames: usize) -> Option<usize> {
        if !clock.is_playing() {
            return None;
        }
        let at = clock.sample_at(self.step_beats(self.next_step));
        let offset = usize::try_from(at.saturating_sub(clock.position())).ok()?;
        (offset < frames).then_some(offset)
    }

    fn chance(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x as f32 / u32::MAX as f32
    }

    /// Plays the step returned by `next_step_within` (its sample is reached) and moves on.
    pub fn fire(&mut self, kit: &mut DrumKit) {
        let step = (self.next_step % STEPS as u64) as usize;
        if step == 0
            && let Some(p) = self.pending.take()
        {
            self.current = p;
        }
        if self.enabled
            && let Some(pattern) = self.patterns.get(usize::from(self.current))
        {
            let rows = pattern.steps;
            for (voice, row) in DrumVoice::ALL.into_iter().zip(rows) {
                let Some(cell) = row.get(step).copied() else {
                    continue;
                };
                if cell.velocity > 0.0
                    && (cell.probability >= 1.0 || self.chance() < cell.probability)
                {
                    kit.trigger(voice, cell.velocity);
                }
            }
        }
        self.next_step += 1;
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Steps fired while running the clock for `frames`, as (sample, step index in bar).
    fn run(seq: &mut Sequencer, clock: &mut Clock, frames: usize) -> Vec<(u64, u8)> {
        let mut kit = DrumKit::new(48_000);
        let mut fired = Vec::new();
        let mut left = frames;
        while left > 0 {
            match seq.next_step_within(clock, left) {
                Some(offset) => {
                    clock.advance(offset);
                    left -= offset;
                    fired.push((clock.position(), seq.step_in_bar()));
                    seq.fire(&mut kit);
                }
                None => {
                    clock.advance(left);
                    left = 0;
                }
            }
        }
        fired
    }

    fn started(bpm: f64) -> Clock {
        let mut clock = Clock::new(48_000, bpm);
        clock.start();
        clock
    }

    #[test]
    fn straight_steps_fall_on_16th_notes() {
        let mut seq = Sequencer::new();
        seq.set_swing(0.5);
        let mut clock = started(120.0);
        // 120 BPM: a 16th lasts 6 000 samples.
        let fired = run(&mut seq, &mut clock, 24_001);
        let samples: Vec<u64> = fired.iter().map(|f| f.0).collect();
        assert_eq!(samples, vec![0, 6_000, 12_000, 18_000, 24_000]);
    }

    #[test]
    fn eighth_swing_delays_the_offbeat_and_its_16ths() {
        let mut seq = Sequencer::new();
        seq.set_swing(0.75);
        let mut clock = started(120.0);
        // Beat = 24 000 samples: the offbeat 8th moves from 12 000 to 18 000, the 16ths around it
        // stay half-way inside each (now unequal) 8th.
        let samples: Vec<u64> = run(&mut seq, &mut clock, 24_001)
            .iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(samples, vec![0, 9_000, 18_000, 21_000, 24_000]);
    }

    #[test]
    fn sixteenth_swing_delays_odd_steps_only() {
        let mut seq = Sequencer::new();
        seq.set_swing_grid(SwingGrid::Sixteenth);
        seq.set_swing(0.75);
        let mut clock = started(120.0);
        let samples: Vec<u64> = run(&mut seq, &mut clock, 24_001)
            .iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(samples, vec![0, 9_000, 12_000, 21_000, 24_000]);
    }

    #[test]
    fn pattern_change_waits_for_the_bar() {
        let mut seq = Sequencer::new();
        let mut clock = started(120.0);
        run(&mut seq, &mut clock, 6_000 * 5); // into the bar
        seq.queue(2, true);
        assert_eq!((seq.current(), seq.pending()), (0, Some(2)));
        // Up to the last step of bar 1: still pattern A.
        run(&mut seq, &mut clock, 6_000 * 11 - 1);
        assert_eq!(seq.current(), 0);
        run(&mut seq, &mut clock, 6_000);
        assert_eq!((seq.current(), seq.pending()), (2, None));
    }

    #[test]
    fn stopped_transport_switches_at_once_and_never_fires() {
        let mut seq = Sequencer::new();
        let mut clock = Clock::new(48_000, 120.0);
        seq.queue(3, clock.is_playing());
        assert_eq!((seq.current(), seq.pending()), (3, None));
        assert!(run(&mut seq, &mut clock, 100_000).is_empty());
    }

    #[test]
    fn probability_plays_about_half_the_time() {
        let mut seq = Sequencer::new();
        let pattern = Pattern::from_rows(&[(DrumVoice::Perc, "????????????????")]);
        let mut played = 0;
        for _ in 0..2_000 {
            if pattern.steps[5][0].probability >= 1.0 || seq.chance() < 0.5 {
                played += 1;
            }
        }
        assert!((800..1_200).contains(&played), "{played}");
    }

    #[test]
    fn text_rows_parse_velocity_and_chance() {
        let p = Pattern::from_rows(&[(DrumVoice::Snare, "Xxo?.")]);
        let row = p.steps[1];
        assert_eq!(row[0].velocity, 1.0);
        assert_eq!(row[1].velocity, 0.75);
        assert_eq!(row[2].velocity, 0.4);
        assert_eq!(row[3].probability, 0.5);
        assert_eq!(row[4].velocity, 0.0);
        assert_eq!(p.steps[0][0], Step::default(), "other voices stay empty");
    }
}
