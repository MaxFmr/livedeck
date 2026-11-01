//! Audio output through cpal (CoreAudio on macOS): device and format selection, stream start,
//! and real-time counters (callback load, xruns).
//!
//! Runs on: control thread to build the stream; the render closure runs on the audio thread, so
//! the wrapper added here only reads `Instant` once per block and touches atomics.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, SampleFormat, StreamConfig, SupportedBufferSize};

pub const PREFERRED_SAMPLE_RATE: u32 = 48_000;

/// Counters shared between the audio callback and the rest of the app.
#[derive(Debug)]
pub struct AudioStats {
    /// Stream start, reference for the `*_at_ms` timestamps.
    pub started: Instant,
    /// Time of the last xrun, in ms since `started` (0 = none yet).
    pub last_xrun_at_ms: AtomicU64,
    /// The OS re-routed the default-device stream by itself (e.g. headphones, interface unplugged).
    pub device_changes: AtomicU32,
    /// Worst callback duration since last `take_max_load`, in nanoseconds.
    max_callback_ns: AtomicU64,
    /// Largest block seen, in frames.
    max_frames: AtomicU32,
    pub xruns: AtomicU32,
    pub errors: AtomicU32,
    pub callbacks: AtomicU64,
    /// The device disappeared or the stream became unusable: the control thread must rebuild it.
    pub invalidated: AtomicBool,
}

impl Default for AudioStats {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            last_xrun_at_ms: AtomicU64::new(0),
            device_changes: AtomicU32::new(0),
            max_callback_ns: AtomicU64::new(0),
            max_frames: AtomicU32::new(0),
            xruns: AtomicU32::new(0),
            errors: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            invalidated: AtomicBool::new(false),
        }
    }
}

impl AudioStats {
    fn ms_since_start(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis())
            .unwrap_or(u64::MAX)
            .max(1)
    }

    /// Returns the worst callback load (fraction of the block duration) and resets it.
    pub fn take_max_load(&self, sample_rate: u32) -> f64 {
        let ns = self.max_callback_ns.swap(0, Ordering::Relaxed) as f64;
        let frames = f64::from(self.max_frames.load(Ordering::Relaxed).max(1));
        ns / (frames * 1.0e9 / f64::from(sample_rate.max(1)))
    }
}

/// Host-time stamps of one block, in µs — the clock of CoreMIDI timestamps (`midir`), so a gesture
/// and the moment its sound leaves the driver can be compared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockStamp {
    /// When the callback actually ran (read from the host clock on entry).
    pub called_us: u64,
    /// CoreAudio's timestamp of the block (`mHostTime`): when its first sample is handed to the
    /// device, ahead of `called_us` by about one buffer plus the safety offset.
    pub callback_us: u64,
    /// cpal's estimate of when the first sample reaches the output: `callback_us` + buffer +
    /// safety offset + device latency (may count the buffer twice, see iteration 009).
    pub playback_us: u64,
}

fn micros(instant: cpal::StreamInstant) -> u64 {
    u64::try_from(instant.as_nanos() / 1_000).unwrap_or(u64::MAX)
}

/// Settings actually obtained from the device.
#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// `None` when the device decides (cpal `BufferSize::Default`).
    pub buffer_frames: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("aucune sortie audio par défaut")]
    NoDevice,
    #[error("aucun format f32 disponible sur « {0} »")]
    NoF32Format(String),
    #[error("cpal : {0}")]
    Cpal(#[from] cpal::Error),
}

/// A running output stream. Dropping it stops the sound.
pub struct AudioOutput {
    _stream: cpal::Stream,
    pub info: OutputInfo,
    pub stats: Arc<AudioStats>,
}

/// Opens the default output in f32, at 48 kHz when supported, with the first of the preferred
/// buffer sizes the device accepts. `make_render` receives the settings actually obtained (so the
/// engine is built at the real sample rate) and returns the closure called as
/// `render(interleaved, channels, stamp)` for every block.
///
/// `render` must follow the real-time rules (docs/rust/realtime-audio.md §1).
pub fn start_output<F, R>(
    preferred_buffers: &[u32],
    make_render: F,
) -> Result<AudioOutput, AudioError>
where
    F: FnOnce(&OutputInfo) -> R,
    R: FnMut(&mut [f32], usize, BlockStamp) + Send + 'static,
{
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(AudioError::NoDevice)?;
    let name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "?".to_string());

    let default = device.default_output_config()?;
    let range = device
        .supported_output_configs()?
        .filter(|r| r.sample_format() == SampleFormat::F32 && r.channels() == default.channels())
        .find(|r| (r.min_sample_rate()..=r.max_sample_rate()).contains(&PREFERRED_SAMPLE_RATE))
        .map(|r| r.with_sample_rate(PREFERRED_SAMPLE_RATE));
    let supported = match range {
        Some(cfg) => cfg,
        None if default.sample_format() == SampleFormat::F32 => default,
        None => return Err(AudioError::NoF32Format(name)),
    };

    let buffer_frames = match supported.buffer_size() {
        SupportedBufferSize::Range { min, max } => preferred_buffers
            .iter()
            .copied()
            .find(|frames| (*min..=*max).contains(frames)),
        SupportedBufferSize::Unknown => preferred_buffers.first().copied(),
    };
    let config = StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: buffer_frames.map_or(BufferSize::Default, BufferSize::Fixed),
    };
    let channels = usize::from(config.channels);
    let info = OutputInfo {
        device: name,
        sample_rate: config.sample_rate,
        channels: config.channels,
        buffer_frames,
    };
    let mut render = make_render(&info);

    let stats = Arc::new(AudioStats::default());
    let data_stats = Arc::clone(&stats);
    let err_stats = Arc::clone(&stats);
    let stream = device.build_output_stream::<f32, _, _>(
        config,
        move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
            let started = Instant::now();
            let timestamp = info.timestamp();
            let stamp = BlockStamp {
                called_us: crate::system::host_time_us(),
                callback_us: micros(timestamp.callback),
                playback_us: micros(timestamp.playback),
            };
            render(data, channels, stamp);
            let ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            data_stats.max_callback_ns.fetch_max(ns, Ordering::Relaxed);
            let frames = u32::try_from(data.len() / channels.max(1)).unwrap_or(u32::MAX);
            data_stats.max_frames.fetch_max(frames, Ordering::Relaxed);
            data_stats.callbacks.fetch_add(1, Ordering::Relaxed);
        },
        move |err: cpal::Error| match err.kind() {
            ErrorKind::Xrun => {
                err_stats.xruns.fetch_add(1, Ordering::Relaxed);
                err_stats
                    .last_xrun_at_ms
                    .store(err_stats.ms_since_start(), Ordering::Relaxed);
            }
            // A stream on the default device follows it by itself: only counted.
            ErrorKind::DeviceChanged => {
                err_stats.device_changes.fetch_add(1, Ordering::Relaxed);
            }
            kind => {
                err_stats.errors.fetch_add(1, Ordering::Relaxed);
                if matches!(
                    kind,
                    ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated
                ) {
                    err_stats.invalidated.store(true, Ordering::Relaxed);
                }
            }
        },
        None,
    )?;
    stream.play()?;

    Ok(AudioOutput {
        _stream: stream,
        info,
        stats,
    })
}
