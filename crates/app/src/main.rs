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
use engine::drums::DrumVoice;
use engine::link::{self, BlockTime, Command, EngineEvent, EngineHandle};
use engine::{Engine, Param};
use io::audio::{AudioOutput, BlockStamp, start_output};
use io::controller::{ControllerEvent, Deck, Led, PadMode};
use io::leds::{LedBank, LedState};
use io::link::{ControllerLink, LinkEvent};
use io::mapping::{
    Action, ActionName, Control, DEFAULT_MAPPING, Mapping, Target, identify, led_of,
};
use io::system::host_time_us;

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
  h        kick (comme le pad Hot Cue 1 du deck A)
  q        quitter

Pads Hot Cue : batterie (A : kick, snare, hat fermé, hat ouvert ; B : clap, perc), avec la\nlatence geste → son mesurée à chaque frappe.";

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
    drum_level: f32,
}

/// Control-thread state.
struct Session {
    wanted: Wanted,
    mapping: Mapping,
    /// Last values echoed to the console (continuous controls are echoed by steps only).
    shown_bpm: f64,
    shown_levels: [f32; 3],
    leds: LedRoles,
    /// LEDs of pads currently held down (lit while held).
    held: Vec<Led>,
    /// Test hits sent to the engine: (gesture host time, host time the control thread sent it).
    pending_hits: Vec<(u64, u64)>,
    latency: Latency,
    /// Last time a tempo control moved without effect because it waits for its pickup.
    tempo_ignored_at: Option<Instant>,
}

/// How long the tap-tempo LED asks for attention after a tempo control moved without effect.
const PICKUP_HINT: Duration = Duration::from_secs(3);

/// LEDs of the controls bound to actions that have a state to show.
struct LedRoles {
    transport: Vec<Led>,
    metronome: Vec<Led>,
    tap_tempo: Vec<Led>,
}

impl LedRoles {
    fn of(mapping: &Mapping) -> Self {
        let leds = |action| mapping.bound(action).filter_map(led_of).collect();
        Self {
            transport: leds(ActionName::TransportToggle),
            metronome: leds(ActionName::MetronomeToggle),
            tap_tempo: leds(ActionName::TapTempo),
        }
    }
}

/// Gesture → sound latencies of the session, in ms.
#[derive(Default)]
struct Latency {
    totals: Vec<f64>,
    reported: usize,
}

impl Latency {
    /// "n, min / median / max" of the measured totals.
    fn summary(&self) -> Option<String> {
        let mut sorted = self.totals.clone();
        sorted.sort_by(f64::total_cmp);
        let (first, last) = (sorted.first()?, sorted.last()?);
        let median = sorted.get(sorted.len() / 2)?;
        Some(format!(
            "{} frappes : min {first:.1} · médiane {median:.1} · max {last:.1} ms",
            sorted.len()
        ))
    }
}

fn ms(from_us: u64, to_us: u64) -> f64 {
    (to_us as f64 - from_us as f64) / 1000.0
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
        move |data: &mut [f32], channels: usize, stamp: BlockStamp| {
            let time = BlockTime {
                called_us: stamp.called_us,
                callback_us: stamp.callback_us,
                playback_us: stamp.playback_us,
            };
            assert_no_alloc(|| audio.process_at(data, channels, time));
        }
    })
    .map_err(|e| e.to_string())?;
    let mut handle = handle.ok_or("le moteur n'a pas été construit")?;
    handle.send(Command::Metronome(wanted.metronome));
    handle.send(Command::SetParam(Param::MasterGain, wanted.master_gain));
    handle.send(Command::SetParam(
        Param::MetronomeLevel,
        wanted.metronome_level,
    ));
    handle.send(Command::SetParam(Param::DrumLevel, wanted.drum_level));
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
    if let ControllerEvent::Pad {
        deck,
        mode,
        pad,
        pressed,
        ..
    } = event
        && let Some(led) = led_of(Control::Pad(deck, mode, pad))
    {
        s.held.retain(|&l| l != led);
        if pressed {
            s.held.push(led);
        }
    }
    let Some(action) = s.mapping.handle(ts_us, event) else {
        if matches!(event, ControllerEvent::Tempo { .. })
            && s.mapping.pending_pickup(Target::Tempo).is_some()
        {
            s.tempo_ignored_at = Some(Instant::now());
        }
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
        Action::Drum {
            voice,
            velocity,
            ts_us,
        } => play_drum(s, handle, voice, velocity, ts_us),
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
                Target::DrumLevel => {
                    s.wanted.drum_level = value;
                    (Param::DrumLevel, 2, "batterie")
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

/// Plays a drum voice; its latency is printed when the engine reports it.
fn play_drum(
    s: &mut Session,
    handle: &mut EngineHandle,
    voice: DrumVoice,
    velocity: f32,
    gesture_us: u64,
) {
    let command = Command::Drum {
        voice,
        velocity,
        gesture_us,
    };
    if handle.send(command) {
        // Bounded: entries whose report never came (stream rebuilt) do not pile up.
        if s.pending_hits.len() >= 64 {
            s.pending_hits.remove(0);
        }
        s.pending_hits.push((gesture_us, host_time_us()));
    }
}

/// The engine played a gesture's drum hit: prints its gesture → sound latency, split by stage.
fn report_hit(s: &mut Session, gesture_us: u64, time: BlockTime) {
    let Some(at) = s.pending_hits.iter().position(|&(g, _)| g == gesture_us) else {
        return;
    };
    let (_, sent_us) = s.pending_hits.remove(at);
    if gesture_us == 0 || time.called_us == 0 || time.callback_us == 0 {
        println!("  ⏱ son de test joué (horodatage indisponible)");
        return;
    }
    // CoreAudio stamps the block with the time it is handed to the device (one buffer plus the
    // safety offset after the call); cpal's playback estimate adds buffer + safety offset + device
    // latency to that stamp, so the device latency alone is the difference of the two.
    let buffer = ms(time.called_us, time.callback_us);
    let device = (ms(time.callback_us, time.playback_us) - buffer).max(0.0);
    let total = ms(gesture_us, time.callback_us) + device;
    s.latency.totals.push(total);
    println!(
        "  ⏱ geste → sortie ≈ {total:.1} ms (MIDI → boucle {:.1} · attente du bloc {:.1} · \
         tampon {buffer:.1} · matériel {device:.1})",
        ms(gesture_us, sent_us),
        ms(sent_us, time.called_us),
    );
}

/// Sets every LED from the session state; blinks and flashes are drawn against `beats`.
fn update_leds(leds: &mut LedBank, s: &Session, beats: f64) {
    leds.set_beats(beats);
    let playing = s.wanted.playing;
    for deck in [Deck::A, Deck::B] {
        // Beat display while playing: Sync on beat 1, Cue on the other beats.
        let (sync, cue) = if playing {
            (LedState::FlashDownbeat, LedState::FlashOtherBeats)
        } else {
            (LedState::Off, LedState::Off)
        };
        leds.set_state(Led::Sync(deck), sync);
        leds.set_state(Led::Cue(deck), cue);
        leds.set_state(Led::Play(deck), LedState::Off);
        // Pads are lit only while held (set below).
        for mode in PadMode::ALL {
            for pad in 0..4 {
                leds.set(Led::Pad { deck, mode, pad }, false);
            }
        }
    }
    for &led in &s.leds.transport {
        // Stopped = waiting to start: blinks in tempo.
        leds.set_state(
            led,
            if playing {
                LedState::On
            } else {
                LedState::Blink
            },
        );
    }
    for &led in &s.leds.metronome {
        leds.set(led, s.wanted.metronome);
    }
    let ignored = s
        .tempo_ignored_at
        .is_some_and(|t| t.elapsed() < PICKUP_HINT);
    if ignored && s.mapping.pending_pickup(Target::Tempo).is_some() {
        // The tempo fader was moved but does nothing until it meets the current tempo.
        for &led in &s.leds.tap_tempo {
            leds.set_state(led, LedState::BlinkFast);
        }
    }
    for &led in &s.held {
        leds.set(led, true);
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
        "h" => play_drum(s, handle, DrumVoice::Kick, 1.0, host_time_us()),
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
            _ => println!("commande inconnue : {other} (p, +, -, t <bpm>, m, h, q)"),
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
            drum_level: 0.8,
        },
        leds: LedRoles::of(&mapping),
        mapping,
        shown_bpm: 0.0,
        shown_levels: [-1.0; 3],
        held: Vec::new(),
        pending_hits: Vec::new(),
        latency: Latency::default(),
        tempo_ignored_at: None,
    };
    println!("Mapping : {mapping_source} ({} liaisons)", s.mapping.len());
    s.mapping.software_changed(Target::Tempo, s.wanted.bpm);
    s.mapping
        .software_changed(Target::MasterGain, f64::from(s.wanted.master_gain));
    s.mapping
        .software_changed(Target::MetronomeLevel, f64::from(s.wanted.metronome_level));
    s.mapping
        .software_changed(Target::DrumLevel, f64::from(s.wanted.drum_level));
    let (mut output, mut handle) = open(&args.buffers, s.wanted)?;
    let session = Instant::now();
    let mut seen_xruns = 0;
    let mut seen_changes = 0;
    println!("{USAGE}\n");

    let (keys_tx, keys) = mpsc::channel::<String>();
    let control_thread = std::thread::current();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if keys_tx.send(line).is_err() {
                break;
            }
            control_thread.unpark();
        }
    });

    let mut link = ControllerLink::new();
    let mut link_events = Vec::new();
    // While stopped, LEDs blink on a free-running beat at the current tempo.
    let mut stopped_since = Instant::now();

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
                    if let Some((id, _)) = identify(event)
                        && let Some(led) = led_of(id.control)
                    {
                        link.leds_mut().touched(led);
                    }
                    handle_controller(ts_us, event, &mut s, &mut handle, args.quiet);
                }
            }
        }
        while let Ok(line) = keys.try_recv() {
            if !handle_line(&line, &mut s, &mut handle) {
                finish(&s);
                return Ok(());
            }
        }
        while let Some(event) = handle.next_event() {
            match event {
                EngineEvent::Beat { bar, beat } => {
                    if !args.quiet {
                        let marker = if beat == 1 { "●" } else { "○" };
                        println!("{marker} mesure {bar:>3} · temps {beat}");
                    }
                }
                EngineEvent::Hit { gesture_us, time } => report_hit(&mut s, gesture_us, time),
            }
        }
        let snap = handle.snapshot();
        let beats = if snap.playing {
            stopped_since = Instant::now();
            snap.beats
        } else {
            stopped_since.elapsed().as_secs_f64() * s.wanted.bpm / 60.0
        };
        update_leds(link.leds_mut(), &s, beats);
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
            finish(&s);
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
            if s.latency.totals.len() > s.latency.reported {
                s.latency.reported = s.latency.totals.len();
                if let Some(summary) = s.latency.summary() {
                    println!("  ⏱ latence : {summary}");
                }
            }
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
        // Woken at once by controller messages (and keyboard lines); otherwise one turn per 2 ms
        // keeps LEDs and reports going.
        std::thread::park_timeout(Duration::from_millis(2));
    }
}

/// End-of-session summary.
fn finish(s: &Session) {
    if let Some(summary) = s.latency.summary() {
        println!("⏱ latence geste → son, {summary}");
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
