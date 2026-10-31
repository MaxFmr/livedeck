//! LiveDeck application: UI (eframe/egui) and thread wiring.
//!
//! Iteration 005: the main thread is the control thread. It owns the `EngineHandle`, reads keyboard
//! commands, shows the engine state and rebuilds the audio stream if the device goes away.

use std::io::BufRead;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use engine::Engine;
use engine::link::{self, Command, EngineEvent, EngineHandle};
use io::audio::{AudioOutput, start_output};

const USAGE: &str = "\
LiveDeck — métronome piloté (test de la plomberie temps réel)

USAGE: app [--bpm <60-200>] [--buffer <frames>]

Commandes (taper puis Entrée) :
  p        lecture / arrêt
  + / -    tempo ±1 BPM par signe (+++ = +3)
  t <bpm>  tempo exact         m        métronome on/off
  q        quitter";

/// What the control thread wants; the engine is rebuilt from it if the stream must be recreated.
#[derive(Debug, Clone, Copy)]
struct Wanted {
    playing: bool,
    bpm: f64,
    metronome: bool,
}

struct Args {
    bpm: f64,
    buffers: Vec<u32>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        bpm: 120.0,
        buffers: vec![128, 256],
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
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

/// Builds the engine at the device rate, starts the stream and replays the wanted state.
fn open(buffers: &[u32], wanted: Wanted) -> Result<(AudioOutput, EngineHandle), String> {
    let mut handle = None;
    let output = start_output(buffers, |info| {
        let engine = Engine::new(info.sample_rate, 4096, wanted.bpm);
        let (mut audio, control) = link::connect(engine);
        handle = Some(control);
        move |data: &mut [f32], channels: usize| audio.process(data, channels)
    })
    .map_err(|e| e.to_string())?;
    let mut handle = handle.ok_or("le moteur n'a pas été construit")?;
    handle.send(Command::Metronome(wanted.metronome));
    if wanted.playing {
        handle.send(Command::Start);
    }
    let info = &output.info;
    let buffer = info
        .buffer_frames
        .map_or_else(|| "défaut".to_string(), |f| format!("{f} frames"));
    println!(
        "Sortie : {} — {} Hz, {} canaux, buffer {buffer}",
        info.device, info.sample_rate, info.channels
    );
    Ok((output, handle))
}

/// Applies one keyboard line; returns false to quit.
fn handle_line(line: &str, wanted: &mut Wanted, handle: &mut EngineHandle) -> bool {
    let line = line.trim().to_lowercase();
    let line = line.as_str();
    let tempo = |wanted: &mut Wanted, handle: &mut EngineHandle, bpm: f64| {
        wanted.bpm = bpm.clamp(engine::clock::MIN_BPM, engine::clock::MAX_BPM);
        handle.send(Command::SetTempo(wanted.bpm));
        println!("tempo {:.0} BPM", wanted.bpm);
    };
    match line {
        "q" => return false,
        "p" => {
            wanted.playing = !wanted.playing;
            handle.send(if wanted.playing {
                Command::Start
            } else {
                Command::Stop
            });
            println!(
                "{}",
                if wanted.playing {
                    "▶ lecture"
                } else {
                    "■ arrêt"
                }
            );
        }
        "m" => {
            wanted.metronome = !wanted.metronome;
            handle.send(Command::Metronome(wanted.metronome));
            println!("métronome {}", if wanted.metronome { "on" } else { "off" });
        }
        "" => {}
        steps if steps.chars().all(|c| c == '+') => {
            tempo(wanted, handle, wanted.bpm + steps.len() as f64);
        }
        steps if steps.chars().all(|c| c == '-') => {
            tempo(wanted, handle, wanted.bpm - steps.len() as f64);
        }
        other => match other.strip_prefix("t ").map(|v| v.trim().parse::<f64>()) {
            Some(Ok(bpm)) => tempo(wanted, handle, bpm),
            _ => println!("commande inconnue : {other} (p, +, -, t <bpm>, m, q)"),
        },
    }
    true
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let _awake = io::system::keep_alive("LiveDeck live session");
    let mut wanted = Wanted {
        playing: true,
        bpm: args
            .bpm
            .clamp(engine::clock::MIN_BPM, engine::clock::MAX_BPM),
        metronome: true,
    };
    let (mut output, mut handle) = open(&args.buffers, wanted)?;
    println!("{USAGE}\n");

    let (keys_tx, keys) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if keys_tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut last_report = Instant::now();
    loop {
        while let Ok(line) = keys.try_recv() {
            if !handle_line(&line, &mut wanted, &mut handle) {
                return Ok(());
            }
        }
        while let Some(EngineEvent::Beat { bar, beat }) = handle.next_event() {
            let marker = if beat == 1 { "●" } else { "○" };
            println!("{marker} mesure {bar:>3} · temps {beat}");
        }
        if output.stats.invalidated.load(Ordering::Relaxed) {
            println!("⚠ sortie audio perdue, reconstruction…");
            drop(output);
            loop {
                sleep(Duration::from_millis(500));
                match open(&args.buffers, wanted) {
                    Ok(reopened) => {
                        (output, handle) = reopened;
                        break;
                    }
                    Err(e) => println!("  nouvel essai ({e})"),
                }
            }
        }
        if last_report.elapsed() >= Duration::from_secs(5) {
            last_report = Instant::now();
            let snap = handle.snapshot();
            println!(
                "  [{:.0} BPM │ charge max {:.1} % │ xruns {} │ NaN {} │ commandes {}]",
                snap.bpm,
                output.stats.take_max_load(output.info.sample_rate) * 100.0,
                output.stats.xruns.load(Ordering::Relaxed),
                snap.nan_count,
                snap.commands
            );
        }
        sleep(Duration::from_millis(20));
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
