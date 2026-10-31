//! MIDI sniffer for the DJControl Mix: shows every incoming message decoded, and can send raw
//! messages (LEDs, dump request). Used by the `/midi-sniff` skill to verify hardware facts.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use io::controller::{ALL_LEDS_OFF, ControllerEvent, Decoder, REQUEST_DUMP};
use midir::{MidiInput, MidiOutput, MidiOutputConnection};

const USAGE: &str = "\
midi-sniff — affiche et décode les messages du DJControl Mix

USAGE: midi-sniff [OPTIONS]

  --list               liste les ports MIDI et quitte
  --port <texte>       port à ouvrir (sous-chaîne, défaut : djcontrol)
  --json               une ligne JSON par message (pour analyse)
  --seconds <n>        s'arrête après n secondes et affiche un résumé
  --dump               envoie B0 7F 7F (le contrôleur renvoie la position des potards)
  --send \"<hex>\"       envoie un message brut, ex. --send \"91 07 7F\" (répétable)
  --leds \"<st> <note>\" balaye des valeurs de LED sur une cible, ex. --leds \"96 00\"
  --off                éteint toutes les LEDs en quittant (B0 7F 00)
  -h, --help           cette aide";

#[derive(Default)]
struct Args {
    list: bool,
    port: Option<String>,
    json: bool,
    seconds: Option<f64>,
    dump: bool,
    send: Vec<Vec<u8>>,
    leds: Option<[u8; 2]>,
    off: bool,
}

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    s.split_whitespace()
        .map(|b| {
            u8::from_str_radix(b.trim_start_matches("0x"), 16)
                .map_err(|_| format!("octet hexadécimal invalide : {b}"))
        })
        .collect()
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} attend une valeur"));
        match arg.as_str() {
            "--list" => args.list = true,
            "--json" => args.json = true,
            "--dump" => args.dump = true,
            "--off" => args.off = true,
            "--port" => args.port = Some(value("--port")?),
            "--seconds" => {
                let v = value("--seconds")?;
                args.seconds = Some(v.parse().map_err(|_| format!("durée invalide : {v}"))?);
            }
            "--send" => args.send.push(parse_hex(&value("--send")?)?),
            "--leds" => match parse_hex(&value("--leds")?)?.as_slice() {
                &[status, note] => args.leds = Some([status, note]),
                _ => return Err("--leds attend deux octets, ex. \"96 00\"".into()),
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Variant name of an event, used for JSON and the summary.
fn kind(event: &ControllerEvent) -> &'static str {
    match event {
        ControllerEvent::Shift { .. } => "Shift",
        ControllerEvent::Vinyl { .. } => "Vinyl",
        ControllerEvent::FilterBassToggle { .. } => "FilterBass",
        ControllerEvent::PadModeSelect { .. } => "PadMode",
        ControllerEvent::Button { .. } => "Button",
        ControllerEvent::Pad { .. } => "Pad",
        ControllerEvent::Mixer { .. } => "Mixer",
        ControllerEvent::Knob { .. } => "Knob",
        ControllerEvent::Tempo { .. } => "Tempo",
        ControllerEvent::Jog { .. } => "Jog",
        ControllerEvent::Unknown { .. } => "Unknown",
    }
}

#[derive(Default)]
struct Stats {
    first_us: Option<u64>,
    last_us: u64,
    per_kind: BTreeMap<&'static str, usize>,
    /// Jog messages per 250 ms bucket, to estimate the maximum rate.
    jog_buckets: BTreeMap<u64, usize>,
    unknown: BTreeMap<[u8; 3], usize>,
}

struct Sniffer {
    decoder: Decoder,
    json: bool,
    stats: Arc<Mutex<Stats>>,
}

impl Sniffer {
    fn on_message(&mut self, ts_us: u64, bytes: &[u8]) {
        let Some(event) = self.decoder.decode(bytes) else {
            return;
        };
        let Ok(mut stats) = self.stats.lock() else {
            return;
        };
        let first = *stats.first_us.get_or_insert(ts_us);
        let t_ms = ts_us.saturating_sub(first) as f64 / 1000.0;
        let dt_ms = ts_us.saturating_sub(stats.last_us.max(first)) as f64 / 1000.0;
        stats.last_us = ts_us;
        *stats.per_kind.entry(kind(&event)).or_default() += 1;
        if let ControllerEvent::Jog { .. } = event {
            *stats.jog_buckets.entry(ts_us / 250_000).or_default() += 1;
        }
        if let ControllerEvent::Unknown {
            status,
            data1,
            data2,
        } = event
        {
            *stats.unknown.entry([status, data1, data2]).or_default() += 1;
        }
        drop(stats);
        if self.json {
            println!(
                "{{\"t_ms\":{t_ms:.3},\"dt_ms\":{dt_ms:.3},\"hex\":\"{}\",\"kind\":\"{}\",\"event\":\"{event}\"}}",
                hex(bytes),
                kind(&event)
            );
        } else {
            println!("{t_ms:>10.1} ms │ {:<8} │ {event}", hex(bytes));
        }
    }
}

fn send(out: &mut Option<MidiOutputConnection>, bytes: &[u8]) {
    match out {
        Some(conn) => match conn.send(bytes) {
            Ok(()) => eprintln!("→ envoyé {}", hex(bytes)),
            Err(e) => eprintln!("→ échec d'envoi {} : {e}", hex(bytes)),
        },
        None => eprintln!("→ pas de port de sortie, {} non envoyé", hex(bytes)),
    }
}

fn print_summary(stats: &Stats) {
    eprintln!("\n── Résumé ──");
    let total: usize = stats.per_kind.values().sum();
    eprintln!("messages : {total}");
    for (kind, n) in &stats.per_kind {
        eprintln!("  {kind:<8} {n}");
    }
    if let Some(max) = stats.jog_buckets.values().max() {
        eprintln!("débit jog max : ~{} messages/s", max * 4);
    }
    for (bytes, n) in &stats.unknown {
        eprintln!("  inconnu {} ×{n}", hex(bytes));
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let input = MidiInput::new("livedeck-sniff-in").map_err(|e| e.to_string())?;
    let output = MidiOutput::new("livedeck-sniff-out").map_err(|e| e.to_string())?;

    if args.list {
        // Write errors are ignored: `--list | grep -q` closes the pipe early.
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "Entrées :");
        for p in input.ports() {
            let name = input.port_name(&p).unwrap_or_else(|e| e.to_string());
            let _ = writeln!(stdout, "  {name}");
        }
        let _ = writeln!(stdout, "Sorties :");
        for p in output.ports() {
            let name = output.port_name(&p).unwrap_or_else(|e| e.to_string());
            let _ = writeln!(stdout, "  {name}");
        }
        return Ok(());
    }

    let hint = args
        .port
        .clone()
        .unwrap_or_else(|| io::CONTROLLER_PORT_HINT.to_string())
        .to_ascii_lowercase();
    let matches = |name: &str| name.to_ascii_lowercase().contains(&hint);

    let in_port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| matches(&n)))
        .ok_or_else(|| {
            format!("aucune entrée MIDI ne contient « {hint} » (branché en USB ? essayer --list)")
        })?;
    let in_name = input.port_name(&in_port).map_err(|e| e.to_string())?;

    let out_port = output
        .ports()
        .into_iter()
        .find(|p| output.port_name(p).is_ok_and(|n| matches(&n)));
    let mut out = match out_port {
        Some(p) => {
            let name = output.port_name(&p).unwrap_or_default();
            match output.connect(&p, "livedeck-sniff-out") {
                Ok(conn) => {
                    eprintln!("sortie : {name}");
                    Some(conn)
                }
                Err(e) => {
                    eprintln!("sortie {name} indisponible : {e}");
                    None
                }
            }
        }
        None => None,
    };

    let stats = Arc::new(Mutex::new(Stats::default()));
    let sniffer = Sniffer {
        decoder: Decoder::new(),
        json: args.json,
        stats: Arc::clone(&stats),
    };
    let _conn = input
        .connect(
            &in_port,
            "livedeck-sniff-in",
            |ts, bytes, s: &mut Sniffer| s.on_message(ts, bytes),
            sniffer,
        )
        .map_err(|e| e.to_string())?;
    eprintln!("entrée : {in_name} — bouge les contrôles (Ctrl+C pour quitter)");

    if args.dump {
        send(&mut out, &REQUEST_DUMP);
    }
    for msg in &args.send {
        send(&mut out, msg);
    }
    if let Some([status, note]) = args.leds {
        for value in [0x00, 0x01, 0x10, 0x20, 0x40, 0x60, 0x7E, 0x7F, 0x00] {
            eprintln!("LED {status:02X} {note:02X} ← {value:02X} ({value})");
            send(&mut out, &[status, note, value]);
            sleep(Duration::from_millis(1500));
        }
    }

    match args.seconds {
        Some(secs) => {
            let end = Instant::now() + Duration::from_secs_f64(secs);
            while Instant::now() < end {
                sleep(Duration::from_millis(50));
            }
        }
        None => loop {
            sleep(Duration::from_secs(1));
        },
    }

    if args.off {
        send(&mut out, &ALL_LEDS_OFF);
    }
    if let Ok(stats) = stats.lock() {
        print_summary(&stats);
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}
