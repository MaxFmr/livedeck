//! Live link to the DJControl Mix: connects when it appears, notices when it goes away, forwards
//! decoded events and keeps its LEDs in sync.
//!
//! Runs on: control thread (`poll`). The CoreMIDI callback only copies the bytes into a lock-free
//! queue, so a slow control loop never blocks MIDI reception, then unparks the control thread so a
//! loop waiting in `std::thread::park_timeout` handles the gesture at once.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::Thread;
use std::time::{Duration, Instant};

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::controller::{ALL_LEDS_OFF, ControllerEvent, Decoder, REQUEST_DUMP};
use crate::is_controller_port;
use crate::leds::LedBank;

const SCAN_EVERY: Duration = Duration::from_secs(1);
/// ~4 s of a fast jog plus faders; overflow is counted, never blocking.
const INPUT_CAPACITY: usize = 2048;

/// One raw message, copied as-is in the CoreMIDI callback.
#[derive(Debug, Clone, Copy)]
struct RawMidi {
    ts_us: u64,
    len: u8,
    bytes: [u8; 3],
}

struct CallbackData {
    tx: Producer<RawMidi>,
    dropped: Arc<AtomicU32>,
    /// The control thread, woken on every message.
    waker: Thread,
}

struct Connection {
    name: String,
    _input: MidiInputConnection<CallbackData>,
    output: Option<MidiOutputConnection>,
    rx: Consumer<RawMidi>,
    dropped: Arc<AtomicU32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkEvent {
    Connected {
        port: String,
        leds: bool,
    },
    Disconnected,
    /// A decoded message; `ts_us` is the CoreMIDI host time in µs.
    Input {
        ts_us: u64,
        event: ControllerEvent,
    },
}

pub struct ControllerLink {
    scanner: Option<MidiInput>,
    connection: Option<Connection>,
    decoder: Decoder,
    leds: LedBank,
    last_scan: Option<Instant>,
    waker: Thread,
}

impl Default for ControllerLink {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerLink {
    /// Create it on the control thread: that thread is unparked when a message arrives.
    #[must_use]
    pub fn new() -> Self {
        Self {
            waker: std::thread::current(),
            scanner: MidiInput::new("livedeck-scan").ok(),
            connection: None,
            decoder: Decoder::new(),
            leds: LedBank::new(),
            last_scan: None,
        }
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// Messages lost because the input queue was full (since the current connection).
    #[must_use]
    pub fn dropped(&self) -> u32 {
        self.connection
            .as_ref()
            .map_or(0, |c| c.dropped.load(Ordering::Relaxed))
    }

    pub fn leds_mut(&mut self) -> &mut LedBank {
        &mut self.leds
    }

    /// Call often (every few ms): scans for the controller once per second, forwards received
    /// events in order, then sends LED changes.
    pub fn poll(&mut self, mut on_event: impl FnMut(LinkEvent)) {
        if self.last_scan.is_none_or(|t| t.elapsed() >= SCAN_EVERY) {
            self.last_scan = Some(Instant::now());
            self.scan(&mut on_event);
        }
        if let Some(conn) = self.connection.as_mut() {
            while let Ok(raw) = conn.rx.pop() {
                let bytes = raw.bytes.get(..usize::from(raw.len)).unwrap_or(&raw.bytes);
                let Some(event) = self.decoder.decode(bytes) else {
                    continue;
                };
                match event {
                    ControllerEvent::PadModeSelect { deck, mode, .. }
                    | ControllerEvent::Pad { deck, mode, .. } => self.leds.mode_shown(deck, mode),
                    _ => {}
                }
                on_event(LinkEvent::Input {
                    ts_us: raw.ts_us,
                    event,
                });
            }
            if let Some(out) = conn.output.as_mut() {
                self.leds.flush(|msg| {
                    let _ = out.send(&msg);
                });
            }
        }
    }

    fn controller_present(&self) -> Option<String> {
        let scanner = self.scanner.as_ref()?;
        scanner
            .ports()
            .iter()
            .filter_map(|p| scanner.port_name(p).ok())
            .find(|name| is_controller_port(name))
    }

    fn scan(&mut self, on_event: &mut impl FnMut(LinkEvent)) {
        let present = self.controller_present();
        match (&self.connection, present) {
            (Some(conn), Some(name)) if conn.name == name => {}
            (Some(_), _) => {
                self.connection = None;
                on_event(LinkEvent::Disconnected);
            }
            (None, Some(name)) => {
                if let Some(conn) = connect(&name, self.waker.clone()) {
                    let leds = conn.output.is_some();
                    self.connection = Some(conn);
                    self.decoder = Decoder::new();
                    self.leds.invalidate();
                    on_event(LinkEvent::Connected { port: name, leds });
                }
            }
            (None, None) => {}
        }
    }
}

impl Drop for ControllerLink {
    fn drop(&mut self) {
        if let Some(out) = self.connection.as_mut().and_then(|c| c.output.as_mut()) {
            let _ = out.send(&ALL_LEDS_OFF);
        }
    }
}

fn connect(name: &str, waker: Thread) -> Option<Connection> {
    let input = MidiInput::new("livedeck-in").ok()?;
    let port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).is_ok_and(|n| n == name))?;
    let (tx, rx) = RingBuffer::new(INPUT_CAPACITY);
    let dropped = Arc::new(AtomicU32::new(0));
    let data = CallbackData {
        tx,
        dropped: Arc::clone(&dropped),
        waker,
    };
    let input_conn = input
        .connect(
            &port,
            "livedeck-in",
            |ts_us, bytes, data: &mut CallbackData| {
                let mut raw = RawMidi {
                    ts_us,
                    len: 0,
                    bytes: [0; 3],
                };
                for (slot, &b) in raw.bytes.iter_mut().zip(bytes) {
                    *slot = b;
                    raw.len += 1;
                }
                if data.tx.push(raw).is_err() {
                    data.dropped.fetch_add(1, Ordering::Relaxed);
                }
                data.waker.unpark();
            },
            data,
        )
        .ok()?;

    let output = MidiOutput::new("livedeck-out").ok().and_then(|out| {
        let port = out
            .ports()
            .into_iter()
            .find(|p| out.port_name(p).is_ok_and(|n| n == name))?;
        out.connect(&port, "livedeck-out").ok()
    });
    let mut connection = Connection {
        name: name.to_string(),
        _input: input_conn,
        output,
        rx,
        dropped,
    };
    if let Some(out) = connection.output.as_mut() {
        // Positions of every knob and fader, so the app starts from the physical state.
        let _ = out.send(&REQUEST_DUMP);
    }
    Some(connection)
}
