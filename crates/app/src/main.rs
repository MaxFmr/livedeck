//! LiveDeck application: UI (eframe/egui) and thread wiring.
//!
//! Iteration 004: command-line metronome to validate the audio path (no UI yet).

use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::{Duration, Instant};

use engine::Engine;

const USAGE: &str = "\
LiveDeck — métronome de test de la sortie audio

USAGE: app [--bpm <60-200>] [--buffer <frames>] [--seconds <n>]

  --bpm       tempo (défaut 120)
  --buffer    taille de bloc demandée (défaut : 128, sinon 256, sinon celle du périphérique)
  --seconds   durée avant arrêt (défaut : jusqu'à Ctrl+C)";

struct Args {
    bpm: f64,
    buffers: Vec<u32>,
    seconds: Option<f64>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        bpm: 120.0,
        buffers: vec![128, 256],
        seconds: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} attend une valeur"));
        match arg.as_str() {
            "--bpm" => {
                let v = value("--bpm")?;
                args.bpm = v.parse().map_err(|_| format!("tempo invalide : {v}"))?;
            }
            "--buffer" => {
                let v = value("--buffer")?;
                args.buffers = vec![v.parse().map_err(|_| format!("taille invalide : {v}"))?];
            }
            "--seconds" => {
                let v = value("--seconds")?;
                args.seconds = Some(v.parse().map_err(|_| format!("durée invalide : {v}"))?);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let mut bpm = args.bpm;
    let output = io::audio::start_output(&args.buffers, |info| {
        // Built (and allocated) here at the device rate, before the stream starts, then moved
        // into the callback: the audio thread owns it exclusively.
        let mut engine = Engine::new(info.sample_rate, 4096, args.bpm);
        engine.start();
        bpm = engine.clock().bpm();
        move |data: &mut [f32], channels: usize| engine.process(data, channels)
    })
    .map_err(|e| e.to_string())?;

    let info = &output.info;
    let buffer = info.buffer_frames.map_or_else(
        || "défaut du périphérique".to_string(),
        |f| format!("{f} frames"),
    );
    println!(
        "Sortie : {} — {} Hz, {} canaux, buffer {buffer} — métronome {bpm:.0} BPM",
        info.device, info.sample_rate, info.channels
    );

    let started = Instant::now();
    let mut worst = 0.0_f64;
    loop {
        sleep(Duration::from_secs(1));
        let load = output.stats.take_max_load(info.sample_rate);
        worst = worst.max(load);
        println!(
            "{:>5.0} s │ charge max {:>5.1} % (pire {:>5.1} %) │ xruns {} │ erreurs {} │ blocs {}",
            started.elapsed().as_secs_f64(),
            load * 100.0,
            worst * 100.0,
            output.stats.xruns.load(Ordering::Relaxed),
            output.stats.errors.load(Ordering::Relaxed),
            output.stats.callbacks.load(Ordering::Relaxed),
        );
        if args
            .seconds
            .is_some_and(|s| started.elapsed().as_secs_f64() >= s)
        {
            break;
        }
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
