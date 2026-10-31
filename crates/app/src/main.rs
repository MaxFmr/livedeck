//! LiveDeck application: UI (eframe/egui) and thread wiring.
//!
//! Iteration 005: the main thread is the control thread. It owns the `EngineHandle`, reads keyboard
//! commands, shows the engine state and rebuilds the audio stream if the device goes away.

use std::io::BufRead;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use assert_no_alloc::assert_no_alloc;
use engine::Engine;
use engine::link::{self, Command, EngineEvent, EngineHandle};
use io::audio::{AudioOutput, start_output};
use io::controller::{Button, ControllerEvent, Deck, Led};
use io::link::{ControllerLink, LinkEvent};

const USAGE: &str = "\
LiveDeck — métronome piloté (test de la plomberie temps réel)

USAGE: app [--bpm <60-200>] [--buffer <frames>] [--seconds <n>] [--quiet] [--no-metronome]

  --seconds       s'arrête après n secondes (endurance)
  --quiet         n'affiche pas chaque temps, seulement les bilans et les incidents
  --no-metronome  démarre métronome coupé (le moteur tourne en silence)

Commandes (taper puis Entrée) :
  p        lecture / arrêt
  + / -    tempo ±1 BPM par signe (+++ = +3)
  t <bpm>  tempo exact         m        métronome on/off
  q        quitter";

// Debug builds abort on any allocation inside `assert_no_alloc` (the audio callback); release
// builds use the system allocator unchanged.
#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

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
    seconds: Option<f64>,
    quiet: bool,
    metronome: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        bpm: 120.0,
        buffers: vec![128, 256],
        seconds: None,
        quiet: false,
        metronome: true,
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
            "--quiet" => args.quiet = true,
            "--no-metronome" => args.metronome = false,
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
        move |data: &mut [f32], channels: usize| assert_no_alloc(|| audio.process(data, channels))
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

fn toggle_play(wanted: &mut Wanted, handle: &mut EngineHandle) {
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

/// Controller → action. Iteration 007 only maps Play; the configurable mapping arrives in 008.
fn handle_controller(
    event: ControllerEvent,
    wanted: &mut Wanted,
    handle: &mut EngineHandle,
    quiet: bool,
) {
    match event {
        ControllerEvent::Button {
            button: Button::Play,
            shift: false,
            pressed: true,
            ..
        } => toggle_play(wanted, handle),
        // Continuous controls flood the console: only discrete gestures are echoed.
        ControllerEvent::Mixer { .. }
        | ControllerEvent::Knob { .. }
        | ControllerEvent::Tempo { .. }
        | ControllerEvent::Jog { .. } => {}
        other if !quiet => println!("  🎛 {other}"),
        _ => {}
    }
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
        "p" => toggle_play(wanted, handle),
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
        metronome: args.metronome,
    };
    let (mut output, mut handle) = open(&args.buffers, wanted)?;
    let session = Instant::now();
    let mut seen_xruns = 0;
    let mut seen_changes = 0;
    println!("{USAGE}\n");

    let (keys_tx, keys) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if keys_tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut link = ControllerLink::new();
    let mut link_events = Vec::new();
    // Beat flashes on Sync (downbeat) / Cue (other beats), switched off after FLASH.
    const FLASH: Duration = Duration::from_millis(120);
    let mut flash_until: Option<Instant> = None;

    let mut last_report = Instant::now();
    loop {
        link.poll(|event| link_events.push(event));
        for event in link_events.drain(..) {
            match event {
                LinkEvent::Connected { port, leds } => println!(
                    "🎛 contrôleur connecté : {port}{}",
                    if leds {
                        ""
                    } else {
                        " (sans sortie : LEDs inactives)"
                    }
                ),
                LinkEvent::Disconnected => {
                    println!(
                        "🎛 contrôleur débranché — le son continue, en attente de reconnexion…"
                    );
                }
                LinkEvent::Input { event, .. } => {
                    handle_controller(event, &mut wanted, &mut handle, args.quiet);
                }
            }
        }
        while let Ok(line) = keys.try_recv() {
            if !handle_line(&line, &mut wanted, &mut handle) {
                return Ok(());
            }
        }
        while let Some(EngineEvent::Beat { bar, beat }) = handle.next_event() {
            for deck in [Deck::A, Deck::B] {
                link.leds_mut().set(Led::Sync(deck), beat == 1);
                link.leds_mut().set(Led::Cue(deck), beat != 1);
            }
            flash_until = Some(Instant::now() + FLASH);
            if !args.quiet {
                let marker = if beat == 1 { "●" } else { "○" };
                println!("{marker} mesure {bar:>3} · temps {beat}");
            }
        }
        if flash_until.is_some_and(|t| Instant::now() >= t) {
            flash_until = None;
            for deck in [Deck::A, Deck::B] {
                link.leds_mut().set(Led::Sync(deck), false);
                link.leds_mut().set(Led::Cue(deck), false);
            }
        }
        for deck in [Deck::A, Deck::B] {
            link.leds_mut().set(Led::Play(deck), wanted.playing);
        }
        let xruns = output.stats.xruns.load(Ordering::Relaxed);
        if xruns != seen_xruns {
            seen_xruns = xruns;
            let at = output.stats.last_xrun_at_ms.load(Ordering::Relaxed) as f64 / 1000.0;
            println!(
                "⚠ xrun n°{xruns} à {at:.3} s du démarrage du flux ({:.1} s de session)",
                session.elapsed().as_secs_f64()
            );
        }
        let changes = output.stats.device_changes.load(Ordering::Relaxed);
        if changes != seen_changes {
            seen_changes = changes;
            println!("↪ sortie re-routée par macOS (n°{changes})");
        }
        if args
            .seconds
            .is_some_and(|s| session.elapsed().as_secs_f64() >= s)
        {
            return Ok(());
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
                "  [{:.0} BPM │ charge max {:.1} % │ xruns {} │ NaN {} │ commandes {} │ contrôleur {}]",
                snap.bpm,
                output.stats.take_max_load(output.info.sample_rate) * 100.0,
                output.stats.xruns.load(Ordering::Relaxed),
                snap.nan_count,
                snap.commands,
                if link.is_connected() {
                    format!("ok, {} perdus", link.dropped())
                } else {
                    "absent".to_string()
                }
            );
        }
        // Control loop period: bounds the latency of controller gestures that go through it.
        sleep(Duration::from_millis(2));
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
