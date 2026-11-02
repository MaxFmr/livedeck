//! 16-step drum sequencer: 4 patterns, per-step velocity and probability, swing, pattern changes
//! quantized to the bar; voice mutes, density (ordered thinning of the steps) and fills.
//!
//! Step `n` (a 16th note, counted from the transport start) falls on beat `n / 4`, then the swing
//! pushes the offbeat 8ths late (or the odd 16ths, with `SwingGrid::Sixteenth`): at 50 % they are
//! straight, at 75 % the pair is a dotted note + its third. The engine splits its blocks on step
//! positions, so every hit lands on its exact sample.
//!
//! The bass line (`BassLine`) rides on the same steps and swing: each drum pattern brings the
//! bass line of the same index, which can also be changed on its own.
//!
//! Runs on: audio thread.

use crate::bass::{BassLine, BassVoice, gate_samples};
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
    /// `X` accent (1.0), `x` normal (0.75), `o` ghost (0.4), `1`–`9` velocity n/9 (crescendos),
    /// `?` normal at 50 % chance, else off.
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
                    '1'..='9' => Step {
                        velocity: f32::from(ch as u8 - b'0') / 9.0,
                        probability: 1.0,
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

/// Fills of the Sampler pads: 1 snare roll, 2 perc roll, 3 kick break, 4 hats rise. Only the part
/// from the next beat to the end of the bar is played, so each one builds up over the whole bar.
#[must_use]
pub fn default_fills() -> [Pattern; PATTERNS] {
    use DrumVoice::{Clap, ClosedHat, Kick, OpenHat, Perc, Snare};
    [
        Pattern::from_rows(&[(Kick, "X..............."), (Snare, "2.3.4.4.5656789X")]),
        Pattern::from_rows(&[
            (Kick, "X.......X......."),
            (Perc, "X..x..x..x.x.xxX"),
            (Clap, "............x..X"),
        ]),
        Pattern::from_rows(&[(Kick, "X.xX..X.xX.XxXXX"), (Snare, "....X.......X...")]),
        Pattern::from_rows(&[
            (Kick, "X...X...X...X..."),
            (ClosedHat, "3344556677889XX."),
            (OpenHat, "...............X"),
        ]),
    ]
}

/// How much a hit holds the groove together, 0..=1: density keeps the hits at or above
/// `1 - density`. Kicks on the beat are the backbone (1.0, kept even at density 0); then the
/// voice, its place in the bar and its velocity decide.
#[must_use]
pub fn importance(voice: DrumVoice, step: usize, velocity: f32) -> f32 {
    if voice == DrumVoice::Kick && step.is_multiple_of(4) {
        return 1.0;
    }
    let voice_weight = match voice {
        DrumVoice::Kick => 1.0,
        DrumVoice::Snare => 0.9,
        DrumVoice::ClosedHat => 0.8,
        DrumVoice::Clap => 0.75,
        DrumVoice::OpenHat => 0.65,
        DrumVoice::Perc => 0.5,
    };
    let place = match step % 16 {
        0 => 1.0,
        8 => 0.95,
        s if s.is_multiple_of(4) => 0.85,
        s if s.is_multiple_of(2) => 0.6,
        _ => 0.4,
    };
    voice_weight * place * (0.6 + 0.4 * velocity.clamp(0.0, 1.0))
}

/// Mute/trigger bit of `voice` (its index in `DrumVoice::ALL`).
#[must_use]
pub fn voice_bit(voice: DrumVoice) -> u8 {
    DrumVoice::ALL
        .iter()
        .position(|&v| v == voice)
        .map_or(0, |i| 1 << i)
}

/// Beat of `step`: inside each swung pair (two 8ths or two 16ths), the second note moves from
/// half-way to `swing`, and the 16ths in between follow linearly.
fn step_beats(step: u64, swing: f32, grid: SwingGrid) -> f64 {
    let pair = match grid {
        SwingGrid::Eighth => 1.0,
        SwingGrid::Sixteenth => 0.5,
    };
    let straight = step as f64 / 4.0;
    let start = (straight / pair).floor() * pair;
    let q = (straight - start) / pair;
    let s = f64::from(swing);
    let warped = if q < 0.5 {
        q * 2.0 * s
    } else {
        s + (q - 0.5) * 2.0 * (1.0 - s)
    };
    start + warped * pair
}

/// A fill replacing the pattern on steps `start..end` (counted from the transport start).
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveFill {
    index: u8,
    start: u64,
    end: u64,
}

#[derive(Debug, Clone)]
pub struct Sequencer {
    patterns: [Pattern; PATTERNS],
    fills: [Pattern; PATTERNS],
    fill: Option<ActiveFill>,
    /// Bit `i` set = voice `DrumVoice::ALL[i]` muted.
    mutes: u8,
    /// 0 = kicks on the beat only … 1 = every step.
    density: f32,
    current: u8,
    pending: Option<u8>,
    /// Index of the next step to play, counted from the transport start.
    next_step: u64,
    swing: f32,
    grid: SwingGrid,
    rng: u32,
    /// Off: steps still advance (the bar position stays right) but play nothing.
    enabled: bool,
    bass: BassLine,
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
            fills: default_fills(),
            fill: None,
            mutes: 0,
            density: 1.0,
            current: 0,
            pending: None,
            next_step: 0,
            swing: 0.54,
            grid: SwingGrid::Eighth,
            rng: 0x9E37_79B9,
            enabled: true,
            bass: BassLine::new(),
        }
    }

    /// Back to step 0 (transport start); a pending pattern starts right away.
    pub fn reset(&mut self) {
        self.next_step = 0;
        self.fill = None;
        if let Some(p) = self.pending.take() {
            self.current = p;
        }
        self.bass.reset();
    }

    /// Queues pattern `index` (0-based) and its bass line for the next bar, or switches now if the
    /// transport is stopped.
    pub fn queue(&mut self, index: u8, playing: bool) {
        if usize::from(index) >= PATTERNS {
            return;
        }
        self.bass.queue(index, playing);
        if playing && index != self.current {
            self.pending = Some(index);
        } else {
            self.current = index;
            self.pending = None;
        }
    }

    /// The bass line, its key and mute.
    #[must_use]
    pub fn bass(&self) -> &BassLine {
        &self.bass
    }

    pub fn bass_mut(&mut self) -> &mut BassLine {
        &mut self.bass
    }

    /// Plays fill `index` (0-based) from the next beat to the end of the bar; ignored when stopped.
    pub fn queue_fill(&mut self, index: u8, playing: bool) {
        if !playing || usize::from(index) >= PATTERNS {
            return;
        }
        let start = self.next_step.div_ceil(4) * 4;
        let end = (start / STEPS as u64 + 1) * STEPS as u64;
        self.fill = Some(ActiveFill { index, start, end });
    }

    /// Fill waiting or playing, and whether it has started.
    #[must_use]
    pub fn fill(&self) -> Option<(u8, bool)> {
        self.fill.map(|f| (f.index, self.next_step > f.start))
    }

    /// Muted voices, bit `i` for `DrumVoice::ALL[i]`; applies from the next step.
    pub fn set_mutes(&mut self, mutes: u8) {
        self.mutes = mutes;
    }

    #[must_use]
    pub fn mutes(&self) -> u8 {
        self.mutes
    }

    pub fn set_density(&mut self, density: f32) {
        if density.is_finite() {
            self.density = density.clamp(0.0, 1.0);
        }
    }

    #[must_use]
    pub fn density(&self) -> f32 {
        self.density
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

    fn step_beats(&self, step: u64) -> f64 {
        step_beats(step, self.swing, self.grid)
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

    /// Plays the step returned by `next_step_within` (its sample is reached) on the drums and the
    /// bass, and moves on. Returns the drum voices triggered (bit `i` = `DrumVoice::ALL[i]`).
    pub fn fire(&mut self, kit: &mut DrumKit, bass: &mut BassVoice, clock: &Clock) -> u8 {
        let mut played = 0;
        let step = (self.next_step % STEPS as u64) as usize;
        if step == 0
            && let Some(p) = self.pending.take()
        {
            self.current = p;
        }
        if self.fill.is_some_and(|f| self.next_step >= f.end) {
            self.fill = None;
        }
        // Fills are a deliberate gesture: density does not thin them.
        let (pattern, thinned) = match self.fill {
            Some(f) if self.next_step >= f.start => (self.fills.get(usize::from(f.index)), false),
            _ => (self.patterns.get(usize::from(self.current)), true),
        };
        if self.enabled
            && let Some(pattern) = pattern
        {
            let rows = pattern.steps;
            let threshold = 1.0 - self.density - 1.0e-6;
            for (i, (voice, row)) in DrumVoice::ALL.into_iter().zip(rows).enumerate() {
                let Some(cell) = row.get(step).copied() else {
                    continue;
                };
                let muted = self.mutes & (1 << i) != 0;
                let thinned_out = thinned && importance(voice, step, cell.velocity) < threshold;
                if cell.velocity > 0.0
                    && !muted
                    && !thinned_out
                    && (cell.probability >= 1.0 || self.chance() < cell.probability)
                {
                    kit.trigger(voice, cell.velocity);
                    played |= 1 << i;
                }
            }
        }
        let (n, swing, grid) = (self.next_step, self.swing, self.grid);
        let gate_of = |len: u8, held: bool| {
            let from = clock.sample_at(step_beats(n, swing, grid));
            let to = clock.sample_at(step_beats(n + u64::from(len), swing, grid));
            gate_samples(to.saturating_sub(from), held)
        };
        self.bass.fire(step, bass, self.enabled, gate_of);
        self.next_step += 1;
        played
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Steps fired while running the clock for `frames`, as (sample, step index in bar).
    fn run(seq: &mut Sequencer, clock: &mut Clock, frames: usize) -> Vec<(u64, u8)> {
        let mut kit = DrumKit::new(48_000);
        let mut bass = BassVoice::new(48_000);
        let mut fired = Vec::new();
        let mut left = frames;
        while left > 0 {
            match seq.next_step_within(clock, left) {
                Some(offset) => {
                    clock.advance(offset);
                    left -= offset;
                    fired.push((clock.position(), seq.step_in_bar()));
                    seq.fire(&mut kit, &mut bass, clock);
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

    /// Voices played on each of the next `steps` steps (transport running at 120 BPM).
    fn played(seq: &mut Sequencer, steps: usize) -> Vec<u8> {
        let mut clock = started(120.0);
        let mut kit = DrumKit::new(48_000);
        let mut bass = BassVoice::new(48_000);
        let mut out = Vec::new();
        while out.len() < steps {
            match seq.next_step_within(&clock, 100_000) {
                Some(offset) => {
                    clock.advance(offset);
                    out.push(seq.fire(&mut kit, &mut bass, &clock));
                }
                None => break,
            }
        }
        out
    }

    #[test]
    fn mutes_silence_their_voices_only() {
        let mut seq = Sequencer::new();
        seq.set_mutes(voice_bit(DrumVoice::Kick));
        let bar = played(&mut seq, 16);
        assert!(bar.iter().all(|v| v & voice_bit(DrumVoice::Kick) == 0));
        assert_ne!(bar[4] & voice_bit(DrumVoice::Snare), 0, "snare still plays");
    }

    #[test]
    fn density_zero_keeps_kicks_on_the_beat_and_one_keeps_all() {
        let kick = voice_bit(DrumVoice::Kick);
        let mut seq = Sequencer::new();
        seq.queue(1, false); // groove B: kicks on and off the beat
        seq.set_density(0.0);
        let bar = played(&mut seq, 16);
        for (step, voices) in bar.iter().enumerate() {
            let on_beat_kick = DEFAULT_KICK_B[step] == b'X' || DEFAULT_KICK_B[step] == b'x';
            let expect = if step % 4 == 0 && on_beat_kick {
                kick
            } else {
                0
            };
            assert_eq!(*voices, expect, "step {step}");
        }
        let mut full = Sequencer::new();
        full.queue(1, false);
        let all = played(&mut full, 16);
        assert!(all.iter().map(|v| v.count_ones()).sum::<u32>() > 20);
    }

    #[test]
    fn density_removes_hits_in_order_of_importance() {
        let mut counts = Vec::new();
        for d in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut seq = Sequencer::new();
            seq.set_density(d);
            counts.push(
                played(&mut seq, 16)
                    .iter()
                    .map(|v| v.count_ones())
                    .sum::<u32>(),
            );
        }
        assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
        assert!(counts[0] < counts[4]);
        // Snare backbeats come back before offbeat hats.
        assert!(importance(DrumVoice::Snare, 4, 1.0) > importance(DrumVoice::ClosedHat, 2, 0.75));
    }

    #[test]
    fn fill_runs_from_the_next_beat_to_the_bar_end_then_gives_back() {
        let snare = voice_bit(DrumVoice::Snare);
        let mut seq = Sequencer::new();
        seq.set_swing(0.5);
        let mut clock = started(120.0);
        run(&mut seq, &mut clock, 6_000 * 5 + 10); // steps 0–5 played, next is 6
        seq.queue_fill(0, true);
        assert_eq!(seq.fill(), Some((0, false)), "waits for beat 3 (step 8)");
        let mut kit = DrumKit::new(48_000);
        let mut bass = BassVoice::new(48_000);
        let mut voices = Vec::new();
        for _ in 0..12 {
            let offset = seq.next_step_within(&clock, 100_000).unwrap();
            clock.advance(offset);
            voices.push((seq.step_in_bar(), seq.fire(&mut kit, &mut bass, &clock)));
        }
        // Steps 6–7: pattern A (closed hat on 6); 8–15: snare roll; then pattern A again.
        assert_eq!(voices[0], (6, voice_bit(DrumVoice::ClosedHat)));
        assert!(voices[2..10].iter().filter(|(_, v)| v & snare != 0).count() >= 6);
        assert_eq!(voices[10].0, 0);
        assert_eq!(seq.fill(), None);
        assert_eq!(
            voices[10].1,
            voice_bit(DrumVoice::Kick) | voice_bit(DrumVoice::ClosedHat)
        );
    }

    #[test]
    fn fill_is_ignored_when_stopped() {
        let mut seq = Sequencer::new();
        seq.queue_fill(2, false);
        assert_eq!(seq.fill(), None);
    }

    /// Kick row of pattern B (kept in sync with `default_patterns`).
    const DEFAULT_KICK_B: &[u8; 16] = b"X.....x...X.....";

    #[test]
    fn text_rows_parse_velocity_and_chance() {
        let p = Pattern::from_rows(&[(DrumVoice::Snare, "Xxo?.9")]);
        let row = p.steps[1];
        assert_eq!(row[5].velocity, 1.0);
        assert_eq!(row[0].velocity, 1.0);
        assert_eq!(row[1].velocity, 0.75);
        assert_eq!(row[2].velocity, 0.4);
        assert_eq!(row[3].probability, 0.5);
        assert_eq!(row[4].velocity, 0.0);
        assert_eq!(p.steps[0][0], Step::default(), "other voices stay empty");
    }

    #[test]
    fn patterns_bring_their_bass_line_which_can_change_alone() {
        let mut seq = Sequencer::new();
        seq.queue(2, true);
        assert_eq!(seq.bass().pending(), Some(2), "coupled to the drum pattern");
        seq.bass_mut().queue(3, true);
        assert_eq!(seq.pending(), Some(2));
        assert_eq!(seq.bass().pending(), Some(3));
        seq.reset();
        assert_eq!((seq.current(), seq.bass().current()), (2, 3));
    }

    #[test]
    fn bass_gate_follows_the_note_length_and_the_swing() {
        // Line A starts with degree 1 held 4 steps: one beat at 120 BPM = 24 000 samples.
        let mut seq = Sequencer::new();
        let clock = started(120.0);
        let mut kit = DrumKit::new(48_000);
        let mut bass = BassVoice::new(48_000);
        seq.fire(&mut kit, &mut bass, &clock);
        assert!(bass.gated());
        let mut out = vec![0.0; 19_150];
        bass.render_add(&mut out);
        assert!(bass.gated(), "80 % of the beat");
        let mut out = vec![0.0; 100];
        bass.render_add(&mut out);
        assert!(!bass.gated());
    }
}
