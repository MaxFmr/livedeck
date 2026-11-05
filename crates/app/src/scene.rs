//! Scenes: snapshots of what is played, stored in 8 slots and in a session file (runs on: control
//! thread).
//!
//! A scene holds the musical state of both decks (style, pattern, bass line, mutes, chords,
//! phrase, arpeggio, voicing, sends, filters). Tempo, levels and the crossfader stay in the
//! player's hands: a recall does not touch them.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Number of scene slots.
pub const SLOTS: usize = 8;

/// What a scene recalls.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub style: u8,
    /// Drum pattern (0-based), swing, muted voices (bit `i` = `DrumVoice::ALL[i]`), density.
    pub pattern: u8,
    pub swing: f32,
    pub mutes: u8,
    pub density: f32,
    pub samples: bool,
    /// Bass line (0-based), mute, tonic (0 = C … 11 = B), filter knob.
    pub bass_line: u8,
    pub bass_muted: bool,
    pub tonic: u8,
    pub bass_cutoff: f32,
    pub scale: u8,
    /// Chords latched (bit `i` = pad `i`; only when the latch is on).
    pub latch: bool,
    pub chords: u8,
    pub inversion: i8,
    pub octave: i8,
    pub extension: u8,
    pub arp_mode: u8,
    pub arp_rate: u8,
    pub arp_octaves: u8,
    /// Phrase playing (0-based) and its variation; none when absent.
    #[serde(default)]
    pub phrase: Option<u8>,
    #[serde(default)]
    pub phrase_variation: bool,
    pub transpose: i8,
    pub brightness: f32,
    pub delay_send: f32,
    pub delay_feedback: f32,
    pub reverb_send: f32,
}

/// The 8 slots of the session.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scenes {
    pub slots: [Option<Scene>; SLOTS],
}

/// Session file: `[scenes.1]` … `[scenes.8]`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionFile {
    #[serde(default)]
    scenes: BTreeMap<String, Scene>,
}

impl Scenes {
    /// Slots filled, bit `i` = slot `i`.
    #[must_use]
    pub fn filled(&self) -> u8 {
        self.slots
            .iter()
            .enumerate()
            .fold(0, |m, (i, s)| if s.is_some() { m | 1 << i } else { m })
    }

    #[must_use]
    pub fn get(&self, slot: u8) -> Option<Scene> {
        self.slots.get(usize::from(slot)).copied().flatten()
    }

    pub fn set(&mut self, slot: u8, scene: Scene) {
        if let Some(s) = self.slots.get_mut(usize::from(slot)) {
            *s = Some(scene);
        }
    }

    pub fn from_toml_str(text: &str) -> Result<Self, String> {
        let file: SessionFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut scenes = Self::default();
        for (name, scene) in file.scenes {
            match name.parse::<u8>() {
                Ok(n @ 1..=8) => scenes.set(n - 1, scene),
                _ => return Err(format!("scène « {name} » : attendue entre 1 et 8")),
            }
        }
        Ok(scenes)
    }

    pub fn to_toml_string(&self) -> Result<String, String> {
        let file = SessionFile {
            scenes: self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.map(|s| ((i + 1).to_string(), s)))
                .collect(),
        };
        let body = toml::to_string(&file).map_err(|e| e.to_string())?;
        Ok(format!(
            "# Session LiveDeck : scènes enregistrées au contrôleur (Shift + pads FX/Loop A).\n\
             # Réécrit à chaque enregistrement.\n\n{body}"
        ))
    }

    /// The session file, or empty slots when it does not exist yet.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_toml_str(&text).map_err(|e| format!("{} : {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{} : {e}", path.display())),
        }
    }

    /// Writes the file through a temporary one, so a crash never leaves it half written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = self.to_toml_string()?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{} : {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        Scene {
            style: 2,
            pattern: 1,
            swing: 0.58,
            mutes: 0b100,
            density: 0.7,
            samples: false,
            bass_line: 3,
            bass_muted: false,
            tonic: 9,
            bass_cutoff: 0.4,
            scale: 1,
            latch: true,
            chords: 0b10,
            inversion: -1,
            octave: 1,
            extension: 2,
            arp_mode: 1,
            arp_rate: 2,
            arp_octaves: 2,
            phrase: Some(3),
            phrase_variation: true,
            transpose: 2,
            brightness: 0.5,
            delay_send: 0.3,
            delay_feedback: 0.5,
            reverb_send: 0.2,
        }
    }

    #[test]
    fn the_session_file_round_trips() {
        let mut scenes = Scenes::default();
        scenes.set(0, scene());
        scenes.set(
            6,
            Scene {
                phrase: None,
                ..scene()
            },
        );
        let text = scenes.to_toml_string().unwrap_or_default();
        assert!(text.contains("[scenes.1]") && text.contains("[scenes.7]"));
        assert_eq!(Scenes::from_toml_str(&text), Ok(scenes.clone()));
        assert_eq!(scenes.filled(), 0b100_0001);
    }

    #[test]
    fn bad_slots_are_refused() {
        assert!(Scenes::from_toml_str("[scenes.9]\nstyle = 0").is_err());
        assert_eq!(Scenes::from_toml_str(""), Ok(Scenes::default()));
    }
}
