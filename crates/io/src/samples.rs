//! Loads a sample kit from a folder of WAV files named after the drum voices (`kick.wav`,
//! `snare.wav`, `closed_hat.wav`, `open_hat.wav`, `clap.wav`, `perc.wav`), mixed down to mono.
//! Missing files leave their voice to the synth. See `tools/import-kit-909.sh`.
//!
//! Runs on: control thread (file I/O and allocation, before the stream starts).

use std::path::{Path, PathBuf};

use engine::drums::DrumVoice;
use engine::sampler::SampleBank;

/// Kit shipped in the repository (iteration 015).
pub const DEFAULT_KIT: &str = "assets/kits/tr909";

#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("{path} : {source}")]
    Wav { path: PathBuf, source: hound::Error },
    #[error("{path} : {found} Hz, alors que les autres samples du kit sont à {expected} Hz")]
    MixedRates {
        path: PathBuf,
        found: u32,
        expected: u32,
    },
    #[error("aucun sample dans {0}")]
    Empty(PathBuf),
}

/// Decodes one WAV file to mono f32 in -1..=1, with its sample rate.
fn read_mono(path: &Path) -> Result<(Vec<f32>, u32), hound::Error> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1_i64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

/// Loads every voice found in `dir`.
pub fn load_kit(dir: &Path) -> Result<SampleBank, SampleError> {
    let mut bank = SampleBank::default();
    for (voice, slot) in DrumVoice::ALL.into_iter().zip(bank.voices.iter_mut()) {
        let path = dir.join(format!("{}.wav", voice.name()));
        if !path.exists() {
            continue;
        }
        let (data, rate) = read_mono(&path).map_err(|source| SampleError::Wav {
            path: path.clone(),
            source,
        })?;
        if bank.sample_rate == 0 {
            bank.sample_rate = rate;
        } else if rate != bank.sample_rate {
            return Err(SampleError::MixedRates {
                path,
                found: rate,
                expected: bank.sample_rate,
            });
        }
        *slot = data;
    }
    if bank.sample_rate == 0 {
        return Err(SampleError::Empty(dir.to_path_buf()));
    }
    Ok(bank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_shipped_909_kit() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(DEFAULT_KIT);
        let bank = load_kit(&dir).expect("kit in assets/");
        assert_eq!(bank.sample_rate, 48_000);
        for (voice, data) in DrumVoice::ALL.iter().zip(&bank.voices) {
            assert!(data.len() > 4_800, "{voice:?}: {} samples", data.len());
            let peak = data.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
            assert!((0.05..=1.0).contains(&peak), "{voice:?}: peak {peak}");
        }
    }

    #[test]
    fn empty_folder_is_an_error() {
        assert!(matches!(
            load_kit(Path::new("/nonexistent")),
            Err(SampleError::Empty(_))
        ));
    }
}
