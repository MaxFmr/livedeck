//! Renders `target/renders/samples.wav`: for each style, pattern B for two bars on the synth, then
//! two bars on the TR-909 samples of `assets/kits/tr909` (bass muted, metronome off). Offline.
//!
//! cargo run -p io --example render_samples --release

use std::path::Path;

use engine::Engine;

const SR: u32 = 48_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bank = io::samples::load_kit(Path::new(io::samples::DEFAULT_KIT))?;
    let mut out: Vec<f32> = Vec::new();
    for style in 0..engine::style::STYLES as u8 {
        let bpm = engine::style::bpm(style);
        let mut engine = Engine::new(SR, 512, bpm);
        engine.set_sample_bank(bank.clone());
        engine.set_metronome(false);
        engine.set_bass_mute(true);
        engine.queue_style(style);
        engine.queue_pattern(1);
        engine.start();
        let bars = (f64::from(SR) * 60.0 / bpm * 8.0) as usize;
        for samples in [false, true] {
            engine.use_samples(samples);
            let start = out.len();
            out.resize(start + bars * 2, 0.0);
            for chunk in out[start..].chunks_mut(256) {
                engine.process(chunk, 2);
            }
            let part = &out[start..];
            let rms = (part.iter().map(|s| s * s).sum::<f32>() / part.len() as f32).sqrt();
            println!(
                "{:<18} {:<8} RMS {:6.1} dB",
                engine::style::name(style),
                if samples { "samples" } else { "synthé" },
                20.0 * rms.log10()
            );
        }
    }
    std::fs::create_dir_all("target/renders")?;
    let path = Path::new("target/renders/samples.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for s in &out {
        writer.write_sample((s.clamp(-1.0, 1.0) * 32_767.0) as i16)?;
    }
    writer.finalize()?;
    println!(
        "{} — {:.1} s",
        path.display(),
        out.len() as f64 / 2.0 / f64::from(SR)
    );
    Ok(())
}
