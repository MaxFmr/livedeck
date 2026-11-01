//! Wanted state of every controller LED, sent as a diff.
//!
//! The controller only shows the pad LEDs of the pad mode it currently displays, so pad LEDs of the
//! other modes are kept here and sent when their mode is shown. After a (re)connection everything is
//! resent.
//!
//! LEDs are on/off only, so states such as « waiting » are drawn in software: each LED holds a
//! `LedState` rendered against the musical position (`set_beats`), so blinks and flashes stay in
//! time with the music.
//!
//! Runs on: control thread.

use crate::controller::{Deck, Led, PadMode, led_message};

const DECKS: [Deck; 2] = [Deck::A, Deck::B];
const MODES: [PadMode; 4] = PadMode::ALL;
/// Sync, Cue, Play per deck.
const TRANSPORT: usize = 3 * 2;
const VINYL: usize = TRANSPORT;
const FIRST_PAD: usize = VINYL + 1;
/// Transport + Vinyl + 2 decks × 4 modes × 4 pads.
pub const LED_COUNT: usize = FIRST_PAD + 2 * 4 * 4;

fn deck_index(deck: Deck) -> usize {
    match deck {
        Deck::A => 0,
        Deck::B => 1,
    }
}

fn mode_index(mode: PadMode) -> usize {
    match mode {
        PadMode::HotCue => 0,
        PadMode::Sampler => 1,
        PadMode::Fx => 2,
        PadMode::Loop => 3,
    }
}

fn index(led: Led) -> Option<usize> {
    match led {
        Led::Sync(deck) => Some(deck_index(deck) * 3),
        Led::Cue(deck) => Some(deck_index(deck) * 3 + 1),
        Led::Play(deck) => Some(deck_index(deck) * 3 + 2),
        Led::Vinyl => Some(VINYL),
        Led::Pad { deck, mode, pad } => (pad < 4)
            .then(|| FIRST_PAD + deck_index(deck) * 16 + mode_index(mode) * 4 + usize::from(pad)),
    }
}

fn led_at(i: usize) -> Option<Led> {
    match i {
        0..TRANSPORT => {
            let deck = *DECKS.get(i / 3)?;
            Some(match i % 3 {
                0 => Led::Sync(deck),
                1 => Led::Cue(deck),
                _ => Led::Play(deck),
            })
        }
        VINYL => Some(Led::Vinyl),
        _ if i < LED_COUNT => {
            let k = i - FIRST_PAD;
            Some(Led::Pad {
                deck: *DECKS.get(k / 16)?,
                mode: *MODES.get(k % 16 / 4)?,
                pad: u8::try_from(k % 4).ok()?,
            })
        }
        _ => None,
    }
}

/// What a LED shows. Blinks and flashes follow the beat position given to `LedBank::set_beats`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LedState {
    #[default]
    Off,
    On,
    /// Half of every beat: « waiting », ready to start.
    Blink,
    /// Four times per beat: « needs attention » (e.g. a fader waiting to be picked up).
    BlinkFast,
    /// The first quarter of every beat.
    FlashBeat,
    /// The first quarter of beat 1 of each bar.
    FlashDownbeat,
    /// The first quarter of beats 2, 3 and 4.
    FlashOtherBeats,
}

/// Fraction of a beat a flash stays lit (a quarter: 125 ms at 120 BPM).
const FLASH: f64 = 0.25;
const BEATS_PER_BAR: f64 = 4.0;

impl LedState {
    /// Whether the LED is lit at `beats` (position in beats; bar starts on multiples of 4).
    #[must_use]
    pub fn lit(self, beats: f64) -> bool {
        let beats = if beats.is_finite() {
            beats.max(0.0)
        } else {
            0.0
        };
        let frac = beats.fract();
        let downbeat = beats.rem_euclid(BEATS_PER_BAR) < 1.0;
        match self {
            Self::Off => false,
            Self::On => true,
            Self::Blink => frac < 0.5,
            Self::BlinkFast => (frac * 4.0).fract() < 0.5,
            Self::FlashBeat => frac < FLASH,
            Self::FlashDownbeat => downbeat && frac < FLASH,
            Self::FlashOtherBeats => !downbeat && frac < FLASH,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LedBank {
    wanted: [LedState; LED_COUNT],
    beats: f64,
    /// What the controller is believed to display; `None` = unknown, must be sent.
    sent: [Option<bool>; LED_COUNT],
    shown_mode: [PadMode; 2],
}

impl Default for LedBank {
    fn default() -> Self {
        Self {
            wanted: [LedState::Off; LED_COUNT],
            beats: 0.0,
            sent: [None; LED_COUNT],
            shown_mode: [PadMode::HotCue; 2],
        }
    }
}

impl LedBank {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, led: Led, on: bool) {
        self.set_state(led, if on { LedState::On } else { LedState::Off });
    }

    pub fn set_state(&mut self, led: Led, state: LedState) {
        if let Some(slot) = index(led).and_then(|i| self.wanted.get_mut(i)) {
            *slot = state;
        }
    }

    #[must_use]
    pub fn state(&self, led: Led) -> LedState {
        index(led)
            .and_then(|i| self.wanted.get(i))
            .copied()
            .unwrap_or_default()
    }

    /// Whether the LED is lit at the current position.
    #[must_use]
    pub fn get(&self, led: Led) -> bool {
        self.state(led).lit(self.beats)
    }

    /// Musical position (in beats) that blinks and flashes are drawn against.
    pub fn set_beats(&mut self, beats: f64) {
        self.beats = beats;
    }

    /// The controller state is unknown (new connection): resend everything on the next flush.
    pub fn invalidate(&mut self) {
        self.sent = [None; LED_COUNT];
    }

    /// The controller may have changed this LED by itself (pads light up under the finger): send
    /// its wanted state again on the next flush.
    pub fn touched(&mut self, led: Led) {
        if let Some(slot) = index(led).and_then(|i| self.sent.get_mut(i)) {
            *slot = None;
        }
    }

    /// The controller now displays `mode` on `deck`: its pad LEDs must be (re)sent.
    pub fn mode_shown(&mut self, deck: Deck, mode: PadMode) {
        if let Some(shown) = self.shown_mode.get_mut(deck_index(deck)) {
            *shown = mode;
        }
        for pad in 0..4 {
            if let Some(slot) =
                index(Led::Pad { deck, mode, pad }).and_then(|i| self.sent.get_mut(i))
            {
                *slot = None;
            }
        }
    }

    fn visible(&self, led: Led) -> bool {
        match led {
            Led::Pad { deck, mode, .. } => self.shown_mode.get(deck_index(deck)) == Some(&mode),
            _ => true,
        }
    }

    /// Calls `send` with the message of every visible LED whose state differs from what was sent.
    pub fn flush(&mut self, mut send: impl FnMut([u8; 3])) {
        for i in 0..LED_COUNT {
            let Some(led) = led_at(i) else { continue };
            let (Some(state), Some(&sent)) = (self.wanted.get(i), self.sent.get(i)) else {
                continue;
            };
            let wanted = state.lit(self.beats);
            if sent == Some(wanted) || !self.visible(led) {
                continue;
            }
            if let Some(msg) = led_message(led, wanted) {
                send(msg);
                if let Some(slot) = self.sent.get_mut(i) {
                    *slot = Some(wanted);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flushed(bank: &mut LedBank) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        bank.flush(|m| out.push(m));
        out
    }

    #[test]
    fn index_round_trips() {
        for i in 0..LED_COUNT {
            let led = led_at(i).expect("every index maps to a LED");
            assert_eq!(index(led), Some(i));
        }
        assert_eq!(led_at(LED_COUNT), None);
    }

    #[test]
    fn first_flush_sends_every_visible_led_then_only_diffs() {
        let mut bank = LedBank::new();
        // 6 transport + Vinyl + 4 pads per deck in the shown mode (Hot Cue).
        assert_eq!(flushed(&mut bank).len(), 6 + 1 + 8);
        assert!(flushed(&mut bank).is_empty());
        bank.set(Led::Play(Deck::A), true);
        assert_eq!(flushed(&mut bank), vec![[0x91, 0x07, 0x7F]]);
        bank.set(Led::Play(Deck::A), true);
        assert!(flushed(&mut bank).is_empty());
    }

    #[test]
    fn hidden_mode_pads_wait_until_shown() {
        let mut bank = LedBank::new();
        flushed(&mut bank);
        let loop_pad = Led::Pad {
            deck: Deck::B,
            mode: PadMode::Loop,
            pad: 2,
        };
        bank.set(loop_pad, true);
        assert!(flushed(&mut bank).is_empty(), "Loop is not displayed yet");
        bank.mode_shown(Deck::B, PadMode::Loop);
        let sent = flushed(&mut bank);
        assert_eq!(sent.len(), 4);
        assert!(sent.contains(&[0x97, 0x32, 0x7F]));
    }

    #[test]
    fn blinks_follow_the_beat() {
        // Beat 0 is a downbeat; 1.1 is early in beat 2; 1.6 is late in beat 2.
        assert!(LedState::Blink.lit(0.1) && !LedState::Blink.lit(0.6));
        assert!(LedState::BlinkFast.lit(0.1) && !LedState::BlinkFast.lit(0.2));
        assert!(LedState::BlinkFast.lit(0.3));
        assert!(LedState::FlashDownbeat.lit(4.1) && !LedState::FlashDownbeat.lit(1.1));
        assert!(LedState::FlashOtherBeats.lit(1.1) && !LedState::FlashOtherBeats.lit(4.1));
        assert!(!LedState::FlashBeat.lit(1.6));
        assert_eq!(LedState::Blink.lit(f64::NAN), LedState::Blink.lit(0.0));
    }

    #[test]
    fn blinking_led_sends_only_on_changes() {
        let mut bank = LedBank::new();
        flushed(&mut bank);
        bank.set_state(Led::Play(Deck::B), LedState::Blink);
        bank.set_beats(0.1);
        assert_eq!(flushed(&mut bank), vec![[0x92, 0x07, 0x7F]]);
        bank.set_beats(0.2);
        assert!(flushed(&mut bank).is_empty(), "still lit: nothing to send");
        bank.set_beats(0.7);
        assert_eq!(flushed(&mut bank), vec![[0x92, 0x07, 0x00]]);
    }

    #[test]
    fn touched_led_is_resent_even_if_unchanged() {
        let mut bank = LedBank::new();
        flushed(&mut bank);
        let pad = Led::Pad {
            deck: Deck::A,
            mode: PadMode::HotCue,
            pad: 1,
        };
        bank.touched(pad);
        assert_eq!(flushed(&mut bank), vec![[0x96, 0x01, 0x00]]);
    }

    #[test]
    fn invalidate_resends_everything() {
        let mut bank = LedBank::new();
        flushed(&mut bank);
        bank.invalidate();
        assert_eq!(flushed(&mut bank).len(), 15);
    }
}
