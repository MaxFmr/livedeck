//! What the control thread derives for the window from the engine snapshot, the style presets and
//! the LED bank (runs on: control thread).

use engine::bass::BassPattern;
use engine::sequencer::Pattern;
use io::controller::{Deck, Led, PadMode};
use io::leds::{LedBank, LedState};

use crate::ui::{BassCell, Light};

/// The pattern as drawn: per voice × step, 0 none, 1 ghost, 2 hit, 3 accent.
pub fn drum_grid(pattern: &Pattern) -> [[u8; 16]; 6] {
    pattern.steps.map(|voice| {
        voice.map(|step| match step.velocity {
            v if v >= 0.95 => 3,
            v if v >= 0.6 => 2,
            v if v > 0.0 => 1,
            _ => 0,
        })
    })
}

/// The bass line as drawn: notes, the steps they are held over, rests.
pub fn bass_cells(line: &BassPattern) -> [BassCell; 16] {
    let mut cells = [BassCell::Rest; 16];
    let mut held = 0u8;
    for (cell, step) in cells.iter_mut().zip(line.steps.iter()) {
        match step {
            Some(note) => {
                *cell = BassCell::Note {
                    degree: note.degree,
                    accent: note.accent,
                    glide: note.glide,
                };
                held = note.len.saturating_sub(1);
            }
            None if held > 0 => {
                *cell = BassCell::Hold;
                held -= 1;
            }
            None => {}
        }
    }
    cells
}

/// Every pad LED, `[deck][mode][pad]`, as the controller shows it.
pub fn pad_lights(leds: &LedBank) -> [[[Light; 4]; 4]; 2] {
    let light = |deck, mode, pad| match leds.state(Led::Pad { deck, mode, pad }) {
        LedState::Off => Light::Off,
        LedState::Blink | LedState::BlinkFast => Light::Wait,
        _ => Light::On,
    };
    [Deck::A, Deck::B]
        .map(|deck| PadMode::ALL.map(|mode| [0, 1, 2, 3].map(|pad| light(deck, mode, pad))))
}

/// Index of the mode each deck shows, in `PadMode::ALL` order.
pub fn shown_modes(leds: &LedBank) -> [u8; 2] {
    [Deck::A, Deck::B].map(|deck| {
        let shown = leds.shown_mode(deck);
        PadMode::ALL.iter().position(|&m| m == shown).unwrap_or(0) as u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_bass_notes_cover_their_length() {
        let line = BassPattern::parse("1! - - . ~5 . . . . . . . . . . .");
        let cells = bass_cells(&line);
        assert_eq!(
            cells[0],
            BassCell::Note {
                degree: 1,
                accent: true,
                glide: false
            }
        );
        assert_eq!(cells[1], BassCell::Hold);
        assert_eq!(cells[2], BassCell::Hold);
        assert_eq!(cells[3], BassCell::Rest);
        assert!(matches!(
            cells[4],
            BassCell::Note {
                degree: 5,
                glide: true,
                ..
            }
        ));
    }

    #[test]
    fn drum_velocities_fall_in_four_levels() {
        let pattern = Pattern::from_rows(&[(engine::drums::DrumVoice::Kick, "Xxo. ............")]);
        assert_eq!(&drum_grid(&pattern)[0][..4], &[3, 2, 1, 0]);
    }

    #[test]
    fn pads_mirror_the_leds() {
        let mut leds = LedBank::new();
        leds.set_state(
            Led::Pad {
                deck: Deck::B,
                mode: PadMode::Sampler,
                pad: 2,
            },
            LedState::Blink,
        );
        leds.set(
            Led::Pad {
                deck: Deck::A,
                mode: PadMode::HotCue,
                pad: 0,
            },
            true,
        );
        let lights = pad_lights(&leds);
        assert_eq!(lights[1][1][2], Light::Wait);
        assert_eq!(lights[0][0][0], Light::On);
        assert_eq!(lights[0][0][1], Light::Off);
    }
}
