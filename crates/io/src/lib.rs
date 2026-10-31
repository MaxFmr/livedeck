//! LiveDeck I/O: audio output (cpal), MIDI in/out (midir), controller decoding and mapping.
//!
//! Runs on: control thread and CoreMIDI callback thread. Never on the audio thread.
//! Controller facts: docs/hardware/djcontrol-mix.md.

pub mod audio;
pub mod controller;
pub mod leds;
pub mod link;
pub mod mapping;
pub mod system;

/// Substring identifying the DJControl Mix among CoreMIDI port names (case-insensitive).
pub const CONTROLLER_PORT_HINT: &str = "djcontrol";

/// Returns true when a MIDI port name looks like the DJControl Mix.
#[must_use]
pub fn is_controller_port(name: &str) -> bool {
    name.to_ascii_lowercase().contains(CONTROLLER_PORT_HINT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_controller_port_names() {
        assert!(is_controller_port("DJControl MIX"));
        assert!(is_controller_port("Hercules DJCONTROL MIX Port 1"));
        assert!(!is_controller_port("IAC Driver Bus 1"));
    }
}
