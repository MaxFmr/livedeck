//! Style presets: everything a style brings to the rhythm deck — drum patterns and fills, bass
//! lines, kit and bass sound, swing. The tempo and the key stay the player's (choice of
//! 2026-11-02); `bpm` is only shown as a hint.
//!
//! Presets are written as text tables (`StyleText`) and built once, on the control thread, by
//! `Engine::new`; switching style on the audio thread only copies arrays.
//!
//! Runs on: control thread (building), audio thread (switching).

use crate::bass::{BassParams, BassPattern, default_bass_patterns};
use crate::drums::{DrumVoice, VoiceParams};
use crate::poly::{Adsr, SynthPatch, Wave};
use crate::sequencer::{PATTERNS, Pattern, SwingGrid, default_fills, default_patterns};

/// Number of styles: the hybrid kit of 010–013, then the five starting presets.
pub const STYLES: usize = 6;

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
    /// `None` = the patterns and fills of the hybrid kit.
    pub patterns: Option<[Rows; PATTERNS]>,
    pub fills: Option<[Rows; PATTERNS]>,
    /// `None` = the bass lines of the hybrid kit.
    pub bass_lines: Option<[&'static str; PATTERNS]>,
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
    pub patterns: [Pattern; PATTERNS],
    pub fills: [Pattern; PATTERNS],
    pub bass_lines: [BassPattern; PATTERNS],
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
            patterns: build(text.patterns, default_patterns()),
            fills: build(text.fills, default_fills()),
            bass_lines: text
                .bass_lines
                .map_or_else(default_bass_patterns, |lines| lines.map(BassPattern::parse)),
        }
    }
}

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

const NEUTRAL: (f32, f32, f32) = (0.0, 1.0, 1.0);

use DrumVoice::{Clap, ClosedHat, Kick, OpenHat, Perc, Snare};

pub const TEXTS: [StyleText; STYLES] = [
    StyleText {
        name: "Hybride",
        bpm: 120.0,
        swing: 0.54,
        grid: SwingGrid::Eighth,
        kit: [NEUTRAL; 6],
        bass: BassParams {
            saw: 0.5,
            sub: 0.45,
            resonance: 1.1,
            env_octaves: 2.5,
            accent_octaves: 4.0,
            decay: 0.2,
            glide: 0.045,
            drive: 1.4,
        },
        synth: SynthPatch {
            wave1: Wave::Saw,
            wave2: Wave::Saw,
            detune: 8.0,
            cutoff: 1_200.0,
            resonance: 0.9,
            env_octaves: 1.5,
            amp: Adsr {
                attack: 0.25,
                decay: 0.5,
                sustain: 0.8,
                release: 0.6,
            },
            filter: Adsr {
                attack: 0.3,
                decay: 0.8,
                sustain: 0.4,
                release: 0.6,
            },
            level: 0.5,
        },
        patterns: None,
        fills: None,
        bass_lines: None,
    },
    // Half-time at 140: the snare lands on beat 3 (step 8). Deep kick, sub-heavy bass, long notes.
    StyleText {
        name: "Dub / steppa",
        bpm: 140.0,
        swing: 0.5,
        grid: SwingGrid::Eighth,
        kit: [
            (-2.0, 1.4, 1.0),
            (-1.0, 1.2, 0.9),
            (0.0, 0.8, 0.7),
            (0.0, 1.2, 0.7),
            (0.0, 1.0, 0.6),
            (2.0, 0.8, 0.9),
        ],
        bass: BassParams {
            saw: 0.25,
            sub: 0.7,
            resonance: 0.8,
            env_octaves: 1.5,
            accent_octaves: 2.5,
            decay: 0.3,
            glide: 0.06,
            drive: 1.6,
        },
        // Skank: short, bright, nothing held.
        synth: SynthPatch {
            wave1: Wave::Square,
            wave2: Wave::Saw,
            detune: 5.0,
            cutoff: 1_800.0,
            resonance: 1.2,
            env_octaves: 2.0,
            amp: Adsr {
                attack: 0.002,
                decay: 0.18,
                sustain: 0.0,
                release: 0.08,
            },
            filter: Adsr {
                attack: 0.0,
                decay: 0.12,
                sustain: 0.0,
                release: 0.1,
            },
            level: 0.6,
        },
        patterns: Some([
            // One drop.
            &[
                (Kick, "........X......."),
                (Snare, "........X......."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (Perc, "....o.......o..."),
            ],
            // Steppa: kick on every beat.
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "........X......."),
                (ClosedHat, "x.xox.xox.xox.xo"),
                (OpenHat, "..............x."),
            ],
            // Rockers.
            &[
                (Kick, "X.....X.X......."),
                (Snare, "....X.......X..."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (OpenHat, "..............x."),
                (Perc, "......o.......o."),
            ],
            // Dub break.
            &[
                (Kick, "X......X..X....."),
                (Snare, "....X..o....X.o."),
                (ClosedHat, "xxxoxxxoxxxoxxxo"),
                (Perc, "..o.....o..o...."),
            ],
        ]),
        fills: Some([
            &[(Kick, "X..............."), (Snare, "......X...X.X.XX")],
            &[(Kick, "X......."), (Perc, "X..X..X..X..XXXX")],
            // Drop: only the first kick, then silence until the bar.
            &[(Kick, "X...............")],
            &[
                (ClosedHat, "3344556677889XX."),
                (OpenHat, "...............X"),
            ],
        ]),
        bass_lines: Some([
            "1! - - - - - . . 1 - - . 5, - 7, -",
            "1! - . 1 . . 1 . 3 - . 3 . . 5, .",
            "1! - - . 1 . 5, . 7, - - . 1 - 3 .",
            "1! - . . 4 - . . 5 - . . 1 - 7, -",
        ]),
    },
    // Swung 16ths, congas on the perc, melodic bass.
    StyleText {
        name: "Afrobeat / reggae",
        bpm: 108.0,
        swing: 0.58,
        grid: SwingGrid::Sixteenth,
        kit: [
            (0.0, 0.8, 1.0),
            (1.0, 0.8, 0.85),
            (0.0, 0.9, 0.8),
            (0.0, 1.0, 0.7),
            (0.0, 1.0, 0.7),
            (3.0, 1.2, 1.0),
        ],
        bass: BassParams {
            saw: 0.45,
            sub: 0.5,
            resonance: 1.0,
            env_octaves: 2.0,
            accent_octaves: 3.0,
            decay: 0.18,
            glide: 0.04,
            drive: 1.4,
        },
        // Organ: squares, little filter movement, held.
        synth: SynthPatch {
            wave1: Wave::Square,
            wave2: Wave::Square,
            detune: 3.0,
            cutoff: 2_500.0,
            resonance: 0.7,
            env_octaves: 0.5,
            amp: Adsr {
                attack: 0.005,
                decay: 0.1,
                sustain: 0.9,
                release: 0.08,
            },
            filter: Adsr {
                attack: 0.0,
                decay: 0.2,
                sustain: 0.8,
                release: 0.1,
            },
            level: 0.45,
        },
        patterns: Some([
            // Afrobeat.
            &[
                (Kick, "X..x..X...X..x.."),
                (Snare, "....X..o.o..X..."),
                (ClosedHat, "xxXxxxXxxxXxxxXx"),
                (Perc, "..x..x.x..x..x.x"),
            ],
            // Roots reggae: one drop with an offbeat rim (the skank's place).
            &[
                (Kick, "........X......."),
                (Snare, "........X......."),
                (ClosedHat, "x.xxx.xxx.xxx.xx"),
                (Perc, "..x...x...x...x."),
            ],
            // Highlife: 3-3-4-3-3 clave on the perc.
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "....x.......x..o"),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (OpenHat, "..x...x...x...x."),
                (Perc, "x..x..x...x..x.."),
            ],
            // Busy.
            &[
                (Kick, "X..x.xX...X..x.."),
                (Snare, "..o.X..o.o.oX.o."),
                (ClosedHat, "XoxoXoxoXoxoXoxo"),
                (Perc, "x.xx.x.xx.x.xx.x"),
            ],
        ]),
        fills: Some([
            &[(Kick, "X..............."), (Snare, "....x.x.xxxx6789")],
            &[(Kick, "X.......X......."), (Perc, "x.xxx.xxx.xxXXXX")],
            &[(Kick, "X...X...X.X.XXXX"), (Snare, "X...X...X.X.XXXX")],
            &[
                (Kick, "X...X...X...X..."),
                (ClosedHat, "3344556677889XX."),
                (OpenHat, "...............X"),
            ],
        ]),
        bass_lines: Some([
            "1 . . 1' . 7 . 5 . . 4 . 5 . 7 .",
            "1! - . . . . . . 1 . 3 . 5 . 3 .",
            "1 . 3 . 5 . 3 . 4 . 6 . 5 . 3 .",
            "1! . 1' . 7 . 5 4 . 3 . 1 . 3 4 5",
        ]),
    },
    // 2-step: shuffled 16ths, short kick, loud clap, short punchy bass notes.
    StyleText {
        name: "UK garage",
        bpm: 132.0,
        swing: 0.62,
        grid: SwingGrid::Sixteenth,
        kit: [
            (1.0, 0.6, 1.0),
            (2.0, 0.7, 0.8),
            (0.0, 0.7, 0.85),
            (0.0, 0.9, 0.8),
            (0.0, 1.0, 1.0),
            (5.0, 0.6, 0.8),
        ],
        bass: BassParams {
            saw: 0.6,
            sub: 0.4,
            resonance: 1.3,
            env_octaves: 2.5,
            accent_octaves: 4.0,
            decay: 0.1,
            glide: 0.03,
            drive: 1.8,
        },
        // Stab: resonant filter snapping shut.
        synth: SynthPatch {
            wave1: Wave::Saw,
            wave2: Wave::Saw,
            detune: 12.0,
            cutoff: 900.0,
            resonance: 1.6,
            env_octaves: 3.0,
            amp: Adsr {
                attack: 0.002,
                decay: 0.25,
                sustain: 0.2,
                release: 0.15,
            },
            filter: Adsr {
                attack: 0.0,
                decay: 0.15,
                sustain: 0.0,
                release: 0.1,
            },
            level: 0.55,
        },
        patterns: Some([
            // 2-step.
            &[
                (Kick, "X.........X..x.."),
                (Clap, "....X.......X..."),
                (ClosedHat, "xox.xoxxxox.xoxx"),
                (OpenHat, "..x.......x....."),
                (Perc, "...o..o....o..o."),
            ],
            &[
                (Kick, "X......x..X....."),
                (Clap, "....X.......X..."),
                (Snare, "..........o...o."),
                (ClosedHat, "x.xxx.xxx.xxx.xx"),
                (Perc, "..o..o..o..o..o."),
            ],
            // Four to the floor (speed garage).
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....X.......X..."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (OpenHat, "..x...x...x...x."),
            ],
            // Broken.
            &[
                (Kick, "X..x......X..x.."),
                (Snare, "....X..o....X.oo"),
                (Clap, "....X.......X..."),
                (ClosedHat, "xoxxxoxxxoxxxoxx"),
                (Perc, "x.o.x.o.x.o.x.o."),
            ],
        ]),
        fills: None,
        bass_lines: Some([
            "1! . . . . . . 1 . . 1' . . 7 . .",
            "1! . . 3 . . 1 . . . 5, . 7, . 1 .",
            ". . 1! . . . 1 . . . 1' . . . 7 .",
            "1! . 1 . . 4 . . 3 . . 1 . 7, ~1 .",
        ]),
    },
    // Heavy, darker kit; long warm bass notes.
    StyleText {
        name: "Lo-fi / hip-hop",
        bpm: 86.0,
        swing: 0.6,
        grid: SwingGrid::Sixteenth,
        kit: [
            (-1.0, 1.1, 1.0),
            (-2.0, 1.1, 1.0),
            (-2.0, 0.6, 0.6),
            (-2.0, 0.8, 0.6),
            (0.0, 1.0, 0.5),
            (-3.0, 1.0, 0.8),
        ],
        bass: BassParams {
            saw: 0.35,
            sub: 0.6,
            resonance: 0.8,
            env_octaves: 1.2,
            accent_octaves: 2.0,
            decay: 0.3,
            glide: 0.06,
            drive: 1.2,
        },
        // Soft keys: dark, slow decay.
        synth: SynthPatch {
            wave1: Wave::Saw,
            wave2: Wave::Square,
            detune: 4.0,
            cutoff: 700.0,
            resonance: 0.8,
            env_octaves: 1.5,
            amp: Adsr {
                attack: 0.01,
                decay: 0.9,
                sustain: 0.3,
                release: 0.4,
            },
            filter: Adsr {
                attack: 0.0,
                decay: 0.5,
                sustain: 0.2,
                release: 0.4,
            },
            level: 0.55,
        },
        patterns: Some([
            // Boom bap.
            &[
                (Kick, "X......x..X....."),
                (Snare, "....X.......X..."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
            ],
            &[
                (Kick, "X.x.......X..x.."),
                (Snare, "....X.......X..o"),
                (ClosedHat, "xox.xox.xox.xoxo"),
                (Perc, "......o.......o."),
            ],
            &[
                (Kick, "X......X.XX....."),
                (Snare, "....X.......X..."),
                (ClosedHat, "x.xxx.xxx.xxx.xx"),
                (OpenHat, "..............x."),
            ],
            &[
                (Kick, "X..x...x..X..x.."),
                (Snare, "....X..o.o..X..."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (Clap, "....x.......x..."),
                (Perc, "..o....o.....o.."),
            ],
        ]),
        fills: None,
        bass_lines: Some([
            "1 - - . . . . 3 5 - . . 4 - 3 .",
            "1 . . 1 . . 7, - . . 5, - 6, - 7, .",
            "1 - - - 3 - . . 4 - - - 3 - 2 .",
            "1 . . 5, 7, . 1 . . 3 . . 4 . 3 .",
        ]),
    },
    // Four to the floor, open hats on the offbeats, offbeat or rolling bass.
    StyleText {
        name: "House / techno",
        bpm: 124.0,
        swing: 0.52,
        grid: SwingGrid::Sixteenth,
        kit: [
            (0.0, 0.9, 1.0),
            (0.0, 1.0, 0.8),
            (0.0, 1.0, 0.8),
            (0.0, 1.0, 0.9),
            (0.0, 1.0, 1.0),
            (4.0, 1.0, 0.8),
        ],
        bass: BassParams {
            saw: 0.55,
            sub: 0.45,
            resonance: 1.2,
            env_octaves: 2.5,
            accent_octaves: 4.0,
            decay: 0.15,
            glide: 0.04,
            drive: 1.5,
        },
        // House stab.
        synth: SynthPatch {
            wave1: Wave::Saw,
            wave2: Wave::Saw,
            detune: 10.0,
            cutoff: 1_000.0,
            resonance: 1.4,
            env_octaves: 3.0,
            amp: Adsr {
                attack: 0.002,
                decay: 0.3,
                sustain: 0.0,
                release: 0.12,
            },
            filter: Adsr {
                attack: 0.0,
                decay: 0.2,
                sustain: 0.0,
                release: 0.1,
            },
            level: 0.55,
        },
        patterns: Some([
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....X.......X..."),
                (ClosedHat, "x.x.x.x.x.x.x.x."),
                (OpenHat, "..x...x...x...x."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Clap, "....X.......X..."),
                (ClosedHat, "XoxoXoxoXoxoXoxo"),
                (OpenHat, "..x...x...x...x."),
                (Perc, "...o.....o....o."),
            ],
            // Techno.
            &[
                (Kick, "X...X...X...X..x"),
                (Clap, "............X..."),
                (ClosedHat, "..x...x...x...x."),
                (Perc, "x..x..x..x..x..."),
            ],
            &[
                (Kick, "X...X...X...X..."),
                (Snare, "....X.......X..o"),
                (Clap, "....X.......X..."),
                (ClosedHat, "xxXxxxXxxxXxxxXx"),
                (OpenHat, "..x...x...x...x."),
                (Perc, "x.?..?.x.?..?.x."),
            ],
        ]),
        fills: None,
        bass_lines: Some([
            ". . 1! . . . 1 . . . 1! . . . 1 .",
            ". . 1! 1 . . 1 . . . 1! 1 . 7, ~1 .",
            ". 1 1 1 . 1 1 1 . 1 1 1 . 1 1 1",
            "1! . . 1 . . 3 . . 1 . . 5, . 7, .",
        ]),
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
    fn hybrid_is_the_sound_of_before() {
        let hybrid = presets()[0];
        assert_eq!(hybrid.patterns, default_patterns());
        assert_eq!(hybrid.bass_lines, default_bass_patterns());
        assert_eq!(hybrid.bass, BassParams::default());
        assert!(hybrid.kit.iter().all(|p| *p == VoiceParams::default()));
        assert_eq!(hybrid.synth, SynthPatch::default());
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
