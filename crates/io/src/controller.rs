//! Decoding of raw DJControl Mix MIDI messages into typed events, and the messages we send back.
//!
//! Runs on: CoreMIDI callback thread or control thread. Allocation-free so it can also be used
//! close to the audio path later.
//! Message map and reliability notes: docs/hardware/djcontrol-mix.md.

use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Deck {
    A,
    B,
}

/// Pad mode selected by the hardware mode buttons (the controller remembers it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadMode {
    HotCue,
    Sampler,
    Fx,
    Loop,
}

impl PadMode {
    pub const ALL: [Self; 4] = [Self::HotCue, Self::Sampler, Self::Fx, Self::Loop];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Sync,
    Cue,
    Play,
    JogTouch,
    Pfl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MixerControl {
    Crossfader,
    Master,
    Headphone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckKnob {
    Volume,
    Filter,
    Bass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JogSurface {
    /// Outer ring (pitch bend in DJ software).
    Edge,
    /// Top surface, sent while it is touched.
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerEvent {
    Shift {
        pressed: bool,
    },
    Vinyl {
        pressed: bool,
    },
    /// Global button switching both Filter/Bass knobs between CC 01 (filter) and CC 02 (bass).
    FilterBassToggle {
        pressed: bool,
    },
    /// Pad mode button. FX and Loop are the Shift layer of the mode buttons, encoded in the note.
    PadModeSelect {
        deck: Deck,
        mode: PadMode,
        pressed: bool,
    },
    Button {
        deck: Deck,
        button: Button,
        shift: bool,
        pressed: bool,
    },
    Pad {
        deck: Deck,
        mode: PadMode,
        /// 0..=3, left to right.
        pad: u8,
        shift: bool,
        pressed: bool,
        velocity: u8,
    },
    /// Continuous values are 14-bit (0..=16383): MSB on CC n, LSB on CC n + 0x20.
    /// An event is emitted on every MSB or LSB update.
    Mixer {
        control: MixerControl,
        value: u16,
    },
    Knob {
        deck: Deck,
        knob: DeckKnob,
        value: u16,
    },
    Tempo {
        deck: Deck,
        value: u16,
    },
    Jog {
        deck: Deck,
        surface: JogSurface,
        shift: bool,
        /// +1 clockwise, -1 counter-clockwise.
        delta: i8,
    },
    /// Anything not in the map, kept raw so the sniffer can show it.
    Unknown {
        status: u8,
        data1: u8,
        data2: u8,
    },
}

/// Asks the controller to send the position of every knob and fader.
pub const REQUEST_DUMP: [u8; 3] = [0xB0, 0x7F, 0x7F];
/// Switches every LED off (send on shutdown).
pub const ALL_LEDS_OFF: [u8; 3] = [0xB0, 0x7F, 0x00];

/// A light the host can drive. Addresses verified on the user's controller (iteration 003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Led {
    Sync(Deck),
    Cue(Deck),
    Play(Deck),
    Vinyl,
    /// Only the LEDs of the pad mode currently shown by the controller are visible: keep the
    /// state of all four modes and resend the shown mode on every `PadModeSelect`.
    Pad {
        deck: Deck,
        mode: PadMode,
        /// 0..=3, pad 1 (top left) is 0.
        pad: u8,
    },
}

/// Note On message switching a LED on or off (LEDs are on/off only, no brightness).
/// Returns `None` for an out-of-range pad index.
#[must_use]
pub fn led_message(led: Led, on: bool) -> Option<[u8; 3]> {
    let deck_channel = |deck| match deck {
        Deck::A => 0x91,
        Deck::B => 0x92,
    };
    let (status, note) = match led {
        Led::Sync(deck) => (deck_channel(deck), 0x05),
        Led::Cue(deck) => (deck_channel(deck), 0x06),
        Led::Play(deck) => (deck_channel(deck), 0x07),
        Led::Vinyl => (0x91, 0x03),
        Led::Pad { deck, mode, pad } => {
            if pad > 3 {
                return None;
            }
            let status = match deck {
                Deck::A => 0x96,
                Deck::B => 0x97,
            };
            let offset = match mode {
                PadMode::HotCue => 0x00,
                PadMode::Sampler => 0x10,
                PadMode::Fx => 0x20,
                PadMode::Loop => 0x30,
            };
            (status, offset + pad)
        }
    };
    Some([status, note, if on { 0x7F } else { 0x00 }])
}

/// Maximum value of a 14-bit continuous control.
pub const MAX_14BIT: u16 = 0x3FFF;

/// Absolute control addressed by (channel, base CC).
#[derive(Debug, Clone, Copy)]
enum Continuous {
    Mixer(MixerControl),
    Knob(Deck, DeckKnob),
    Tempo(Deck),
}

impl Continuous {
    fn from_address(channel: u8, base_cc: u8) -> Option<Self> {
        let deck = match channel {
            0 => {
                return match base_cc {
                    0x00 => Some(Self::Mixer(MixerControl::Crossfader)),
                    0x03 => Some(Self::Mixer(MixerControl::Master)),
                    0x04 => Some(Self::Mixer(MixerControl::Headphone)),
                    _ => None,
                };
            }
            // Shift held moves the deck controls to channels 4/5 (seen in the 015 test: Shift +
            // Volume A never reached the plain channel). Same control; the mapping picks the layer
            // from the Shift state.
            1 | 4 => Deck::A,
            2 | 5 => Deck::B,
            _ => return None,
        };
        match base_cc {
            0x00 => Some(Self::Knob(deck, DeckKnob::Volume)),
            0x01 => Some(Self::Knob(deck, DeckKnob::Filter)),
            0x02 => Some(Self::Knob(deck, DeckKnob::Bass)),
            0x08 => Some(Self::Tempo(deck)),
            _ => None,
        }
    }

    fn event(self, value: u16) -> ControllerEvent {
        match self {
            Self::Mixer(control) => ControllerEvent::Mixer { control, value },
            Self::Knob(deck, knob) => ControllerEvent::Knob { deck, knob, value },
            Self::Tempo(deck) => ControllerEvent::Tempo { deck, value },
        }
    }
}

/// Stateful decoder: the only state is the last MSB/LSB of each continuous control, needed to
/// rebuild 14-bit values. Indexed by channel (0..=5) and base CC (0x00..=0x0F).
#[derive(Debug, Default, Clone)]
pub struct Decoder {
    halves: [[[u8; 2]; 16]; 6],
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes one MIDI message. Returns `None` for empty input or non channel-voice messages
    /// (clock, SysEx…).
    pub fn decode(&mut self, bytes: &[u8]) -> Option<ControllerEvent> {
        let (&status, rest) = bytes.split_first()?;
        if !(0x80..0xF0).contains(&status) {
            return None;
        }
        let data1 = rest.first().copied().unwrap_or(0);
        let data2 = rest.get(1).copied().unwrap_or(0);
        let unknown = ControllerEvent::Unknown {
            status,
            data1,
            data2,
        };
        let channel = status & 0x0F;
        let event = match status & 0xF0 {
            // Note Off and Note On with velocity 0 are both releases.
            0x80 => decode_note(channel, data1, false, 0),
            0x90 => decode_note(channel, data1, data2 > 0, data2),
            0xB0 => self.decode_cc(channel, data1, data2),
            _ => None,
        };
        Some(event.unwrap_or(unknown))
    }

    fn decode_cc(&mut self, channel: u8, cc: u8, value: u8) -> Option<ControllerEvent> {
        // Jogs are relative and exist on the plain and Shift deck channels.
        if let Some((deck, shift)) = deck_layer(channel) {
            let surface = match cc {
                0x09 => Some(JogSurface::Edge),
                0x0A => Some(JogSurface::Top),
                _ => None,
            };
            if let Some(surface) = surface {
                return Some(ControllerEvent::Jog {
                    deck,
                    surface,
                    shift,
                    delta: if value < 0x40 { 1 } else { -1 },
                });
            }
        }
        let (base_cc, is_lsb) = match cc {
            0x00..=0x0F => (cc, false),
            0x20..=0x2F => (cc - 0x20, true),
            _ => return None,
        };
        let target = Continuous::from_address(channel, base_cc)?;
        let half = self
            .halves
            .get_mut(usize::from(channel))?
            .get_mut(usize::from(base_cc))?;
        let [msb, lsb] = half;
        if is_lsb {
            *lsb = value & 0x7F;
        } else {
            *msb = value & 0x7F;
        }
        Some(target.event((u16::from(*msb) << 7) | u16::from(*lsb)))
    }
}

fn percent(value: u16) -> f32 {
    f32::from(value) * 100.0 / f32::from(MAX_14BIT)
}

/// Channels 1/2 are decks A/B, channels 4/5 the same controls with Shift held (0-based).
fn deck_layer(channel: u8) -> Option<(Deck, bool)> {
    match channel {
        1 => Some((Deck::A, false)),
        2 => Some((Deck::B, false)),
        4 => Some((Deck::A, true)),
        5 => Some((Deck::B, true)),
        _ => None,
    }
}

fn decode_note(channel: u8, note: u8, pressed: bool, velocity: u8) -> Option<ControllerEvent> {
    match (channel, note) {
        (0, 0x03) => return Some(ControllerEvent::Shift { pressed }),
        (0, 0x01) => return Some(ControllerEvent::FilterBassToggle { pressed }),
        // One physical button, sent on both deck channels (91 03 and 92 03).
        (1 | 2, 0x03) => return Some(ControllerEvent::Vinyl { pressed }),
        _ => {}
    }
    if let (Some((deck, false)), 0x0F..=0x12) = (deck_layer(channel), note) {
        let mode = match note {
            0x0F => PadMode::HotCue,
            0x10 => PadMode::Sampler,
            0x11 => PadMode::Fx,
            _ => PadMode::Loop,
        };
        return Some(ControllerEvent::PadModeSelect {
            deck,
            mode,
            pressed,
        });
    }
    if let Some(deck) = match channel {
        6 => Some(Deck::A),
        7 => Some(Deck::B),
        _ => None,
    } {
        let pad = note & 0x07;
        if pad > 3 || note > 0x3F {
            return None;
        }
        let mode = match note >> 4 {
            0 => PadMode::HotCue,
            1 => PadMode::Sampler,
            2 => PadMode::Fx,
            _ => PadMode::Loop,
        };
        return Some(ControllerEvent::Pad {
            deck,
            mode,
            pad,
            shift: note & 0x08 != 0,
            pressed,
            velocity,
        });
    }
    let (deck, shift) = deck_layer(channel)?;
    let button = match note {
        0x05 => Button::Sync,
        0x06 => Button::Cue,
        0x07 => Button::Play,
        0x08 => Button::JogTouch,
        0x0C => Button::Pfl,
        _ => return None,
    };
    Some(ControllerEvent::Button {
        deck,
        button,
        shift,
        pressed,
    })
}

impl fmt::Display for ControllerEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sh = |shift: bool| if shift { " +shift" } else { "" };
        let st = |pressed: bool| if pressed { "down" } else { "up" };
        match *self {
            Self::Shift { pressed } => write!(f, "Shift {}", st(pressed)),
            Self::Vinyl { pressed } => write!(f, "Vinyl {}", st(pressed)),
            Self::FilterBassToggle { pressed } => write!(f, "Filter/Bass toggle {}", st(pressed)),
            Self::PadModeSelect {
                deck,
                mode,
                pressed,
            } => write!(f, "{deck:?} mode button [{mode:?}] {}", st(pressed)),
            Self::Button {
                deck,
                button,
                shift,
                pressed,
            } => write!(f, "{deck:?} {button:?}{} {}", sh(shift), st(pressed)),
            Self::Pad {
                deck,
                mode,
                pad,
                shift,
                pressed,
                velocity,
            } => write!(
                f,
                "{deck:?} pad {} [{mode:?}]{} {} vel={velocity}",
                pad + 1,
                sh(shift),
                st(pressed)
            ),
            Self::Mixer { control, value } => {
                write!(f, "{control:?} = {value} ({:.1} %)", percent(value))
            }
            Self::Knob { deck, knob, value } => {
                write!(f, "{deck:?} {knob:?} = {value} ({:.1} %)", percent(value))
            }
            Self::Tempo { deck, value } => {
                write!(f, "{deck:?} Tempo = {value} ({:.1} %)", percent(value))
            }
            Self::Jog {
                deck,
                surface,
                shift,
                delta,
            } => write!(f, "{deck:?} Jog {surface:?}{} {delta:+}", sh(shift)),
            Self::Unknown {
                status,
                data1,
                data2,
            } => write!(f, "UNKNOWN {status:02X} {data1:02X} {data2:02X}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(bytes: &[u8]) -> ControllerEvent {
        Decoder::new().decode(bytes).expect("channel message")
    }

    #[test]
    fn led_addresses_match_hardware() {
        // Verified LED by LED with tools/led-scan.sh (iteration 003).
        assert_eq!(
            led_message(Led::Sync(Deck::A), true),
            Some([0x91, 0x05, 0x7F])
        );
        assert_eq!(
            led_message(Led::Cue(Deck::B), true),
            Some([0x92, 0x06, 0x7F])
        );
        assert_eq!(
            led_message(Led::Play(Deck::B), false),
            Some([0x92, 0x07, 0x00])
        );
        assert_eq!(led_message(Led::Vinyl, true), Some([0x91, 0x03, 0x7F]));
        let pad = |deck, mode, pad| Led::Pad { deck, mode, pad };
        assert_eq!(
            led_message(pad(Deck::A, PadMode::Loop, 0), true),
            Some([0x96, 0x30, 0x7F])
        );
        assert_eq!(
            led_message(pad(Deck::B, PadMode::HotCue, 3), true),
            Some([0x97, 0x03, 0x7F])
        );
        assert_eq!(
            led_message(pad(Deck::A, PadMode::Sampler, 2), true),
            Some([0x96, 0x12, 0x7F])
        );
        assert_eq!(led_message(pad(Deck::A, PadMode::Fx, 4), true), None);
    }

    #[test]
    fn mode_buttons_and_filter_toggle() {
        // Captured on the user's controller (iteration 003).
        let select = |deck, mode| ControllerEvent::PadModeSelect {
            deck,
            mode,
            pressed: true,
        };
        assert_eq!(dec(&[0x91, 0x0F, 0x7F]), select(Deck::A, PadMode::HotCue));
        assert_eq!(dec(&[0x91, 0x10, 0x7F]), select(Deck::A, PadMode::Sampler));
        assert_eq!(dec(&[0x91, 0x11, 0x7F]), select(Deck::A, PadMode::Fx));
        assert_eq!(dec(&[0x92, 0x12, 0x7F]), select(Deck::B, PadMode::Loop));
        assert_eq!(
            dec(&[0x90, 0x01, 0x00]),
            ControllerEvent::FilterBassToggle { pressed: false }
        );
    }

    #[test]
    fn global_buttons() {
        assert_eq!(
            dec(&[0x90, 0x03, 0x7F]),
            ControllerEvent::Shift { pressed: true }
        );
        assert_eq!(
            dec(&[0x90, 0x03, 0x00]),
            ControllerEvent::Shift { pressed: false }
        );
        assert_eq!(
            dec(&[0x91, 0x03, 0x7F]),
            ControllerEvent::Vinyl { pressed: true }
        );
        assert_eq!(
            dec(&[0x92, 0x03, 0x00]),
            ControllerEvent::Vinyl { pressed: false }
        );
    }

    #[test]
    fn deck_buttons_and_shift_layer() {
        let cases = [
            (0x91, 0x07, Deck::A, Button::Play, false),
            (0x92, 0x06, Deck::B, Button::Cue, false),
            (0x91, 0x05, Deck::A, Button::Sync, false),
            (0x92, 0x0C, Deck::B, Button::Pfl, false),
            (0x91, 0x08, Deck::A, Button::JogTouch, false),
            (0x94, 0x07, Deck::A, Button::Play, true),
            (0x95, 0x05, Deck::B, Button::Sync, true),
        ];
        for (status, note, deck, button, shift) in cases {
            assert_eq!(
                dec(&[status, note, 0x7F]),
                ControllerEvent::Button {
                    deck,
                    button,
                    shift,
                    pressed: true
                }
            );
        }
    }

    #[test]
    fn release_by_note_off_or_zero_velocity() {
        let expected = ControllerEvent::Button {
            deck: Deck::A,
            button: Button::Play,
            shift: false,
            pressed: false,
        };
        assert_eq!(dec(&[0x91, 0x07, 0x00]), expected);
        assert_eq!(dec(&[0x81, 0x07, 0x40]), expected);
    }

    #[test]
    fn pads_encode_mode_pad_and_shift() {
        let cases = [
            (0x96, 0x00, Deck::A, PadMode::HotCue, 0, false),
            (0x96, 0x0B, Deck::A, PadMode::HotCue, 3, true),
            (0x97, 0x12, Deck::B, PadMode::Sampler, 2, false),
            (0x96, 0x21, Deck::A, PadMode::Fx, 1, false),
            (0x97, 0x2A, Deck::B, PadMode::Fx, 2, true),
            (0x96, 0x33, Deck::A, PadMode::Loop, 3, false),
        ];
        for (status, note, deck, mode, pad, shift) in cases {
            assert_eq!(
                dec(&[status, note, 0x7F]),
                ControllerEvent::Pad {
                    deck,
                    mode,
                    pad,
                    shift,
                    pressed: true,
                    velocity: 0x7F
                }
            );
        }
        // Out of the documented range: kept raw.
        assert!(matches!(
            dec(&[0x96, 0x04, 0x7F]),
            ControllerEvent::Unknown { .. }
        ));
        assert!(matches!(
            dec(&[0x96, 0x40, 0x7F]),
            ControllerEvent::Unknown { .. }
        ));
    }

    #[test]
    fn continuous_controls_are_14_bit() {
        // Bytes captured from the user's controller after a dump request (2026-10-31).
        let mut d = Decoder::new();
        let mixer = |control, value| ControllerEvent::Mixer { control, value };
        let knob = |deck, knob, value| ControllerEvent::Knob { deck, knob, value };
        assert_eq!(
            d.decode(&[0xB0, 0x00, 0x54]),
            Some(mixer(MixerControl::Crossfader, 0x54 << 7))
        );
        assert_eq!(
            d.decode(&[0xB0, 0x20, 0x1E]),
            Some(mixer(MixerControl::Crossfader, (0x54 << 7) | 0x1E))
        );
        assert_eq!(
            d.decode(&[0xB0, 0x24, 0x70]),
            Some(mixer(MixerControl::Headphone, 0x70))
        );
        assert_eq!(
            d.decode(&[0xB1, 0x20, 0x4E]),
            Some(knob(Deck::A, DeckKnob::Volume, 0x4E))
        );
        assert_eq!(
            d.decode(&[0xB2, 0x02, 0x40]),
            Some(knob(Deck::B, DeckKnob::Bass, 0x40 << 7))
        );
        assert_eq!(
            d.decode(&[0xB2, 0x21, 0x7F]),
            Some(knob(Deck::B, DeckKnob::Filter, 0x7F))
        );
        // With Shift held, deck controls come on channels 4/5: same control.
        assert_eq!(
            d.decode(&[0xB4, 0x00, 0x10]),
            Some(knob(Deck::A, DeckKnob::Volume, 0x10 << 7))
        );
    }

    #[test]
    fn tempo_is_rebuilt_from_msb_and_lsb() {
        let mut d = Decoder::new();
        assert_eq!(
            d.decode(&[0xB1, 0x08, 0x40]),
            Some(ControllerEvent::Tempo {
                deck: Deck::A,
                value: 0x40 << 7
            })
        );
        assert_eq!(
            d.decode(&[0xB1, 0x28, 0x05]),
            Some(ControllerEvent::Tempo {
                deck: Deck::A,
                value: (0x40 << 7) | 0x05
            })
        );
        // Deck B keeps its own halves.
        assert_eq!(
            d.decode(&[0xB2, 0x28, 0x7F]),
            Some(ControllerEvent::Tempo {
                deck: Deck::B,
                value: 0x7F
            })
        );
    }

    #[test]
    fn jogs_are_relative() {
        let jog = |deck, surface, shift, delta| ControllerEvent::Jog {
            deck,
            surface,
            shift,
            delta,
        };
        assert_eq!(
            dec(&[0xB1, 0x09, 0x01]),
            jog(Deck::A, JogSurface::Edge, false, 1)
        );
        assert_eq!(
            dec(&[0xB2, 0x0A, 0x7F]),
            jog(Deck::B, JogSurface::Top, false, -1)
        );
        assert_eq!(
            dec(&[0xB4, 0x0A, 0x01]),
            jog(Deck::A, JogSurface::Top, true, 1)
        );
        assert_eq!(
            dec(&[0xB5, 0x0A, 0x7F]),
            jog(Deck::B, JogSurface::Top, true, -1)
        );
    }

    #[test]
    fn ignores_non_channel_messages() {
        let mut d = Decoder::new();
        assert_eq!(d.decode(&[]), None);
        assert_eq!(d.decode(&[0xF8]), None);
        assert_eq!(d.decode(&[0xF0, 0x00, 0xF7]), None);
    }

    #[test]
    fn display_is_readable() {
        let text = dec(&[0x97, 0x2A, 0x7F]).to_string();
        assert_eq!(text, "B pad 3 [Fx] +shift down vel=127");
    }
}
