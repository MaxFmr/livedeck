//! LiveDeck application: UI (eframe/egui) and thread wiring.
//!
//! The main thread is the control thread: it owns the `EngineHandle`, maps controller gestures
//! through `config/mapping.toml`, reads keyboard commands, shows the engine state and rebuilds the
//! audio stream if the device goes away.

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use assert_no_alloc::assert_no_alloc;
use engine::link::{self, Command, EngineEvent, EngineHandle};
use engine::{Engine, Param};
use io::audio::{AudioOutput, start_output};
use io::controller::{ControllerEvent, Deck, Led};
use io::link::{ControllerLink, LinkEvent};
use io::mapping::{Action, DEFAULT_MAPPING, Mapping, Target};

const USAGE: &str = "\
LiveDeck — métronome piloté (test de la plomberie temps réel)

USAGE: app [--bpm <60-200>] [--buffer <frames>] [--mapping <fichier>] [--seconds <n>] [--quiet]
           [--no-metronome]

  --mapping       fichier de mapping (défaut : config/mapping.toml, sinon le mapping intégré)

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
    master_gain: f32,
    metronome_level: f32,
}

/// Control-thread state.
struct Session {
    wanted: Wanted,
    mapping: Mapping,
    /// Last values echoed to the console (continuous controls are echoed by steps only).
    shown_bpm: f64,
    shown_levels: [f32; 2],
}

struct Args {
    bpm: f64,
    buffers: Vec<u32>,
    seconds: Option<f64>,
    quiet: bool,
    metronome: bool,
    mapping: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        bpm: 120.0,
        buffers: vec![128, 256],
        seconds: None,
        quiet: false,
        metronome: true,
        mapping: None,
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
            "--mapping" => args.mapping = Some(PathBuf::from(value("--mapping")?)),
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
    handle.send(Command::SetParam(Param::MasterGain, wanted.master_gain));
    handle.send(Command::SetParam(
        Param::MetronomeLevel,
        wanted.metronome_level,
    ));
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

fn set_tempo(s: &mut Session, handle: &mut EngineHandle, bpm: f64, from_software: bool) {
    s.wanted.bpm = bpm.clamp(engine::clock::MIN_BPM, engine::clock::MAX_BPM);
    handle.send(Command::SetTempo(s.wanted.bpm));
    if (s.wanted.bpm - s.shown_bpm).abs() >= 1.0 {
        s.shown_bpm = s.wanted.bpm;
        println!("  ♩ tempo {:.1} BPM", s.wanted.bpm);
    }
    if from_software {
        // Controls bound to the tempo must meet the new value before taking over (pickup).
        s.mapping.software_changed(Target::Tempo, s.wanted.bpm);
    }
}

fn toggle_metronome(s: &mut Session, handle: &mut EngineHandle) {
    s.wanted.metronome = !s.wanted.metronome;
    handle.send(Command::Metronome(s.wanted.metronome));
    println!(
        "métronome {}",
        if s.wanted.metronome { "on" } else { "off" }
    );
}

/// Controller gesture → mapped action.
fn handle_controller(
    ts_us: u64,
    event: ControllerEvent,
    s: &mut Session,
    handle: &mut EngineHandle,
    quiet: bool,
) {
    let discrete = !matches!(
        event,
        ControllerEvent::Mixer { .. }
            | ControllerEvent::Knob { .. }
            | ControllerEvent::Tempo { .. }
            | ControllerEvent::Jog { .. }
    );
    // Continuous controls flood the console: only discrete gestures are echoed.
    if discrete && !quiet {
        println!("  🎛 {event}");
    }
    let Some(action) = s.mapping.handle(ts_us, event) else {
        return;
    };
    match action {
        Action::TransportToggle => toggle_play(&mut s.wanted, handle),
        Action::TransportRestart => {
            s.wanted.playing = true;
            handle.send(Command::Start);
            println!("⏮ reprise à la mesure 1");
        }
        Action::MetronomeToggle => toggle_metronome(s, handle),
        // Absolute tempo comes from the bound fader (hardware) or tap tempo (already registered
        // as a software change by the mapping).
        Action::SetTempo(bpm) => set_tempo(s, handle, bpm, false),
        Action::NudgeTempo(delta) => {
            let bpm = s.wanted.bpm + delta;
            set_tempo(s, handle, bpm, true);
        }
        Action::SetLevel(target, value) => {
            let (param, slot, name) = match target {
                Target::MasterGain => {
                    s.wanted.master_gain = value;
                    (Param::MasterGain, 0, "master")
                }
                Target::MetronomeLevel => {
                    s.wanted.metronome_level = value;
                    (Param::MetronomeLevel, 1, "métronome")
                }
                Target::Tempo => return,
            };
            if let Some(shown) = s.shown_levels.get_mut(slot)
                && (value - *shown).abs() >= 0.1
            {
                *shown = value;
                println!("  🔊 niveau {name} {:.0} %", value * 100.0);
            }
            handle.send(Command::SetParam(param, value));
        }
    }
}

/// Applies one keyboard line; returns false to quit.
fn handle_line(line: &str, s: &mut Session, handle: &mut EngineHandle) -> bool {
    let line = line.trim().to_lowercase();
    let line = line.as_str();
    let tempo = |s: &mut Session, handle: &mut EngineHandle, bpm: f64| {
        set_tempo(s, handle, bpm, true);
        println!("tempo {:.0} BPM", s.wanted.bpm);
    };
    match line {
        "q" => return false,
        "p" => toggle_play(&mut s.wanted, handle),
        "m" => toggle_metronome(s, handle),
        "" => {}
        // Markers written into the session log by test scripts (tools/try-mapping.sh).
        marker if marker.starts_with('#') => println!("── {marker}"),
        steps if steps.chars().all(|c| c == '+') => {
            tempo(s, handle, s.wanted.bpm + steps.len() as f64);
        }
        steps if steps.chars().all(|c| c == '-') => {
            tempo(s, handle, s.wanted.bpm - steps.len() as f64);
        }
        other => match other.strip_prefix("t ").map(|v| v.trim().parse::<f64>()) {
            Some(Ok(bpm)) => tempo(s, handle, bpm),
            _ => println!("commande inconnue : {other} (p, +, -, t <bpm>, m, q)"),
        },
    }
    true
}

/// Loads the mapping: explicit path, else `config/mapping.toml` if present, else the built-in one.
fn load_mapping(path: Option<&PathBuf>) -> Result<(Mapping, String), String> {
    let default_path = PathBuf::from("config/mapping.toml");
    match path {
        Some(p) => Mapping::load(p).map(|m| (m, p.display().to_string())),
        None if default_path.exists() => {
            Mapping::load(&default_path).map(|m| (m, default_path.display().to_string()))
        }
        None => Mapping::from_toml_str(DEFAULT_MAPPING).map(|m| (m, "intégré".to_string())),
    }
    .map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let _awake = io::system::keep_alive("LiveDeck live session");
    let (mapping, mapping_source) = load_mapping(args.mapping.as_ref())?;
    let mut s = Session {
        wanted: Wanted {
            playing: true,
            bpm: args
                .bpm
                .clamp(engine::clock::MIN_BPM, engine::clock::MAX_BPM),
            metronome: args.metronome,
            master_gain: 1.0,
            metronome_level: 0.7,
        },
        mapping,
        shown_bpm: 0.0,
        shown_levels: [-1.0; 2],
    };
    println!("Mapping : {mapping_source} ({} liaisons)", s.mapping.len());
    s.mapping.software_changed(Target::Tempo, s.wanted.bpm);
    s.mapping
        .software_changed(Target::MasterGain, f64::from(s.wanted.master_gain));
    s.mapping
        .software_changed(Target::MetronomeLevel, f64::from(s.wanted.metronome_level));
    let (mut output, mut handle) = open(&args.buffers, s.wanted)?;
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
                LinkEvent::Input { ts_us, event } => {
                    handle_controller(ts_us, event, &mut s, &mut handle, args.quiet);
                }
            }
        }
        while let Ok(line) = keys.try_recv() {
            if !handle_line(&line, &mut s, &mut handle) {
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
            link.leds_mut().set(Led::Play(deck), s.wanted.playing);
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
                match open(&args.buffers, s.wanted) {
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
            if let Some(hw) = s.mapping.pending_pickup(Target::Tempo) {
                println!(
                    "  ⇅ fader Tempo à {:.0} % : le ramener vers {:.0} BPM pour reprendre la main",
                    hw * 100.0,
                    s.wanted.bpm
                );
            }
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
