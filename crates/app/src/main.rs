//! LiveDeck application: UI (eframe/egui) and thread wiring.
//!
//! The main thread runs the window (macOS event loop, `ui`); the control thread owns the
//! `EngineHandle`, maps controller gestures through `config/mapping.toml`, reads keyboard commands,
//! prints the engine state, publishes it to the window and rebuilds the audio stream if the device
//! goes away. `--headless`: no window, the control loop runs on the main thread.

// Design tokens (D5); the widgets that use the rest come with D6.
#[allow(dead_code)]
mod theme;
mod ui;
mod view;
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::thread::sleep;
use std::time::{Duration, Instant};

use assert_no_alloc::assert_no_alloc;
use engine::chords::{Extension, Voicing};
use engine::drums::DrumVoice;
use engine::link::{self, BlockTime, Command, EngineEvent, EngineHandle, EngineSnapshot};
use engine::melody::{ArpMode, ArpRate, ArpSettings};
use engine::repeat::Division;
use engine::sampler::SampleBank;
use engine::{Engine, Param};
use io::audio::{AudioOutput, BlockStamp, start_output};
use io::controller::{ControllerEvent, Deck, Led, PadMode};
use io::leds::{LedBank, LedState};
use io::link::{ControllerLink, LinkEvent};
use io::mapping::{
    Action, ActionName, Control, DEFAULT_MAPPING, Mapping, Target, identify, led_of,
};
use io::system::host_time_us;
use rtrb::{Consumer, RingBuffer};

const USAGE: &str = "\
LiveDeck — deck rythme (séquenceur de batterie) et métronome

USAGE: app [--bpm <60-200>] [--buffer <frames>] [--mapping <fichier>] [--seconds <n>] [--quiet]
           [--no-metronome] [--record <wav>] [--kit <dossier>] [--headless]

  --mapping       fichier de mapping (défaut : config/mapping.toml, sinon le mapping intégré)
  --kit           dossier du kit de samples, kick.wav, snare.wav… (défaut : assets/kits/tr909)
  --headless      sans fenêtre (console seule) ; dans la fenêtre, F = plein écran

  --seconds       s'arrête après n secondes (endurance)
  --quiet         n'affiche pas chaque temps, seulement les bilans et les incidents
  --no-metronome  démarre métronome coupé (le moteur tourne en silence)
  --record <wav>  enregistre la sortie (voie gauche = son, voie droite = beat-repeat engagé),
                  60 s au plus, écrit à la sortie

Commandes (taper puis Entrée) :
  p        lecture / arrêt
  midi <hex…>  rejoue des messages du contrôleur (ex. midi 900B7F 900B00)
  shot <f.bmp> capture de la fenêtre
  + / -    tempo ±1 BPM par signe (+++ = +3)
  t <bpm>  tempo exact         m        métronome on/off
  h        kick (comme le pad Hot Cue 1 du deck A)
  q        quitter

Deck A : pads Hot Cue = patterns A–D (à la mesure suivante), pads Sampler = fills
(Shift : kick, snare, hats), pads FX = mutes kick / snare+clap / hats / perc,
pads Loop tenus = beat-repeat 1/4 … 1/32, fader Tempo = densité, jog bord = swing,
jog dessus = tempo. Basse : chaque pattern amène sa ligne, Shift + Hot Cue = ligne seule,
Cue A = mute, potard Bass A = filtre, Cue B = tonique −1 (Shift : +1), à la mesure.
Shift + PFL A = batterie samples 909 ↔ synthé, à la mesure. Volume A = batterie, Shift + Volume A = basse.
Shift + jog gauche (un demi-tour) = style (kit, patterns, basse, swing), à la mesure ; le tempo reste le tien.
Deck B : pads Hot Cue = accords I, IV, V, vi (tenus), Volume B = mélodie (Shift : métronome),
fader Tempo B = brillance ; chaque style amène son patch de synthé. Shift + Hot Cue B = ii, iii,
vii, I à l'octave ; pads Loop B = latch, renversement +/−, octave +/−, extension (Shift : triade) ;
Shift + jog droit = gamme, à la mesure. Pads FX B = arpège off / montant / aller-retour / aléatoire
(Shift : 1 à 4 octaves), bord du jog B = vitesse 1/4 … 1/32 ; pads Sampler B = phrases du style, à la
mesure (Shift : variation) ; jog B par le dessus = transposition dans la gamme, un degré par quart de tour.
Effets : potard Bass B = delay de la mélodie (Shift : répétitions), Headphone = réverb (stéréo),
Shift + PFL B = temps du delay 1/8 pointé, 1/4, 1/8, 1/4 pointé, 1/16 (chaque style amène le sien).
Chaque son joué au pad affiche sa latence geste → son.";

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
    bass_level: f32,
    melody_level: f32,
    /// Melody sent to the delay, delay feedback (knob 0..=1) and mix sent to the reverb.
    delay_send: f32,
    delay_feedback: f32,
    reverb_send: f32,
    /// Melody synth brightness 0..=1 (0.5 = the patch).
    brightness: f32,
    /// Drum pattern asked for (0-based) and swing (0.5..=0.75).
    pattern: u8,
    swing: f32,
    /// Muted drum voices (bit `i` = `DrumVoice::ALL[i]`) and density 0..=1.
    mutes: u8,
    density: f32,
    /// Bass line (0-based), mute, tonic (0 = C … 11 = B) and filter knob 0..=1.
    bass_line: u8,
    bass_muted: bool,
    tonic: u8,
    bass_cutoff: f32,
    /// Style (see `engine::style`).
    style: u8,
    /// Scale (index of `engine::bass::Scale::ALL`), chord latch and voicing.
    scale: u8,
    latch: bool,
    voicing: Voicing,
    /// Arpeggiator and melody transposition (scale degrees).
    arp: ArpSettings,
    transpose: i8,
    /// Drums from the sample kit rather than the synth.
    samples: bool,
}

/// Control-thread state.
struct Session {
    wanted: Wanted,
    mapping: Mapping,
    /// Last values echoed to the console (continuous controls are echoed by steps only).
    shown_bpm: f64,
    shown_levels: [f32; 8],
    shown_swing: f32,
    shown_density: f32,
    /// Beat-repeat pads held, last pressed last: releasing it falls back to the previous one.
    repeats_held: Vec<Division>,
    leds: LedRoles,
    /// LEDs of pads currently held down (lit while held).
    held: Vec<Led>,
    /// Test hits sent to the engine: (gesture host time, host time the control thread sent it).
    pending_hits: Vec<(u64, u64)>,
    latency: Latency,
    /// Last time a tempo control moved without effect because it waits for its pickup.
    tempo_ignored_at: Option<Instant>,
    /// The window's HUD: the last continuous control touched, and when.
    hud: Option<(ui::Hud, Instant)>,
    /// A sample kit was loaded at start (Shift + PFL A can switch to it).
    kit_loaded: bool,
}

/// How long the tap-tempo LED asks for attention after a tempo control moved without effect.
const PICKUP_HINT: Duration = Duration::from_secs(3);

/// LEDs of the controls bound to actions that have a state to show.
struct LedRoles {
    transport: Vec<Led>,
    metronome: Vec<Led>,
    tap_tempo: Vec<Led>,
    /// Pads launching drum patterns, with the pattern index.
    patterns: Vec<(Led, u8)>,
    /// Fill pads with the fill index, mute pads with their voice mask.
    fills: Vec<(Led, u8)>,
    mutes: Vec<(Led, u8)>,
    bass_mute: Vec<Led>,
    latch: Vec<Led>,
    /// Arpeggio mode pads with the mode index, phrase pads with the phrase index.
    arp_modes: Vec<(Led, u8)>,
    phrases: Vec<(Led, u8)>,
}

impl LedRoles {
    fn of(mapping: &Mapping) -> Self {
        let leds = |action| mapping.bound(action).filter_map(led_of).collect();
        let indexed = |action| {
            mapping
                .indexed(action)
                .filter_map(|(control, v)| led_of(control).map(|led| (led, v)))
                .collect()
        };
        Self {
            transport: leds(ActionName::TransportToggle),
            metronome: leds(ActionName::MetronomeToggle),
            tap_tempo: leds(ActionName::TapTempo),
            patterns: indexed(ActionName::Pattern),
            fills: indexed(ActionName::Fill),
            mutes: indexed(ActionName::Mute),
            bass_mute: leds(ActionName::BassMute),
            latch: leds(ActionName::LatchToggle),
            arp_modes: indexed(ActionName::ArpMode),
            phrases: indexed(ActionName::Phrase),
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
    record: Option<PathBuf>,
    kit: Option<PathBuf>,
    headless: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        bpm: 120.0,
        buffers: vec![128, 256],
        seconds: None,
        quiet: false,
        metronome: true,
        mapping: None,
        kit: None,
        record: None,
        headless: false,
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
            "--record" => args.record = Some(PathBuf::from(value("--record")?)),
            "--kit" => args.kit = Some(PathBuf::from(value("--kit")?)),
            "--headless" => args.headless = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

/// Output recording (`--record`): the audio callback pushes interleaved (sound, beat-repeat
/// engaged) frames into a ring; the control thread drains it into a buffer allocated up front.
struct Recording {
    path: PathBuf,
    sample_rate: u32,
    frames: Vec<f32>,
    ring: Option<Consumer<f32>>,
}

const RECORD_SECONDS: usize = 60;

impl Recording {
    fn drain(&mut self) {
        let Some(ring) = self.ring.as_mut() else {
            return;
        };
        while let Ok(x) = ring.pop() {
            if self.frames.len() < self.frames.capacity() {
                self.frames.push(x);
            }
        }
    }

    /// Writes a 16-bit stereo WAV.
    fn save(&mut self) -> std::io::Result<()> {
        self.drain();
        let data_len = u32::try_from(self.frames.len() * 2).unwrap_or(u32::MAX);
        let sr = self.sample_rate;
        let mut out = Vec::with_capacity(44 + self.frames.len() * 2);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&sr.to_le_bytes());
        out.extend_from_slice(&(sr * 4).to_le_bytes());
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.frames {
            out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32_767.0) as i16).to_le_bytes());
        }
        std::fs::write(&self.path, out)
    }
}

/// Builds the engine at the device rate, starts the stream and replays the wanted state.
fn open(
    buffers: &[u32],
    wanted: Wanted,
    kit: Option<&SampleBank>,
    recording: Option<&mut Recording>,
) -> Result<(AudioOutput, EngineHandle), String> {
    let mut handle = None;
    let record = recording.is_some();
    let mut ring = None;
    let mut rate = 0;
    let output = start_output(buffers, |info| {
        let mut engine = Engine::new(info.sample_rate, 4096, wanted.bpm);
        if let Some(kit) = kit {
            engine.set_sample_bank(kit.clone());
        }
        let (mut audio, control) = link::connect(engine);
        handle = Some(control);
        rate = info.sample_rate;
        // One second of margin: the control thread drains it every few ms.
        let mut tap = record.then(|| {
            let (tx, rx) = RingBuffer::<f32>::new(info.sample_rate as usize * 2);
            ring = Some(rx);
            tx
        });
        move |data: &mut [f32], channels: usize, stamp: BlockStamp| {
            let time = BlockTime {
                called_us: stamp.called_us,
                callback_us: stamp.callback_us,
                playback_us: stamp.playback_us,
            };
            assert_no_alloc(|| {
                audio.process_at(data, channels, time);
                if let Some(tap) = tap.as_mut() {
                    let marker = if audio.engine().beat_repeat_engaged() {
                        0.5
                    } else {
                        0.0
                    };
                    for frame in data.chunks_exact(channels.max(1)) {
                        let _ = tap.push(frame.first().copied().unwrap_or(0.0));
                        let _ = tap.push(marker);
                    }
                }
            });
        }
    })
    .map_err(|e| e.to_string())?;
    if let Some(r) = recording {
        r.ring = ring;
        r.sample_rate = rate;
    }
    let mut handle = handle.ok_or("le moteur n'a pas été construit")?;
    handle.send(Command::Metronome(wanted.metronome));
    handle.send(Command::SetParam(Param::MasterGain, wanted.master_gain));
    handle.send(Command::SetParam(
        Param::MetronomeLevel,
        wanted.metronome_level,
    ));
    handle.send(Command::SetParam(Param::DrumLevel, wanted.drum_level));
    handle.send(Command::SetParam(Param::BassLevel, wanted.bass_level));
    handle.send(Command::SetParam(Param::MelodyLevel, wanted.melody_level));
    handle.send(Command::SetParam(Param::Brightness, wanted.brightness));
    handle.send(Command::SetParam(Param::DelaySend, wanted.delay_send));
    handle.send(Command::SetParam(
        Param::DelayFeedback,
        wanted.delay_feedback,
    ));
    handle.send(Command::SetParam(Param::ReverbSend, wanted.reverb_send));
    // The style first: it sets the swing, which may have been nudged since.
    handle.send(Command::QueueStyle(wanted.style));
    handle.send(Command::SetScale(wanted.scale));
    handle.send(Command::Latch(wanted.latch));
    handle.send(Command::Voicing {
        inversion: wanted.voicing.inversion,
        octave: wanted.voicing.octave,
        extension: wanted.voicing.extension.index(),
    });
    send_arp(&mut handle, wanted.arp);
    handle.send(Command::Transpose(wanted.transpose));
    handle.send(Command::UseSamples(wanted.samples));
    handle.send(Command::QueuePattern(wanted.pattern));
    handle.send(Command::SetSwing(wanted.swing));
    handle.send(Command::SetMutes(wanted.mutes));
    handle.send(Command::SetDensity(wanted.density));
    handle.send(Command::QueueBassLine(wanted.bass_line));
    handle.send(Command::BassMute(wanted.bass_muted));
    handle.send(Command::SetTonic(wanted.tonic));
    handle.send(Command::SetParam(Param::BassCutoff, wanted.bass_cutoff));
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
            if let Some(hw) = s.mapping.pending_pickup(Target::Tempo) {
                let hud = ui::Hud {
                    zone: ui::Zone::Center,
                    label: "Fader tempo",
                    value: ui::HudValue::Percent(hw),
                    pickup: Some(ui::HudValue::Bpm(s.wanted.bpm)),
                };
                s.hud = Some((hud, Instant::now()));
            }
        }
        return;
    };
    let hud = hud_of(action, s);
    if let Some(hud) = hud {
        s.hud = Some((hud, Instant::now()));
    }
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
        Action::QueuePattern(index) => {
            s.wanted.pattern = index;
            s.wanted.bass_line = index;
            handle.send(Command::QueuePattern(index));
        }
        Action::QueueBassLine(index) => {
            s.wanted.bass_line = index;
            handle.send(Command::QueueBassLine(index));
            println!("  ♪ ligne de basse {} (à la mesure)", pattern_name(index));
        }
        Action::ToggleBassMute => {
            s.wanted.bass_muted = !s.wanted.bass_muted;
            handle.send(Command::BassMute(s.wanted.bass_muted));
            println!("  {} basse", if s.wanted.bass_muted { "🔇" } else { "🔈" });
        }
        Action::NudgeTonic(delta) => {
            s.wanted.tonic = (i16::from(s.wanted.tonic) + i16::from(delta)).rem_euclid(12) as u8;
            handle.send(Command::SetTonic(s.wanted.tonic));
            println!(
                "  ♮ tonalité {} {} (à la mesure)",
                engine::bass::tonic_name(s.wanted.tonic),
                scale_name(s.wanted.scale)
            );
        }
        Action::Chord { pad, held } => {
            handle.send(Command::Chord { pad, on: held });
            if held {
                const NAMES: [&str; 8] = ["I", "IV", "V", "vi", "ii", "iii", "vii", "I'"];
                println!("  ♫ accord {}", NAMES.get(usize::from(pad)).unwrap_or(&"?"));
            }
        }
        Action::ToggleLatch => {
            s.wanted.latch = !s.wanted.latch;
            handle.send(Command::Latch(s.wanted.latch));
            println!("  ⚓ latch {}", if s.wanted.latch { "on" } else { "off" });
        }
        Action::NudgeInversion(delta) => {
            s.wanted.voicing.inversion = (s.wanted.voicing.inversion + delta).clamp(
                -engine::chords::MAX_INVERSION,
                engine::chords::MAX_INVERSION,
            );
            send_voicing(s, handle);
        }
        Action::NudgeChordOctave(delta) => {
            s.wanted.voicing.octave = (s.wanted.voicing.octave + delta)
                .clamp(-engine::chords::MAX_OCTAVE, engine::chords::MAX_OCTAVE);
            send_voicing(s, handle);
        }
        Action::Extension(step) => {
            s.wanted.voicing.extension = if step == 0 {
                Extension::Triad
            } else {
                Extension::from_index(s.wanted.voicing.extension.index() + 1)
            };
            send_voicing(s, handle);
        }
        Action::NudgeScale(delta) => {
            let last = engine::bass::Scale::ALL.len() as i16 - 1;
            let scale = (i16::from(s.wanted.scale) + i16::from(delta)).clamp(0, last) as u8;
            if scale != s.wanted.scale {
                s.wanted.scale = scale;
                handle.send(Command::SetScale(scale));
            }
        }
        Action::ArpMode(index) => {
            s.wanted.arp.mode = ArpMode::from_index(index);
            send_arp(handle, s.wanted.arp);
            echo_arp(s.wanted.arp);
        }
        Action::ArpOctaves(octaves) => {
            s.wanted.arp.octaves = octaves;
            send_arp(handle, s.wanted.arp);
            echo_arp(s.wanted.arp);
        }
        Action::NudgeArpRate(delta) => {
            let last = ArpRate::ALL.len() as i16 - 1;
            let index = s.wanted.arp.rate.index();
            let rate = (i16::from(index) + i16::from(delta)).clamp(0, last) as u8;
            if rate != index {
                s.wanted.arp.rate = ArpRate::from_index(rate);
                send_arp(handle, s.wanted.arp);
                echo_arp(s.wanted.arp);
            }
        }
        Action::CycleDelayTime => {
            handle.send(Command::CycleDelayTime);
        }
        Action::Phrase { index, variation } => {
            handle.send(Command::Phrase { index, variation });
        }
        Action::NudgeTranspose(delta) => {
            let max = engine::chords::MAX_TRANSPOSE;
            let transpose = (s.wanted.transpose + delta).clamp(-max, max);
            if transpose != s.wanted.transpose {
                s.wanted.transpose = transpose;
                handle.send(Command::Transpose(transpose));
                println!("  ⇅ transposition {transpose:+} degré(s)");
            }
        }
        Action::SetBrightness(value) => {
            s.wanted.brightness = value;
            handle.send(Command::SetParam(Param::Brightness, value));
        }
        Action::ToggleSamples => {
            if s.kit_loaded {
                s.wanted.samples = !s.wanted.samples;
                handle.send(Command::UseSamples(s.wanted.samples));
            } else {
                println!("  ⚠ pas de kit de samples chargé (voir --kit)");
            }
        }
        Action::NudgeStyle(delta) => {
            let last = engine::style::STYLES as i16 - 1;
            let style = (i16::from(s.wanted.style) + i16::from(delta)).clamp(0, last) as u8;
            if style != s.wanted.style {
                s.wanted.style = style;
                handle.send(Command::QueueStyle(style));
            }
        }
        Action::SetBassCutoff(value) => {
            s.wanted.bass_cutoff = value;
            handle.send(Command::SetParam(Param::BassCutoff, value));
        }
        Action::NudgeSwing(delta) => {
            s.wanted.swing = (s.wanted.swing + delta)
                .clamp(engine::sequencer::MIN_SWING, engine::sequencer::MAX_SWING);
            handle.send(Command::SetSwing(s.wanted.swing));
            if (s.wanted.swing - s.shown_swing).abs() >= 0.01 {
                s.shown_swing = s.wanted.swing;
                println!("  ≈ swing {:.0} %", s.wanted.swing * 100.0);
            }
        }
        Action::ToggleMute(mask) => {
            // A partly muted group is unmuted as a whole.
            let all_muted = s.wanted.mutes & mask == mask;
            if all_muted {
                s.wanted.mutes &= !mask;
            } else {
                s.wanted.mutes |= mask;
            }
            handle.send(Command::SetMutes(s.wanted.mutes));
            println!(
                "  {} {}",
                if all_muted { "🔈" } else { "🔇" },
                voice_names(mask)
            );
        }
        Action::Fill(index) => {
            handle.send(Command::Fill(index));
            println!(
                "  ↯ fill {} (de la prochaine noire à la fin de la mesure)",
                index + 1
            );
        }
        Action::BeatRepeat { division, held } => {
            let was = s.repeats_held.last().copied();
            s.repeats_held.retain(|&d| d != division);
            if held {
                s.repeats_held.push(division);
                handle.send(Command::BeatRepeat(Some(division)));
                println!("  ⟳ beat-repeat 1/{}", division.denominator());
            } else if let Some(&back) = s.repeats_held.last() {
                // Back to the pad still held (its own loop, in phase); releasing a pad that was
                // not the one heard only forgets its loop.
                handle.send(Command::BeatRepeatBack {
                    released: division,
                    back,
                });
                if was == Some(division) {
                    println!("  ⟳ beat-repeat 1/{} (retour)", back.denominator());
                }
            } else {
                handle.send(Command::BeatRepeat(None));
                println!("  ⟳ beat-repeat relâché");
            }
        }
        Action::SetDensity(value) => {
            s.wanted.density = value;
            handle.send(Command::SetDensity(value));
            // By 10 % steps, plus the ends of the travel.
            let at_end = (value == 0.0 || value == 1.0) && value != s.shown_density;
            if (value - s.shown_density).abs() >= 0.1 || at_end {
                s.shown_density = value;
                println!("  ▥ densité {:.0} %", value * 100.0);
            }
        }
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
                Target::BassLevel => {
                    s.wanted.bass_level = value;
                    (Param::BassLevel, 3, "basse")
                }
                Target::MelodyLevel => {
                    s.wanted.melody_level = value;
                    (Param::MelodyLevel, 4, "mélodie")
                }
                Target::DelaySend => {
                    s.wanted.delay_send = value;
                    (Param::DelaySend, 5, "envoi delay")
                }
                Target::DelayFeedback => {
                    s.wanted.delay_feedback = value;
                    (Param::DelayFeedback, 6, "répétitions du delay")
                }
                Target::ReverbSend => {
                    s.wanted.reverb_send = value;
                    (Param::ReverbSend, 7, "envoi réverb")
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

/// "snare + clap" for a voice mask.
fn voice_names(mask: u8) -> String {
    DrumVoice::ALL
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, v)| v.name())
        .collect::<Vec<_>>()
        .join(" + ")
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
fn update_leds(leds: &mut LedBank, s: &Session, snap: &EngineSnapshot, beats: f64) {
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
    for &(led, index) in &s.leds.patterns {
        // Playing pattern lit, the one waiting for the bar blinks in tempo.
        let state = if snap.pending_pattern == Some(index) {
            LedState::Blink
        } else if snap.pattern == index {
            LedState::On
        } else {
            LedState::Off
        };
        leds.set_state(led, state);
    }
    for &(led, index) in &s.leds.fills {
        // Waiting for the next beat: blinks fast; playing: lit.
        let state = match snap.fill {
            Some((f, true)) if f == index => LedState::On,
            Some((f, false)) if f == index => LedState::BlinkFast,
            _ => LedState::Off,
        };
        leds.set_state(led, state);
    }
    for &(led, mask) in &s.leds.mutes {
        // Lit = the group plays (like a channel on a console).
        leds.set(led, snap.mutes & mask != mask);
    }
    for &led in &s.leds.latch {
        leds.set(led, s.wanted.latch);
    }
    for &(led, mode) in &s.leds.arp_modes {
        leds.set(led, snap.arp_mode == mode);
    }
    for &(led, index) in &s.leds.phrases {
        // Playing lit, waiting for the bar (start or stop) blinks.
        let playing = snap.phrase.is_some_and(|(p, _)| p == index);
        let state = match snap.pending_phrase {
            Some(next) if next.is_some_and(|(p, _)| p == index) || (next.is_none() && playing) => {
                LedState::Blink
            }
            _ if playing => LedState::On,
            _ => LedState::Off,
        };
        leds.set_state(led, state);
    }
    for &led in &s.leds.bass_mute {
        // Lit = the bass plays, like the mute pads (replaces the beat display on that button).
        leds.set_state(
            led,
            if snap.bass_muted {
                LedState::Off
            } else {
                LedState::On
            },
        );
    }
    for &led in &s.held {
        leds.set(led, true);
    }
}

/// Sends the chord voicing and echoes it.
fn send_voicing(s: &Session, handle: &mut EngineHandle) {
    let v = s.wanted.voicing;
    handle.send(Command::Voicing {
        inversion: v.inversion,
        octave: v.octave,
        extension: v.extension.index(),
    });
    println!(
        "  ♯ voicing : renversement {:+}, octave {:+}, {}",
        v.inversion,
        v.octave,
        v.extension.name()
    );
}

fn send_arp(handle: &mut EngineHandle, arp: ArpSettings) {
    handle.send(Command::Arp {
        mode: arp.mode.index(),
        rate: arp.rate.index(),
        octaves: arp.octaves,
    });
}

fn echo_arp(arp: ArpSettings) {
    if arp.mode == ArpMode::Off {
        println!("  ⁘ arpège off");
    } else {
        println!(
            "  ⁘ arpège {}, {}, {} octave(s)",
            arp.mode.name(),
            arp.rate.name(),
            arp.octaves
        );
    }
}

/// "phrase 2" or "phrase 2 (variation)".
fn phrase_label((index, variation): (u8, bool)) -> String {
    let suffix = if variation { " (variation)" } else { "" };
    format!("phrase {}{suffix}", index + 1)
}

fn scale_name(index: u8) -> &'static str {
    engine::bass::Scale::from_index(index).map_or("?", engine::bass::Scale::name)
}

/// "A" … "D".
fn pattern_name(index: u8) -> char {
    char::from(b'A' + index.min(25))
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

/// Loads the sample kit: explicit folder, else `assets/kits/tr909` if present. A missing or broken
/// kit is reported and leaves the drums on the synth.
fn load_kit(path: Option<&PathBuf>) -> Option<SampleBank> {
    let dir = path
        .cloned()
        .unwrap_or_else(|| PathBuf::from(io::samples::DEFAULT_KIT));
    if path.is_none() && !dir.exists() {
        return None;
    }
    match io::samples::load_kit(&dir) {
        Ok(bank) => {
            let count = bank.voices.iter().filter(|v| !v.is_empty()).count();
            println!(
                "Kit : {} ({count} samples à {} Hz)",
                dir.display(),
                bank.sample_rate
            );
            Some(bank)
        }
        Err(e) => {
            println!("⚠ kit non chargé : {e}");
            None
        }
    }
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

/// The control thread's side of the window.
struct WindowLink {
    shared: Arc<ui::Shared>,
    ctx: egui::Context,
}

/// The control loop; with a window, closing it ends the session, and the end of the session
/// closes it.
fn run(args: &Args, window: Option<&WindowLink>) -> Result<(), String> {
    let result = control(args, window);
    if let Some(w) = window {
        w.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    result
}

fn control(args: &Args, window: Option<&WindowLink>) -> Result<(), String> {
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
            bass_level: 0.8,
            melody_level: 0.8,
            delay_send: 0.0,
            delay_feedback: 0.5,
            reverb_send: 0.0,
            brightness: 0.5,
            pattern: 0,
            swing: 0.54,
            mutes: 0,
            density: 1.0,
            bass_line: 0,
            bass_muted: false,
            tonic: 9,
            bass_cutoff: 0.45,
            style: 0,
            scale: 0,
            latch: false,
            voicing: Voicing::default(),
            arp: ArpSettings::default(),
            transpose: 0,
            samples: false,
        },
        leds: LedRoles::of(&mapping),
        mapping,
        shown_bpm: 0.0,
        shown_levels: [-1.0; 8],
        shown_swing: 0.54,
        shown_density: 1.0,
        repeats_held: Vec::new(),
        held: Vec::new(),
        pending_hits: Vec::new(),
        latency: Latency::default(),
        tempo_ignored_at: None,
        hud: None,
        kit_loaded: false,
    };
    let kit = load_kit(args.kit.as_ref());
    s.kit_loaded = kit.is_some();
    println!("Mapping : {mapping_source} ({} liaisons)", s.mapping.len());
    s.mapping.software_changed(Target::Tempo, s.wanted.bpm);
    s.mapping
        .software_changed(Target::MasterGain, f64::from(s.wanted.master_gain));
    s.mapping
        .software_changed(Target::MetronomeLevel, f64::from(s.wanted.metronome_level));
    s.mapping
        .software_changed(Target::DrumLevel, f64::from(s.wanted.drum_level));
    s.mapping
        .software_changed(Target::BassLevel, f64::from(s.wanted.bass_level));
    s.mapping
        .software_changed(Target::MelodyLevel, f64::from(s.wanted.melody_level));
    s.mapping
        .software_changed(Target::DelaySend, f64::from(s.wanted.delay_send));
    s.mapping
        .software_changed(Target::DelayFeedback, f64::from(s.wanted.delay_feedback));
    let mut recording = args.record.clone().map(|path| Recording {
        path,
        sample_rate: 48_000,
        frames: Vec::with_capacity(48_000 * 2 * RECORD_SECONDS),
        ring: None,
    });
    let (mut output, mut handle) = open(&args.buffers, s.wanted, kit.as_ref(), recording.as_mut())?;
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
    let mut replay = io::controller::Decoder::new();
    let mut link_events = Vec::new();
    // While stopped, LEDs blink on a free-running beat at the current tempo.
    let mut stopped_since = Instant::now();
    let mut shown_pattern = (u8::MAX, None);
    let mut shown_style = (u8::MAX, None);
    let mut shown_samples = (false, None);
    let mut shown_scale = (u8::MAX, None);
    let mut shown_phrase = (None, None);
    let mut shown_delay = u8::MAX;

    let mut last_report = Instant::now();
    let mut last_xrun: Option<Instant> = None;
    // The window draws the patterns and bass lines of the style heard.
    let presets = engine::style::presets();
    let mut meters = view::Meters::new();
    let mut shown_load = 0.0;
    let mut frames_seen = 0;
    loop {
        if window.is_some_and(|w| w.shared.quit_requested()) {
            finish(&s, recording.as_mut());
            return Ok(());
        }
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
            // Development commands: gestures replayed from raw MIDI, window captures.
            if let Some(hex) = line.trim().strip_prefix("midi ") {
                for event in hex
                    .split_whitespace()
                    .filter_map(|h| replay.decode(&parse_hex(h)))
                {
                    if let ControllerEvent::PadModeSelect { deck, mode, .. }
                    | ControllerEvent::Pad { deck, mode, .. } = event
                    {
                        link.leds_mut().mode_shown(deck, mode);
                    }
                    handle_controller(host_time_us(), event, &mut s, &mut handle, args.quiet);
                }
                continue;
            }
            if let Some(path) = line.trim().strip_prefix("shot ") {
                match window {
                    Some(w) => ui::capture(&w.ctx, PathBuf::from(path)),
                    None => println!("shot : pas de fenêtre (--headless)"),
                }
                continue;
            }
            if !handle_line(&line, &mut s, &mut handle) {
                finish(&s, recording.as_mut());
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
        update_leds(link.leds_mut(), &s, &snap, beats);
        if let Some(r) = recording.as_mut() {
            r.drain();
        }
        if (snap.scale, snap.pending_scale) != shown_scale {
            shown_scale = (snap.scale, snap.pending_scale);
            match snap.pending_scale {
                Some(next) => println!("  ◇ gamme {} à la mesure suivante", scale_name(next)),
                None => println!("  ◆ gamme {}", scale_name(snap.scale)),
            }
        }
        if (snap.phrase, snap.pending_phrase) != shown_phrase {
            shown_phrase = (snap.phrase, snap.pending_phrase);
            match (snap.pending_phrase, snap.phrase) {
                (Some(Some(next)), _) => {
                    println!("  ◇ {} à la mesure suivante", phrase_label(next));
                }
                (Some(None), _) => println!("  ◇ phrase arrêtée à la mesure suivante"),
                (None, Some(now)) => println!("  ◆ {}", phrase_label(now)),
                (None, None) => println!("  ◆ pas de phrase"),
            }
        }
        if snap.delay_time != shown_delay {
            // Skip the first value: the starting delay time is not news.
            if shown_delay != u8::MAX {
                println!(
                    "  ⌛ delay {}",
                    engine::fx::DelayTime::from_index(snap.delay_time).name()
                );
            }
            shown_delay = snap.delay_time;
        }
        if (snap.samples, snap.pending_samples) != shown_samples {
            shown_samples = (snap.samples, snap.pending_samples);
            let source = |on: bool| if on { "samples 909" } else { "synthé" };
            match snap.pending_samples {
                Some(next) => println!("  ◇ batterie {} à la mesure suivante", source(next)),
                None => println!("  🥁 batterie {}", source(snap.samples)),
            }
        }
        if (snap.style, snap.pending_style) != shown_style {
            let applied = snap.style != shown_style.0;
            shown_style = (snap.style, snap.pending_style);
            let label = |i: u8| {
                format!(
                    "{} ({:.0} BPM conseillé)",
                    engine::style::name(i),
                    engine::style::bpm(i)
                )
            };
            match snap.pending_style {
                Some(next) => println!("  ◇ style {} à la mesure suivante", label(next)),
                None => println!("  ◆ style {}", label(snap.style)),
            }
            if applied {
                // The style brought its swing: the jog edge nudges from there.
                s.wanted.swing = snap.swing;
                s.shown_swing = snap.swing;
                println!("  ≈ swing {:.0} %", snap.swing * 100.0);
            }
        }
        if (snap.pattern, snap.pending_pattern) != shown_pattern {
            shown_pattern = (snap.pattern, snap.pending_pattern);
            let name = |p: u8| char::from(b'A' + p);
            match snap.pending_pattern {
                Some(next) => println!(
                    "  ⏭ pattern {} → {} à la mesure suivante",
                    name(snap.pattern),
                    name(next)
                ),
                None => println!("  ▣ pattern {}", name(snap.pattern)),
            }
        }
        let xruns = output.stats.xruns.load(Ordering::Relaxed);
        if xruns != seen_xruns {
            seen_xruns = xruns;
            last_xrun = Some(Instant::now());
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
            finish(&s, recording.as_mut());
            return Ok(());
        }
        if output.stats.invalidated.load(Ordering::Relaxed) {
            println!("⚠ sortie audio perdue, reconstruction…");
            drop(output);
            loop {
                sleep(Duration::from_millis(500));
                match open(&args.buffers, s.wanted, kit.as_ref(), recording.as_mut()) {
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
            shown_load = output.stats.take_max_load(output.info.sample_rate);
            if let Some(w) = window {
                let frames = w.shared.frames();
                println!(
                    "  🖥 fenêtre : {:.1} images/s",
                    (frames - frames_seen) as f64 / 5.0
                );
                frames_seen = frames;
            }
            println!(
                "  [{:.0} BPM │ charge max {:.1} % │ xruns {} │ NaN {} │ commandes {} │ contrôleur {}]",
                snap.bpm,
                shown_load * 100.0,
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
        if let Some(w) = window {
            let view = ui::View {
                playing: snap.playing,
                bpm: snap.bpm,
                beats: snap.beats,
                tonic: snap.tonic,
                scale: snap.scale,
                pending_key: (snap.pending_tonic.is_some() || snap.pending_scale.is_some()).then(
                    || {
                        (
                            snap.pending_tonic.unwrap_or(snap.tonic),
                            snap.pending_scale.unwrap_or(snap.scale),
                        )
                    },
                ),
                style: snap.style,
                pending_style: snap.pending_style,
                swing: snap.swing,
                controller: link.is_connected(),
                load: shown_load as f32,
                xrun_recent: last_xrun.is_some_and(|t| t.elapsed() < Duration::from_secs(10)),
                step: snap.step,
                pattern: snap.pattern,
                pending_pattern: snap.pending_pattern,
                grid: presets
                    .get(usize::from(snap.style))
                    .and_then(|style| style.patterns.get(usize::from(snap.pattern)))
                    .map(view::drum_grid)
                    .unwrap_or_default(),
                mutes: snap.mutes,
                density: snap.density,
                fill: snap.fill,
                beat_repeat: snap.beat_repeat,
                bass_line: snap.bass_line,
                pending_bass_line: snap.pending_bass_line,
                bass_muted: snap.bass_muted,
                bass: presets
                    .get(usize::from(snap.style))
                    .and_then(|style| style.bass_lines.get(usize::from(snap.bass_line)))
                    .map(view::bass_cells)
                    .unwrap_or_default(),
                bass_cutoff: s.wanted.bass_cutoff,
                pads: view::pad_lights(link.leds()),
                shown_mode: view::shown_modes(link.leds()),
                chords: snap.chords,
                transpose: s.wanted.transpose,
                octave: s.wanted.voicing.octave,
                inversion: s.wanted.voicing.inversion,
                extension: s.wanted.voicing.extension.index(),
                latch: s.wanted.latch,
                arp_mode: snap.arp_mode,
                arp_rate: snap.arp_rate,
                arp_octaves: snap.arp_octaves,
                phrase: snap.phrase,
                pending_phrase: snap.pending_phrase,
                delay_time: snap.delay_time,
                hud: shown_hud(&s),
                meters: meters.update(snap.peaks),
                limiting: snap.limiter_gain < 0.98,
                overload: shown_load > 0.7,
            };
            // Playing, the window repaints at its own pace; otherwise on a change, at most at its
            // playing rate (a falling meter changes on every turn of this loop).
            if w.shared.publish(view) {
                w.ctx.request_repaint_after(ui::FRAME);
            }
        }
        // Woken at once by controller messages (and keyboard lines); otherwise one turn per 2 ms
        // keeps LEDs and reports going.
        std::thread::park_timeout(Duration::from_millis(2));
    }
}

/// The HUD a continuous gesture shows (read before the action is applied: levels carry their own
/// value, nudged settings are completed by `hud_value`).
fn hud_of(action: Action, s: &Session) -> Option<ui::Hud> {
    use ui::{HudValue::Percent, Zone};
    let (zone, label, value) = match action {
        Action::SetLevel(target, v) => {
            let (zone, label) = match target {
                Target::MasterGain => (Zone::Center, "Master"),
                Target::MetronomeLevel => (Zone::B, "Métronome"),
                Target::DrumLevel => (Zone::A, "Batterie"),
                Target::BassLevel => (Zone::A, "Basse"),
                Target::MelodyLevel => (Zone::B, "Mélodie"),
                Target::DelaySend => (Zone::B, "Envoi delay"),
                Target::DelayFeedback => (Zone::B, "Répétitions"),
                Target::ReverbSend => (Zone::Center, "Réverb"),
                Target::Tempo => return None,
            };
            (zone, label, Percent(v))
        }
        Action::SetTempo(bpm) => (Zone::Center, "Tempo", ui::HudValue::Bpm(bpm)),
        Action::NudgeTempo(delta) => (
            Zone::Center,
            "Tempo",
            ui::HudValue::Bpm(s.wanted.bpm + delta),
        ),
        Action::SetDensity(v) => (Zone::A, "Densité", Percent(v)),
        Action::SetBassCutoff(v) => (Zone::A, "Filtre basse", Percent(v)),
        Action::SetBrightness(v) => (Zone::B, "Brillance", Percent(v)),
        Action::NudgeSwing(delta) => (
            Zone::A,
            "Swing",
            Percent(
                (s.wanted.swing + delta)
                    .clamp(engine::sequencer::MIN_SWING, engine::sequencer::MAX_SWING),
            ),
        ),
        _ => return None,
    };
    Some(ui::Hud {
        zone,
        label,
        value,
        pickup: None,
    })
}

/// The HUD while it is shown: ~1 s after the last movement, longer while waiting for pickup.
fn shown_hud(s: &Session) -> Option<ui::Hud> {
    let (hud, at) = s.hud?;
    let ms = if hud.pickup.is_some() {
        theme::PICKUP_HUD_MS
    } else {
        theme::HUD_MS
    };
    (at.elapsed() < Duration::from_millis(ms)).then_some(hud)
}

/// "97397F" → [0x97, 0x39, 0x7F].
fn parse_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .filter_map(|pair| std::str::from_utf8(pair).ok())
        .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
        .collect()
}

/// End-of-session summary; writes the recording if any.
fn finish(s: &Session, recording: Option<&mut Recording>) {
    if let Some(r) = recording {
        match r.save() {
            Ok(()) => println!(
                "⏺ enregistrement : {} ({:.1} s)",
                r.path.display(),
                r.frames.len() as f64 / 2.0 / f64::from(r.sample_rate)
            ),
            Err(e) => println!("⚠ enregistrement non écrit : {e}"),
        }
    }
    if let Some(summary) = s.latency.summary() {
        println!("⏱ latence geste → son, {summary}");
    }
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => fail(&e),
    };
    if args.headless {
        if let Err(e) = run(&args, None) {
            fail(&e);
        }
        return;
    }
    // Before the control thread touches MIDI: the window's event loop keeps CoreMIDI's port list
    // current (hot-plug).
    let _midi = io::link::bind_midi_to_this_thread();
    let shared = Arc::new(ui::Shared::default());
    let mut control_thread = None;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LiveDeck")
            .with_inner_size(theme::MIN_WINDOW)
            .with_min_inner_size(theme::MIN_WINDOW),
        ..eframe::NativeOptions::default()
    };
    let result = eframe::run_native(
        "LiveDeck",
        options,
        Box::new(|cc| {
            let link = WindowLink {
                shared: Arc::clone(&shared),
                ctx: cc.egui_ctx.clone(),
            };
            let spawned = std::thread::Builder::new()
                .name("control".into())
                .spawn(move || {
                    if let Err(e) = run(&args, Some(&link)) {
                        fail(&e);
                    }
                })
                .map_err(|e| format!("thread de contrôle : {e}"))?;
            control_thread = Some(spawned);
            Ok(Box::new(ui::Window::new(&cc.egui_ctx, Arc::clone(&shared))))
        }),
    );
    // The window is gone (closed, or the session ended): let the control thread finish (recording,
    // latency summary).
    shared.request_quit();
    if let Some(thread) = control_thread {
        let _ = thread.join();
    }
    if let Err(e) = result {
        fail(&format!("fenêtre : {e}"));
    }
}

fn fail(message: &str) -> ! {
    eprintln!("erreur : {message}");
    std::process::exit(1);
}
