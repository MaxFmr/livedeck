//! Renders the drum kit for listening: `target/renders/kit.wav` (each voice alone, then a few bars
//! of groove), `target/renders/patterns.wav` (the sequencer's patterns A–D, two bars each) and
//! `target/renders/gestures.wav` (density, fills and beat-repeat on groove B) and
//! `target/renders/bass.wav` (the four bass lines alone, then with their drums, a filter sweep and
//! a key change) and `target/renders/styles.wav` (each style at its suggested tempo, patterns A–D,
//! one bar each) and `target/renders/chords.wav` (each style's synth patch on I–vi–IV–V over its
//! pattern A) and `target/renders/voicings.wav` (I–vi–IV–V: triads, inversions, 7ths, then in
//! dorian and major) and `target/renders/arp.wav` (latched I–vi–IV–V arpeggiated up, up-down
//! on 2 octaves, random in 1/16 triplets, then phrases 1–4 of the hybrid style, a variation and a
//! transposition) and `target/renders/fx.wav` (Dub kit at 118: a phrase over latched chords,
//! dry, then with delay, a long-feedback throw, the reverb added, each delay time, and the tails
//! ringing out; prints the render load).
//! Offline, so it may allocate and write files.
//!
//! cargo run -p engine --example render_kit --release

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use engine::bass::Scale;
use engine::chords::{Extension, Voicing};
use engine::drums::DrumVoice;
use engine::melody::{ArpMode, ArpRate, ArpSettings};
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
    save(&dir.join("styles.wav"), &styles)?;

    // Chords: each style at its tempo, I–vi–IV–V, one bar each, held three beats.
    let mut chords = Vec::new();
    for index in 0..engine::style::STYLES as u8 {
        let bpm = engine::style::bpm(index);
        let mut engine = Engine::new(SR, 512, bpm);
        engine.set_metronome(false);
        engine.queue_style(index);
        engine.start();
        let beat = (f64::from(SR) * 60.0 / bpm) as usize;
        for pad in [0, 3, 1, 2] {
            engine.chord(pad, true);
            let start = chords.len();
            chords.resize(start + beat * 4 * 2, 0.0);
            let (held, rest) = chords[start..].split_at_mut(beat * 3 * 2);
            for chunk in held.chunks_mut(256) {
                engine.process(chunk, 2);
            }
            engine.chord(pad, false);
            for chunk in rest.chunks_mut(256) {
                engine.process(chunk, 2);
            }
        }
    }
    save(&dir.join("chords.wav"), &chords)?;

    // Voicings on the hybrid pad at 120 BPM, drums at low density: four passes of I–vi–IV–V.
    let mut engine = Engine::new(SR, 512, BPM);
    engine.set_metronome(false);
    engine.set_density(0.0);
    engine.start();
    let mut voicings = Vec::new();
    let passes = [
        (Voicing::default(), Scale::Minor),
        // Inversions that keep the hands close: I root, vi first, IV second, V first.
        (
            Voicing {
                inversion: 1,
                ..Voicing::default()
            },
            Scale::Minor,
        ),
        (
            Voicing {
                extension: Extension::Seventh,
                ..Voicing::default()
            },
            Scale::Minor,
        ),
        (
            Voicing {
                extension: Extension::Add9,
                ..Voicing::default()
            },
            Scale::Dorian,
        ),
    ];
    for (voicing, scale) in passes {
        engine.queue_scale(scale);
        engine.set_voicing(voicing);
        for pad in [0, 3, 1, 2] {
            let start = voicings.len();
            voicings.resize(start + bar * 2, 0.0);
            let (held, rest) = voicings[start..].split_at_mut(bar * 2 * 7 / 8);
            // One frame first: the bar's first step applies the queued scale, then the chord.
            let (first, held) = held.split_at_mut(2);
            engine.process(first, 2);
            engine.chord(pad, true);
            for chunk in held.chunks_mut(256) {
                engine.process(chunk, 2);
            }
            engine.chord(pad, false);
            for chunk in rest.chunks_mut(256) {
                engine.process(chunk, 2);
            }
        }
    }
    save(&dir.join("voicings.wav"), &voicings)?;

    // Arpeggios and phrases at 120 BPM, drums at low density.
    let mut engine = Engine::new(SR, 512, BPM);
    engine.set_metronome(false);
    engine.set_density(0.0);
    engine.set_latch(true);
    engine.start();
    let mut arp = Vec::new();
    let mut play_bar = |engine: &mut Engine| {
        let start = arp.len();
        arp.resize(start + bar * 2, 0.0);
        for chunk in arp[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
    };
    let modes = [
        (ArpMode::Up, ArpRate::Sixteenth, 1),
        (ArpMode::UpDown, ArpRate::Sixteenth, 2),
        (ArpMode::Random, ArpRate::SixteenthTriplet, 1),
    ];
    for (mode, rate, octaves) in modes {
        engine.set_arp(ArpSettings {
            mode,
            rate,
            octaves,
        });
        for pad in [0, 3, 1, 2] {
            engine.chord(pad, true);
            engine.chord(pad, false);
            play_bar(&mut engine);
        }
    }
    // Phrases over a latched I (no arpeggio), each started at the bar.
    engine.set_arp(ArpSettings::default());
    engine.chord(0, true);
    engine.chord(0, false);
    for (index, variation, transpose) in [
        (0, false, 0),
        (1, false, 0),
        (2, false, 0),
        (3, false, 0),
        (3, true, 0),
        (0, false, 2),
    ] {
        engine.press_phrase(index, variation);
        engine.set_transpose(transpose);
        play_bar(&mut engine);
        play_bar(&mut engine);
    }
    save(&dir.join("arp.wav"), &arp)?;

    // Send effects on the Dub kit at 118 BPM.
    let mut engine = Engine::new(SR, 512, 118.0);
    engine.set_metronome(false);
    engine.queue_style(2);
    engine.set_density(0.4);
    engine.set_latch(true);
    engine.start();
    let bar = (f64::from(SR) * 60.0 / 140.0 * 4.0) as usize;
    let mut fx = Vec::new();
    let mut busy = std::time::Duration::ZERO;
    let mut play_bars = |engine: &mut Engine, bars: usize| {
        let start = fx.len();
        fx.resize(start + bar * bars * 2, 0.0);
        let t = std::time::Instant::now();
        for chunk in fx[start..].chunks_mut(256) {
            engine.process(chunk, 2);
        }
        busy += t.elapsed();
    };
    engine.chord(0, true);
    engine.chord(0, false);
    engine.press_phrase(0, false);
    play_bars(&mut engine, 2);
    engine.set_param(Param::DelaySend, 0.8);
    engine.set_param(Param::DelayFeedback, 0.5);
    play_bars(&mut engine, 2);
    engine.set_param(Param::DelayFeedback, 0.95);
    play_bars(&mut engine, 2);
    engine.set_param(Param::DelayFeedback, 0.5);
    engine.set_param(Param::ReverbSend, 0.7);
    engine.chord(3, true);
    engine.chord(3, false);
    play_bars(&mut engine, 2);
    for _ in 0..4 {
        engine.cycle_delay_time();
        play_bars(&mut engine, 1);
    }
    // Phrase stopped and the sends closed: only the tails remain.
    engine.press_phrase(0, false);
    play_bars(&mut engine, 1);
    engine.set_latch(false);
    engine.set_param(Param::DelaySend, 0.0);
    play_bars(&mut engine, 2);
    let seconds = fx.len() as f64 / 2.0 / f64::from(SR);
    println!(
        "fx : rendu {:.1} % du temps réel",
        busy.as_secs_f64() / seconds * 100.0
    );
    save(&dir.join("fx.wav"), &fx)
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
