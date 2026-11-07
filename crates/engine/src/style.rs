//! Style presets: everything a style brings — drum patterns and fills, bass lines, kit and bass
//! sound, swing, synth patch, melody phrases and send effects. The tempo and the key stay the player's (choice of
//! 2026-11-02); `bpm` is only shown as a hint.
//!
//! Presets are written as text tables (`StyleText`) and built once, on the control thread, by
//! `Engine::new`; switching style on the audio thread only copies arrays.
//!
//! Runs on: control thread (building), audio thread (switching).

use crate::bass::{BassParams, BassPattern, default_bass_patterns};
use crate::drums::{DrumVoice, VoiceParams};
use crate::fx::DelayTime;
use crate::poly::{Adsr, SynthPatch, Wave};
use crate::sequencer::{PATTERNS, Pattern, SwingGrid, default_fills, default_patterns};

/// Number of kits (styles): 808, 909, Dub (R1).
pub const STYLES: usize = 3;

/// Drum rows of one pattern, in the text form of `Pattern::from_rows`.
type Rows = &'static [(DrumVoice, &'static str)];

/// A style as written below.
pub struct StyleText {
    pub name: &'static str,
    /// Suggested tempo (shown, not applied).
    pub bpm: f64,
    pub swing: f32,
    pub grid: SwingGrid,
    /// (tune in semitones, decay factor, level) per voice, in `DrumVoice::ALL` order.
    pub kit: [(f32, f32, f32); 6],
    pub bass: BassParams,
    /// Patch of the melody synth.
    pub synth: SynthPatch,
    pub fx: FxPreset,
    /// `None` = the patterns and fills of the hybrid kit.
    pub patterns: Option<[Rows; PATTERNS]>,
    pub fills: Option<[Rows; PATTERNS]>,
    /// `None` = the bass lines of the hybrid kit.
    pub bass_lines: Option<[&'static str; PATTERNS]>,
    /// Melody phrases of the Sampler pads B, in the notation of the bass lines; `None` = those of
    /// the hybrid kit.
    pub phrases: Option<[&'static str; PATTERNS]>,
}

/// Send effects of a style: delay time (until Shift + PFL B changes it) and reverb length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxPreset {
    pub delay: DelayTime,
    /// RT60 of the reverb, in seconds.
    pub reverb_decay: f32,
}

/// A style ready to be switched to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylePreset {
    pub name: &'static str,
    pub bpm: f64,
    pub swing: f32,
    pub grid: SwingGrid,
    pub kit: [VoiceParams; 6],
    pub bass: BassParams,
    pub synth: SynthPatch,
    pub fx: FxPreset,
    pub patterns: [Pattern; PATTERNS],
    pub fills: [Pattern; PATTERNS],
    pub bass_lines: [BassPattern; PATTERNS],
    pub phrases: [BassPattern; PATTERNS],
}

impl StylePreset {
    #[must_use]
    pub fn from_text(text: &StyleText) -> Self {
        let build = |rows: Option<[Rows; PATTERNS]>, default: [Pattern; PATTERNS]| {
            rows.map_or(default, |rows| rows.map(Pattern::from_rows))
        };
        Self {
            name: text.name,
            bpm: text.bpm,
            swing: text.swing,
            grid: text.grid,
            kit: text
                .kit
                .map(|(tune, decay, level)| VoiceParams { tune, decay, level }),
            bass: text.bass,
            synth: text.synth,
            fx: text.fx,
            patterns: build(text.patterns, default_patterns()),
            fills: build(text.fills, default_fills()),
            bass_lines: text
                .bass_lines
                .map_or_else(default_bass_patterns, |lines| lines.map(BassPattern::parse)),
            phrases: text
                .phrases
                .unwrap_or(DEFAULT_PHRASES)
                .map(BassPattern::parse),
        }
    }
}

/// Phrases of the hybrid kit: a falling answer, a repeated hook, a slow descent, offbeat thirds.
const DEFAULT_PHRASES: [&str; PATTERNS] = [
    "1 - 3 - 5 - 4 3 2 - . . 1 - . .",
    "5 . 5 6 5 . 3 . 4 . 4 5 4 . 2 .",
    "1' - 7 - 5 - 3 - 4 - 5 - 3 - - -",
    ". 3 . 5 . 3 . 1 . 3 . 5 . 6 5 .",
];

/// Every style, in the order of the Shift + jog scroll.
#[must_use]
pub fn presets() -> [StylePreset; STYLES] {
    TEXTS.each_ref().map(StylePreset::from_text)
}

/// Name of style `index` (for display).
#[must_use]
pub fn name(index: u8) -> &'static str {
    TEXTS.get(usize::from(index)).map_or("?", |t| t.name)
}

/// Suggested tempo of style `index`.
#[must_use]
pub fn bpm(index: u8) -> f64 {
    TEXTS.get(usize::from(index)).map_or(120.0, |t| t.bpm)
}

use DrumVoice::{Clap, ClosedHat, Kick, OpenHat, Perc, Snare};

/// A stab for dub chords: short, filtered, made to be thrown into the delay.
const DUB_STAB: SynthPatch = SynthPatch {
    wave1: Wave::Saw,
    wave2: Wave::Saw,
    detune: 6.0,
    cutoff: 950.0,
    resonance: 1.1,
    env_octaves: 2.5,
    amp: Adsr {
        attack: 0.003,
        decay: 0.28,
        sustain: 0.0,
        release: 0.22,
    },
    filter: Adsr {
        attack: 0.0,
        decay: 0.2,
        sustain: 0.0,
        release: 0.15,
    },
    level: 0.55,
};

/// Bass lines shared by the kits: offbeat roots, the dub techno pulse (degrees of the key).
const DUB_LINES: [&str; PATTERNS] = [
    ". . 1 . . . 1 . . . 1 . . . 1 .",
    ". . 1 - . . 1 . . . 5, - . . 1 .",
    ". . 1 . . 1 . . . . 4, . . 1 . .",
    ". . 1 - . . 1 - . . 7, - . . 5, -",
];

/// Phrases until the generated melody (R3) replaces them.
const DUB_PHRASES: [&str; PATTERNS] = [
    "5 - - - . . 3 - . . . . 4 - - -",
    ". . 1' - . . 7 - 5 - - - . . . .",
    "3 . . 3 . . 3 . 4 . . 4 . . 5 .",
    "1 - - - 3 - - - 5 - - - 4 - 3 -",
];

/// Fills shared by the kits (from the next beat to the end of the bar).
const DUB_FILLS: [Rows; PATTERNS] = [
    &[(Kick, "X...X...X...X..."), (Snare, "..........o.xxXX")],
    &[(Kick, "X...X...X...X..."), (Perc, "X..X..X..X..XXXX")],
    // Drop: the first kick, then silence until the bar.
    &[(Kick, "X...............")],
    &[
        (Kick, "X...X...X...X..."),
        (ClosedHat, "3344556677889XX."),
        (OpenHat, "...............X"),
    ],
];

/// The three kits of the groovebox (R1, 2026-11-07): every pattern keeps the kick on each beat.
pub const TEXTS: [StyleText; STYLES] = [
    // Deep, long kick and a soft clap: the analogue boom.
    StyleText {
        name: "808 · boom",
        bpm: 124.0,
        swing: 0.54,
        grid: SwingGrid::Sixteenth,
        kit: [
            (-3.0, 1.7, 1.0),
            (0.0, 0.8, 0.7),
            (0.0, 0.7, 0.65),
            (0.0, 1.0, 0.6),
            (0.0, 1.2, 0.85),
            (4.0, 0.7, 0.7),
        ],
        bass: BassParams {
            saw: 0.2,
            sub: 0.8,
            resonance: 0.7,
            env_octaves: 1.8,
            accent_octaves: 2.5,
            decay: 0.35,
            glide: 0.05,
            drive: 1.3,
        },
        synth: DUB_STAB,
        fx: FxPreset {
            delay: DelayTime::DottedEighth,
            reverb_decay: 3.5,
        },
        patterns: Some([
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....x.......x..."),
                (ClosedHat, "..x...x...x...x."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....x.......x..."),
                (ClosedHat, "x.xox.xox.xox.xo"),
                (OpenHat, "..............x."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....x.......x..o"),
                (ClosedHat, "..x...x...x...x."),
                (Perc, "...o..o....o..o."),
            ],
            &[
                (Kick, "X...X...X...X.o."),
                (Snare, "....x.......x..."),
                (ClosedHat, "xxoxxxoxxxoxxxox"),
                (OpenHat, "..x...x...x...x."),
                (Perc, "......o.......o."),
            ],
        ]),
        fills: Some(DUB_FILLS),
        bass_lines: Some(DUB_LINES),
        phrases: Some(DUB_PHRASES),
    },
    // Punchy kick, open hats on the offbeat: straight techno.
    StyleText {
        name: "909 · techno",
        bpm: 128.0,
        swing: 0.52,
        grid: SwingGrid::Sixteenth,
        kit: [
            (0.0, 1.0, 1.0),
            (1.0, 0.8, 0.8),
            (2.0, 0.6, 0.6),
            (1.0, 1.1, 0.75),
            (0.0, 0.9, 0.8),
            (5.0, 0.6, 0.75),
        ],
        bass: BassParams {
            saw: 0.55,
            sub: 0.45,
            resonance: 1.4,
            env_octaves: 3.0,
            accent_octaves: 4.0,
            decay: 0.18,
            glide: 0.04,
            drive: 1.6,
        },
        synth: DUB_STAB,
        fx: FxPreset {
            delay: DelayTime::DottedEighth,
            reverb_decay: 2.8,
        },
        patterns: Some([
            &[(Kick, "X...X...X...X..."), (OpenHat, "..x...x...x...x.")],
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....x.......x..."),
                (ClosedHat, ".x.x.x.x.x.x.x.x"),
                (OpenHat, "..x...x...x...x."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....x.......x..."),
                (OpenHat, "..x...x...x...x."),
                (Perc, "...x..x...x..x.."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "....x..o....x.o."),
                (ClosedHat, "xoxoxoxoxoxoxoxo"),
                (OpenHat, "..x...x...x...x."),
            ],
        ]),
        fills: Some(DUB_FILLS),
        bass_lines: Some(DUB_LINES),
        phrases: Some(DUB_PHRASES),
    },
    // Muffled kick, rimshot (high, short snare), felt hats, a lot of space.
    StyleText {
        name: "Dub",
        bpm: 118.0,
        swing: 0.56,
        grid: SwingGrid::Sixteenth,
        kit: [
            (-1.0, 1.3, 0.95),
            (7.0, 0.35, 0.7),
            (-2.0, 0.6, 0.5),
            (-2.0, 0.9, 0.5),
            (0.0, 1.0, 0.6),
            (3.0, 0.8, 0.7),
        ],
        bass: BassParams {
            saw: 0.3,
            sub: 0.65,
            resonance: 0.9,
            env_octaves: 2.0,
            accent_octaves: 3.0,
            decay: 0.25,
            glide: 0.06,
            drive: 1.4,
        },
        synth: DUB_STAB,
        fx: FxPreset {
            delay: DelayTime::DottedEighth,
            reverb_decay: 5.0,
        },
        patterns: Some([
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "......o.......o."),
                (ClosedHat, "..o...o...o...o."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "....o.......o..."),
                (ClosedHat, "..x...x...x...x."),
                (Perc, "...o.....o......"),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Snare, ".......o.......o"),
                (ClosedHat, "x.ox.xo.x.ox.xo."),
                (OpenHat, "..........x....."),
            ],
            &[
                (Kick, "X...X...X...X..o"),
                (Snare, "....x..o....x..."),
                (ClosedHat, "..x...x...x...x."),
                (OpenHat, "......x.......x."),
                (Perc, ".o....o..o....o."),
            ],
        ]),
        fills: Some(DUB_FILLS),
        bass_lines: Some(DUB_LINES),
        phrases: Some(DUB_PHRASES),
    },
];

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::sequencer::STEPS;

    #[test]
    fn every_row_and_line_is_one_bar() {
        for style in &TEXTS {
            for rows in style.patterns.iter().chain(style.fills.iter()).flatten() {
                for (voice, text) in *rows {
                    // A fill may stop early (the rest of the bar is silent).
                    assert!(
                        text.chars().count() <= STEPS,
                        "{} {voice:?} {text}",
                        style.name
                    );
                }
            }
            if let Some(patterns) = style.patterns {
                for rows in patterns {
                    for (voice, text) in rows {
                        assert_eq!(
                            text.chars().count(),
                            STEPS,
                            "{} {voice:?} {text}",
                            style.name
                        );
                    }
                }
            }
            for line in style.bass_lines.iter().flatten() {
                assert_eq!(
                    line.split_whitespace().count(),
                    STEPS,
                    "{} {line}",
                    style.name
                );
            }
        }
    }

    #[test]
    fn every_pattern_keeps_the_kick_on_each_beat() {
        for style in presets() {
            for pattern in &style.patterns {
                for beat in [0, 4, 8, 12] {
                    assert!(pattern.steps[0][beat].velocity > 0.0, "{}", style.name);
                }
            }
        }
    }

    #[test]
    fn every_style_plays_kicks_and_bass() {
        for (i, style) in presets().iter().enumerate() {
            for (p, pattern) in style.patterns.iter().enumerate() {
                assert!(
                    pattern.steps[0].iter().any(|s| s.velocity > 0.0),
                    "{} pattern {p}: no kick",
                    name(i as u8)
                );
            }
            assert!(
                style
                    .bass_lines
                    .iter()
                    .all(|l| l.steps.iter().any(Option::is_some))
            );
            assert!((0.5..=0.75).contains(&style.swing));
        }
    }
}
