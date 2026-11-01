//! Renders the drum kit for listening: `target/renders/kit.wav` (each voice alone, then a few bars
//! of groove) and `target/renders/patterns.wav` (the sequencer's patterns A–D, two bars each).
//! Offline, so it may allocate and write files.
//!
//! cargo run -p engine --example render_kit --release

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use engine::Engine;
use engine::drums::DrumVoice;

const SR: u32 = 48_000;
const BPM: f64 = 120.0;

fn write_wav(path: &Path, interleaved: &[f32], channels: u16) -> io::Result<()> {
    let data_len = u32::try_from(interleaved.len() * 2).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + interleaved.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&SR.to_le_bytes());
    out.extend_from_slice(&(SR * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in interleaved {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    fs::File::create(path)?.write_all(&out)
}

fn main() -> io::Result<()> {
    let mut engine = Engine::new(SR, 512, BPM);
    let mut audio = Vec::new();
    let mut render = |engine: &mut Engine, frames: usize| {
        let start = audio.len();
        audio.resize(start + frames * 2, 0.0);
        for chunk in audio[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
    };

    // Each voice alone, half a second apart.
    for voice in DrumVoice::ALL {
        engine.trigger_drum(voice, 1.0);
        render(&mut engine, SR as usize * 3 / 4);
    }
    render(&mut engine, SR as usize / 2);

    // Four bars of a hybrid groove, triggered on 16th notes.
    let sixteenth = (f64::from(SR) * 60.0 / BPM / 4.0) as usize;
    let grid: [(DrumVoice, &str, f32); 6] = [
        (DrumVoice::Kick, "x.....x...x.....", 1.0),
        (DrumVoice::Snare, "....x.......x...", 0.9),
        (DrumVoice::ClosedHat, "x.x.x.x.x.x.x.x.", 0.6),
        (DrumVoice::OpenHat, "...............x", 0.5),
        (DrumVoice::Clap, "............x...", 0.7),
        (DrumVoice::Perc, "..x....x.....x..", 0.5),
    ];
    for _bar in 0..4 {
        for step in 0..16 {
            for (voice, pattern, velocity) in grid {
                if pattern.as_bytes().get(step) == Some(&b'x') {
                    engine.trigger_drum(voice, velocity);
                }
            }
            render(&mut engine, sixteenth);
        }
    }
    render(&mut engine, SR as usize);

    let dir = Path::new("target/renders");
    fs::create_dir_all(dir)?;
    save(&dir.join("kit.wav"), &audio)?;

    // The sequencer: patterns A–D, two bars each, swing 54 %, metronome off.
    let mut engine = Engine::new(SR, 512, BPM);
    engine.set_metronome(false);
    engine.set_swing(0.54);
    engine.start();
    let bar = sixteenth * 16;
    let mut patterns = Vec::new();
    for index in 0..4 {
        engine.queue_pattern(index);
        let start = patterns.len();
        patterns.resize(start + bar * 2 * 2, 0.0);
        for chunk in patterns[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
    }
    save(&dir.join("patterns.wav"), &patterns)
}

fn save(path: &Path, audio: &[f32]) -> io::Result<()> {
    write_wav(path, audio, 2)?;
    let peak = audio.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    println!(
        "{} — {:.1} s, crête {:.2} ({:.1} dBFS)",
        path.display(),
        audio.len() as f64 / 2.0 / f64::from(SR),
        peak,
        20.0 * peak.log10()
    );
    Ok(())
}
