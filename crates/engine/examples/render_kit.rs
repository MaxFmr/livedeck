//! Renders the drum kit for listening: `target/renders/kit.wav` (each voice alone, then a few bars
//! of groove), `target/renders/patterns.wav` (the sequencer's patterns A–D, two bars each) and
//! `target/renders/gestures.wav` (density, fills and beat-repeat on groove B) and
//! `target/renders/bass.wav` (the four bass lines alone, then with their drums, a filter sweep and
//! a key change) and `target/renders/styles.wav` (each style at its suggested tempo, patterns A–D,
//! one bar each).
//! Offline, so it may allocate and write files.
//!
//! cargo run -p engine --example render_kit --release

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use engine::drums::DrumVoice;
use engine::repeat::Division;
use engine::{Engine, Param};

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
    save(&dir.join("patterns.wav"), &patterns)?;

    // Live gestures on groove B: density 100 → 60 → 30 → 0 % (a bar each), fills 1–4 (one bar
    // each, launched on beat 1 so they run from beat 2), then beat-repeat 1/4, 1/8, 1/16, 1/32
    // (half a bar each, from beat 3) — 12 bars.
    let mut engine = Engine::new(SR, 512, BPM);
    engine.set_metronome(false);
    engine.queue_pattern(1);
    engine.start();
    let mut gestures = Vec::new();
    let mut play = |engine: &mut Engine, frames: usize| {
        let start = gestures.len();
        gestures.resize(start + frames * 2, 0.0);
        for chunk in gestures[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
    };
    for density in [1.0, 0.6, 0.3, 0.0] {
        engine.set_density(density);
        play(&mut engine, bar);
    }
    engine.set_density(1.0);
    for fill in 0..4 {
        play(&mut engine, sixteenth);
        engine.queue_fill(fill);
        play(&mut engine, bar - sixteenth);
    }
    for division in [4, 8, 16, 32] {
        play(&mut engine, bar / 2);
        engine.beat_repeat(Division::from_denominator(division));
        play(&mut engine, bar / 2);
        engine.beat_repeat(None);
    }
    save(&dir.join("gestures.wav"), &gestures)?;

    // Bass: lines A–D alone (drums muted), one bar each; then with their drums, two bars each;
    // then line B with the filter opening over two bars, and two bars in E minor — 16 bars.
    let mut engine = Engine::new(SR, 512, BPM);
    engine.set_metronome(false);
    engine.set_mutes(0b11_1111);
    engine.start();
    let mut bass = Vec::new();
    let mut play = |engine: &mut Engine, frames: usize| {
        let start = bass.len();
        bass.resize(start + frames * 2, 0.0);
        for chunk in bass[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
    };
    for index in 0..4 {
        engine.queue_pattern(index);
        play(&mut engine, bar);
    }
    engine.set_mutes(0);
    for index in 0..4 {
        engine.queue_pattern(index);
        play(&mut engine, bar * 2);
    }
    engine.queue_pattern(1);
    let slices = 32;
    for k in 0..slices {
        engine.set_param(Param::BassCutoff, k as f32 / (slices - 1) as f32);
        play(&mut engine, bar * 2 / slices);
    }
    engine.set_param(Param::BassCutoff, 0.45);
    engine.queue_tonic(4);
    play(&mut engine, bar * 2);
    save(&dir.join("bass.wav"), &bass)?;

    // Styles: each at its suggested tempo, patterns A–D one bar each, 24 bars in all.
    let mut styles = Vec::new();
    for index in 0..engine::style::STYLES as u8 {
        let bpm = engine::style::bpm(index);
        let mut engine = Engine::new(SR, 512, bpm);
        engine.set_metronome(false);
        engine.queue_style(index);
        engine.start();
        let bar = (f64::from(SR) * 60.0 / bpm * 4.0) as usize;
        for pattern in 0..4 {
            engine.queue_pattern(pattern);
            let start = styles.len();
            styles.resize(start + bar * 2, 0.0);
            for chunk in styles[start..].chunks_mut(256) {
                engine.process(chunk, 2);
            }
        }
        println!("  {} — {bpm:.0} BPM", engine::style::name(index));
    }
    save(&dir.join("styles.wav"), &styles)
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
