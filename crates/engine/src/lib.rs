//! LiveDeck audio engine: clock, sequencers, voices and mixing.
//!
//! Runs on: audio thread (everything reachable from `process`/`render`).
//! Pure Rust, no I/O, no dependency on cpal/midir/egui, so it can be tested offline.
//! Real-time rules: docs/rust/realtime-audio.md §1.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod bass;
pub mod chords;
pub mod clock;
pub mod drums;
pub mod dsp;
pub mod fx;
pub mod limiter;
pub mod link;
pub mod looper;
pub mod melody;
pub mod metronome;
pub mod poly;
pub mod repeat;
pub mod sampler;
pub mod sequencer;
pub mod smooth;
pub mod style;

use bass::BassVoice;
use clock::Clock;
use drums::{DrumKit, DrumVoice};
use fx::{Delay, DelayTime, Reverb};
use limiter::Limiter;
use melody::Melody;
use metronome::Click;
use poly::PolySynth;
use repeat::{BeatRepeat, Division};
use sequencer::Sequencer;
use smooth::Smoothed;

/// Continuous parameters, normalized 0..=1 (curves are applied by the engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Param {
    MasterGain,
    MetronomeLevel,
    /// Level of the drums.
    DrumLevel,
    /// Level of the bass.
    BassLevel,
    /// Level of the melody synth.
    MelodyLevel,
    /// Melody synth filter around its patch cutoff (0.5 = the patch).
    Brightness,
    /// Base cutoff of the bass filter (linear knob, exponential in Hz).
    BassCutoff,
    /// Melody sent to the delay.
    DelaySend,
    /// Delay feedback, 0 ..= `fx::MAX_FEEDBACK` over the travel.
    DelayFeedback,
    /// Whole mix sent to the reverb (drums half as much, bass barely).
    ReverbSend,
    /// Crossfader between the decks: 0 = rhythm only, 0.5 = both full, 1 = melody only.
    Crossfade,
}

/// Beats per bar (4/4 only for now).
pub const BEATS_PER_BAR: u64 = 4;

/// Time budget of one audio block, in microseconds.
///
/// Used to express callback load as a fraction of the block (see `/perf-check`).
#[must_use]
pub fn block_budget_us(frames: u32, sample_rate: u32) -> f64 {
    if sample_rate == 0 {
        return 0.0;
    }
    f64::from(frames) * 1_000_000.0 / f64::from(sample_rate)
}

pub struct Engine {
    clock: Clock,
    click: Click,
    drums: DrumKit,
    bass: BassVoice,
    synth: PolySynth,
    chords: chords::ChordPads,
    melody: Melody,
    looper: looper::Looper,
    /// Style whose synth patch is loaded.
    synth_style: u8,
    sequencer: Sequencer,
    limiter: Limiter,
    metronome: bool,
    master_gain: Smoothed,
    metronome_level: Smoothed,
    drum_level: Smoothed,
    bass_level: Smoothed,
    melody_level: Smoothed,
    repeat: BeatRepeat,
    delay: Delay,
    delay_time: DelayTime,
    reverb: Reverb,
    delay_send: Smoothed,
    delay_feedback: Smoothed,
    reverb_send: Smoothed,
    crossfade: Smoothed,
    /// Clock position of the last drum hit (sequencer or pad) since the transport started.
    last_hit: Option<u64>,
    /// Length of a roll waiting for the next hit (no hit in the current division at the press).
    roll_pending: Option<usize>,
    /// Peaks of the last `process` call: drums, bass, melody (with its echoes), master (after the
    /// limiter), for the meters.
    peaks: [f32; 4],
    /// Mix bus (left once the effects return), allocated once; blocks larger than this are
    /// processed in slices.
    mono: Vec<f32>,
    /// Right channel: the dry mix is mono, only the reverb differs.
    right: Vec<f32>,
    /// Drum bus (before the drum level), same size as `mono`.
    drum_bus: Vec<f32>,
    /// Bass bus (kept out of the beat-repeat), same size as `mono`.
    bass_bus: Vec<f32>,
    /// Melody synth bus, same size as `mono`.
    synth_bus: Vec<f32>,
}

impl Engine {
    /// Allocates everything the audio thread will ever need. Call before starting the stream.
    #[must_use]
    pub fn new(sample_rate: u32, max_block: usize, bpm: f64) -> Self {
        let mut engine = Self::bare(sample_rate, max_block, bpm);
        // Start on the first kit: its sounds, patterns and lines; the synth patch and the effects
        // follow on the first block (`synth_style` differs).
        engine
            .sequencer
            .load_style(0, &mut engine.drums, &mut engine.bass);
        engine.synth_style = u8::MAX;
        engine
    }

    fn bare(sample_rate: u32, max_block: usize, bpm: f64) -> Self {
        Self {
            clock: Clock::new(sample_rate, bpm),
            click: Click::new(sample_rate),
            drums: DrumKit::new(sample_rate),
            bass: BassVoice::new(sample_rate),
            synth: PolySynth::new(sample_rate),
            chords: chords::ChordPads::default(),
            melody: Melody::new(),
            looper: looper::Looper::new(),
            synth_style: 0,
            sequencer: Sequencer::new(),
            limiter: Limiter::new(sample_rate),
            metronome: true,
            master_gain: Smoothed::new(1.0, sample_rate, 0.01),
            metronome_level: Smoothed::new(gain_curve(0.7), sample_rate, 0.01),
            drum_level: Smoothed::new(gain_curve(0.8), sample_rate, 0.01),
            bass_level: Smoothed::new(gain_curve(0.8), sample_rate, 0.01),
            melody_level: Smoothed::new(gain_curve(0.8), sample_rate, 0.01),
            // One beat at the slowest tempo: the longest division repeated.
            repeat: BeatRepeat::new(
                (f64::from(sample_rate) * 60.0 / clock::MIN_BPM).ceil() as usize
            ),
            delay: Delay::new(sample_rate),
            delay_time: DelayTime::default(),
            reverb: Reverb::new(sample_rate),
            delay_send: Smoothed::new(0.0, sample_rate, 0.01),
            delay_feedback: Smoothed::new(0.5 * fx::MAX_FEEDBACK, sample_rate, 0.05),
            reverb_send: Smoothed::new(0.0, sample_rate, 0.01),
            crossfade: Smoothed::new(0.5, sample_rate, 0.01),
            last_hit: None,
            roll_pending: None,
            peaks: [0.0; 4],
            mono: vec![0.0; max_block.max(64)],
            right: vec![0.0; max_block.max(64)],
            drum_bus: vec![0.0; max_block.max(64)],
            bass_bus: vec![0.0; max_block.max(64)],
            synth_bus: vec![0.0; max_block.max(64)],
        }
    }

    pub fn start(&mut self) {
        self.clock.start();
        self.sequencer.reset();
        self.melody.reset();
        self.looper.reset();
        self.sync_arp();
        self.repeat.release();
        self.last_hit = None;
        self.roll_pending = None;
    }

    pub fn stop(&mut self) {
        self.clock.stop();
        self.bass.note_off();
        self.chords.release_all(&mut self.synth);
        self.melody.stop();
        self.looper.stop();
        for owner in looper::NOTE_OWNER..looper::LOOP_OWNER + 8 {
            self.synth.release(owner);
        }
        self.sync_arp();
        self.repeat.release();
        self.roll_pending = None;
    }

    /// The arpeggio plays the chords while it is on and the transport runs; otherwise the pads
    /// sound their chords themselves.
    fn sync_arp(&mut self) {
        let on = self.melody.arp_active(&self.clock);
        if !on {
            self.synth.release(melody::ARP_OWNER);
        }
        let key = self.sequencer.bass().key();
        self.chords.set_arp(&mut self.synth, key, on);
    }

    /// Arpeggiator settings (mode, rate, octaves), from its next tick.
    pub fn set_arp(&mut self, settings: melody::ArpSettings) {
        let (swing, grid) = (self.sequencer.swing(), self.sequencer.grid());
        self.melody.set_arp(settings, &self.clock, swing, grid);
        self.sync_arp();
    }

    /// Phrase pad `index` (0..4, `variation` with Shift): starts or stops at the next bar (now if
    /// stopped).
    pub fn press_phrase(&mut self, index: u8, variation: bool) {
        self.melody
            .press_phrase(index, variation, self.clock.is_playing());
    }

    /// Note pad `pad` (0..8: degrees 1 to 8) pressed or released; recorded into the loop being
    /// recorded.
    pub fn note(&mut self, pad: u8, on: bool) {
        let key = (self.sequencer.bass().key(), self.chords.transpose());
        let beats = self.clock.beats();
        self.looper.note(pad, on, &mut self.synth, key, beats);
    }

    /// Loop pad `slot`: records an empty slot from the next bar, starts or stops a recorded one at
    /// the bar.
    pub fn press_loop(&mut self, slot: u8) {
        let (beats, playing) = (self.clock.beats(), self.clock.is_playing());
        self.looper.press(usize::from(slot), beats, playing);
    }

    /// REC: records the first empty loop from the next bar; pressed while recording, drops it.
    pub fn rec_loop(&mut self) {
        let (beats, playing) = (self.clock.beats(), self.clock.is_playing());
        self.looper.rec(beats, playing);
    }

    /// Erases loop `slot`.
    pub fn clear_loop(&mut self, slot: u8) {
        self.looper.clear(usize::from(slot), &mut self.synth);
    }

    /// Length of the next loop recorded, one step of `looper::LENGTHS` per unit.
    pub fn nudge_loop_bars(&mut self, delta: i8) {
        self.looper.nudge_bars(delta);
    }

    #[must_use]
    pub fn looper(&self) -> &looper::Looper {
        &self.looper
    }

    /// Melody transposition in scale degrees (chords, arpeggio, phrases), right away.
    pub fn set_transpose(&mut self, degrees: i8) {
        let key = self.sequencer.bass().key();
        self.chords.set_transpose(&mut self.synth, key, degrees);
    }

    #[must_use]
    pub fn melody(&self) -> &Melody {
        &self.melody
    }

    pub fn set_tempo(&mut self, bpm: f64) {
        self.clock.set_bpm(bpm);
    }

    pub fn set_metronome(&mut self, on: bool) {
        self.metronome = on;
    }

    /// Sets a normalized (0..=1) parameter; it glides there in ~10 ms.
    pub fn set_param(&mut self, param: Param, value: f32) {
        match param {
            Param::MasterGain => self.master_gain.set(gain_curve(value)),
            Param::MetronomeLevel => self.metronome_level.set(gain_curve(value)),
            Param::DrumLevel => self.drum_level.set(gain_curve(value)),
            Param::BassLevel => self.bass_level.set(gain_curve(value)),
            Param::MelodyLevel => self.melody_level.set(gain_curve(value)),
            Param::Brightness => self.synth.set_brightness(value),
            Param::BassCutoff => self.bass.set_cutoff(value),
            Param::DelaySend => self.delay_send.set(gain_curve(value)),
            Param::DelayFeedback => self
                .delay_feedback
                .set(value.clamp(0.0, 1.0) * fx::MAX_FEEDBACK),
            Param::ReverbSend => self.reverb_send.set(gain_curve(value)),
            Param::Crossfade => self.crossfade.set(if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.5
            }),
        }
    }

    /// Next delay time of the scroll (1/8 dotted, 1/4, 1/8, 1/4 dotted, 1/16), crossfaded now;
    /// a style change brings back the time of the style.
    pub fn cycle_delay_time(&mut self) {
        self.delay_time = self.delay_time.next();
    }

    #[must_use]
    pub fn delay_time(&self) -> DelayTime {
        self.delay_time
    }

    /// Peaks of the last rendered block: drums, bass, melody, master (0..=1, master after the
    /// limiter).
    #[must_use]
    pub fn peaks(&self) -> [f32; 4] {
        self.peaks
    }

    /// Lowest limiter gain of the last block (1 = not limiting).
    #[must_use]
    pub fn limiter_gain(&self) -> f32 {
        self.limiter.lowest
    }

    #[must_use]
    pub fn reverb_decay(&self) -> f32 {
        self.reverb.decay()
    }

    /// Plays a drum voice from the next rendered sample, playing or not.
    pub fn trigger_drum(&mut self, voice: DrumVoice, velocity: f32) {
        self.drums.trigger(voice, velocity);
        if self.clock.is_playing() {
            let now = self.clock.position();
            self.last_hit = Some(now);
            if let Some(len) = self.roll_pending.take() {
                self.repeat.engage(now, len, now + len as u64);
            }
        }
    }

    pub fn drums_mut(&mut self) -> &mut DrumKit {
        &mut self.drums
    }

    /// Queues drum pattern `index` (0-based) for the next bar (now if stopped).
    pub fn queue_pattern(&mut self, index: u8) {
        self.sequencer.queue(index, self.clock.is_playing());
    }

    /// Chord pad `pad` (0..4 = I, IV, V, vi; 4..8 = ii, iii, vii, I an octave up) pressed or
    /// released: played now in the key of the bass, held until released (or latched).
    pub fn chord(&mut self, pad: u8, on: bool) {
        let key = self.sequencer.bass().key();
        self.chords.pad(&mut self.synth, key, pad, on);
        self.looper.chord(pad, on, self.clock.beats());
    }

    /// Latch of the chord pads.
    pub fn set_latch(&mut self, on: bool) {
        self.chords.set_latch(&mut self.synth, on);
    }

    /// Voicing of the chords, applied to those sounding too.
    pub fn set_voicing(&mut self, voicing: chords::Voicing) {
        let key = self.sequencer.bass().key();
        self.chords.set_voicing(&mut self.synth, key, voicing);
    }

    /// Scale (bass and chords) for the next bar, or now if stopped.
    pub fn queue_scale(&mut self, scale: bass::Scale) {
        let playing = self.clock.is_playing();
        self.sequencer.bass_mut().queue_scale(scale, playing);
    }

    #[must_use]
    pub fn chords(&self) -> &chords::ChordPads {
        &self.chords
    }

    #[must_use]
    pub fn synth(&self) -> &PolySynth {
        &self.synth
    }

    /// Hands over the decoded sample kit (control thread, before the stream starts).
    pub fn set_sample_bank(&mut self, bank: sampler::SampleBank) {
        self.drums.set_samples(bank);
    }

    /// Drum source for the next bar (now if stopped): samples (when loaded) or synth.
    pub fn use_samples(&mut self, on: bool) {
        self.sequencer.queue_samples(on);
        if !self.clock.is_playing() {
            self.sequencer.apply_samples(&mut self.drums);
        }
    }

    #[must_use]
    pub fn drums(&self) -> &DrumKit {
        &self.drums
    }

    /// Style `index` (see `style::presets`) for the next bar, or now if stopped.
    pub fn queue_style(&mut self, index: u8) {
        self.sequencer.queue_style(index);
        if !self.clock.is_playing() {
            self.sequencer.apply_style(&mut self.drums, &mut self.bass);
        }
    }

    /// Bass line `index` (0-based) alone for the next bar (now if stopped); `queue_pattern` also
    /// brings the line of the same index.
    pub fn queue_bass_line(&mut self, index: u8) {
        let playing = self.clock.is_playing();
        self.sequencer.bass_mut().queue(index, playing);
    }

    /// Bass tonic (0 = C … 11 = B) for the next bar (now if stopped).
    pub fn queue_tonic(&mut self, tonic: u8) {
        let playing = self.clock.is_playing();
        self.sequencer.bass_mut().queue_tonic(tonic, playing);
    }

    /// Bass mute: closes the note sounding from the next step.
    pub fn set_bass_mute(&mut self, muted: bool) {
        self.sequencer.bass_mut().set_muted(muted);
    }

    /// Fill `index` (0-based) from the next beat to the end of the bar; ignored when stopped.
    pub fn queue_fill(&mut self, index: u8) {
        self.sequencer.queue_fill(index, self.clock.is_playing());
    }

    /// Muted drum voices (bit `i` = `DrumVoice::ALL[i]`), from the next step.
    pub fn set_mutes(&mut self, mutes: u8) {
        self.sequencer.set_mutes(mutes);
    }

    /// Drum density, 0 (kicks on the beat) ..= 1 (every step).
    pub fn set_density(&mut self, density: f32) {
        self.sequencer.set_density(density);
    }

    /// A Loop pad is pressed: a shorter division than the loop heard subdivides it; otherwise a
    /// new slice of `division` of the groove, from the last hit, loops from its next boundary.
    /// `None` (every pad released) goes back to the groove. Ignored when stopped.
    pub fn beat_repeat(&mut self, division: Option<Division>) {
        self.roll_pending = None;
        let Some(division) = division.filter(|_| self.clock.is_playing()) else {
            self.repeat.release();
            return;
        };
        let len = (division.beats() * self.clock.samples_per_beat())
            .round()
            .max(1.0) as u64;
        // Shorter than the loop heard: subdivide it (roll what is heard).
        if self.repeat.subdivide(len as usize) {
            return;
        }
        let now = self.clock.position();
        // Roll the last hit if it falls within one division before the press: the roll follows
        // what is being played. An older hit would replay the past (heard in the 012 test, at low
        // density): then the roll waits for the next hit instead.
        match self.last_hit.filter(|&hit| now.saturating_sub(hit) < len) {
            Some(hit) => self.repeat.engage(hit, len as usize, hit + len),
            None => {
                self.repeat.cancel_pending();
                self.roll_pending = Some(len as usize);
            }
        }
    }

    /// The Loop pad of `released` is let go while the one of `back` is still held: back to the
    /// loop of `back` (in phase if it is kept, else a new slice).
    pub fn beat_repeat_back(&mut self, released: Division, back: Division) {
        self.roll_pending = None;
        if !self.clock.is_playing() {
            self.repeat.release();
            return;
        }
        let len = |d: Division| (d.beats() * self.clock.samples_per_beat()).round() as usize;
        if !self.repeat.go_back(len(released), len(back)) {
            self.beat_repeat(Some(back));
        }
    }

    #[must_use]
    pub fn beat_repeat_engaged(&self) -> bool {
        self.repeat.engaged() || self.roll_pending.is_some()
    }

    /// Drum sequencer on/off (the bar position keeps running).
    pub fn set_sequencer(&mut self, on: bool) {
        self.sequencer.set_enabled(on);
    }

    /// Swing of the odd 16ths, 0.5 (straight) ..= 0.75.
    pub fn set_swing(&mut self, swing: f32) {
        self.sequencer.set_swing(swing);
    }

    #[must_use]
    pub fn sequencer(&self) -> &Sequencer {
        &self.sequencer
    }

    #[must_use]
    pub fn metronome_enabled(&self) -> bool {
        self.metronome
    }

    #[must_use]
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Non-finite samples caught by the master guard.
    #[must_use]
    pub fn nan_count(&self) -> u32 {
        self.limiter.nan_count
    }

    /// Renders into an interleaved buffer: the stereo mix goes to channels 0 and 1, the others get
    /// silence.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        self.peaks = [0.0; 4];
        self.limiter.begin_block();
        let mut frames_out = out.chunks_exact_mut(channels);
        loop {
            let remaining = frames_out.len();
            if remaining == 0 {
                break;
            }
            let n = remaining.min(self.mono.len());
            // The style changed (at the bar, or now when stopped): its synth patch and its
            // effects follow.
            if self.sequencer.style() != self.synth_style {
                self.synth_style = self.sequencer.style();
                if let Some(style) = self.sequencer.style_preset() {
                    self.synth.set_patch(style.synth);
                    self.delay_time = style.fx.delay;
                    self.reverb.set_decay(style.fx.reverb_decay);
                }
            }
            let (Some(mono), Some(right), Some(bus), Some(bass), Some(synth)) = (
                self.mono.get_mut(..n),
                self.right.get_mut(..n),
                self.drum_bus.get_mut(..n),
                self.bass_bus.get_mut(..n),
                self.synth_bus.get_mut(..n),
            ) else {
                break;
            };
            mono.fill(0.0);
            bus.fill(0.0);
            bass.fill(0.0);
            synth.fill(0.0);
            let melody_gain = self.melody_level.skip(n);
            self.click.gain = self.metronome_level.skip(n);
            let drum_gain = self.drum_level.skip(n);
            let bass_gain = self.bass_level.skip(n);
            let delay_send = self.delay_send.skip(n);
            self.delay.set_feedback(self.delay_feedback.skip(n));
            let reverb_send = self.reverb_send.skip(n);
            let (rhythm_fade, melody_fade) = crossfade_gains(self.crossfade.skip(n));
            let (drum_gain, bass_gain) = (drum_gain * rhythm_fade, bass_gain * rhythm_fade);
            let melody_gain = melody_gain * melody_fade;
            self.delay
                .set_length((self.delay_time.beats() * self.clock.samples_per_beat()) as f32);
            let start = self.clock.position();
            render_mono(
                Voices {
                    clock: &mut self.clock,
                    click: &mut self.click,
                    metronome: self.metronome,
                    sequencer: &mut self.sequencer,
                    drums: &mut self.drums,
                    bass: &mut self.bass,
                    synth: &mut self.synth,
                    chords: &self.chords,
                    melody: &mut self.melody,
                    looper: &mut self.looper,
                    last_hit: &mut self.last_hit,
                    roll_pending: &mut self.roll_pending,
                    repeat: &mut self.repeat,
                },
                Buses {
                    mono,
                    drums: bus,
                    bass,
                    synth,
                },
            );
            self.repeat.process(bus, start);
            for ((((left, right), &drum), &bass), &synth) in mono
                .iter_mut()
                .zip(right.iter_mut())
                .zip(bus.iter())
                .zip(bass.iter())
                .zip(synth.iter())
            {
                let (drum, bass, synth) = (drum * drum_gain, bass * bass_gain, synth * melody_gain);
                let echo = self.delay.tick(synth * delay_send);
                let (wet_l, wet_r) = self
                    .reverb
                    .tick((synth + 0.5 * drum + 0.1 * bass + echo) * reverb_send);
                let [p_drum, p_bass, p_melody, _] = &mut self.peaks;
                *p_drum = p_drum.max(drum.abs());
                *p_bass = p_bass.max(bass.abs());
                *p_melody = p_melody.max((synth + echo).abs());
                let dry = *left + drum + bass + synth + echo;
                let master = self.master_gain.tick();
                *left = (dry + wet_l) * master;
                *right = (dry + wet_r) * master;
            }
            self.limiter.process_stereo(mono, right);
            if let Some(master) = self.peaks.get_mut(3) {
                for (&l, &r) in mono.iter().zip(right.iter()) {
                    *master = master.max(l.abs()).max(r.abs());
                }
            }
            for ((frame, &left), &right) in frames_out
                .by_ref()
                .take(n)
                .zip(mono.iter())
                .zip(right.iter())
            {
                for (ch, slot) in frame.iter_mut().enumerate() {
                    *slot = match ch {
                        0 => left,
                        // A mono output gets the left (the dry mix is centered anyway).
                        1 => right,
                        _ => 0.0,
                    };
                }
            }
        }
    }
}

/// Crossfader law (rhythm, melody): both full in the middle, each deck fades out over the far half
/// of the travel (x² so the fade sounds even). Applied before the sends: echoes and reverb tails
/// of a deck faded out ring on.
fn crossfade_gains(x: f32) -> (f32, f32) {
    let side = |d: f32| {
        let g = (2.0 * d).clamp(0.0, 1.0);
        g * g
    };
    (side(1.0 - x), side(x))
}

/// Perceptual fader law: 0..=1 → amplitude (x², so the middle of the travel is about -12 dB).
fn gain_curve(x: f32) -> f32 {
    let x = if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    };
    x * x
}

/// What one slice renders (borrowed from the engine, which also owns the mix bus).
struct Voices<'a> {
    clock: &'a mut Clock,
    click: &'a mut Click,
    metronome: bool,
    sequencer: &'a mut Sequencer,
    drums: &'a mut DrumKit,
    bass: &'a mut BassVoice,
    synth: &'a mut PolySynth,
    chords: &'a chords::ChordPads,
    melody: &'a mut Melody,
    looper: &'a mut looper::Looper,
    last_hit: &'a mut Option<u64>,
    roll_pending: &'a mut Option<usize>,
    repeat: &'a mut BeatRepeat,
}

/// Buffers of one slice: click, drums, bass, melody synth (same length).
struct Buses<'a> {
    mono: &'a mut [f32],
    drums: &'a mut [f32],
    bass: &'a mut [f32],
    synth: &'a mut [f32],
}

/// Renders one slice (click into `mono`, drums, bass and synth into their buses), splitting it at
/// beats, sequencer steps and melody events so every click, hit and note starts on its exact
/// sample.
fn render_mono(v: Voices<'_>, buses: Buses<'_>) {
    let mut rest = buses.mono;
    let mut bus_rest = buses.drums;
    let mut bass_rest = buses.bass;
    let mut synth_rest = buses.synth;
    while !rest.is_empty() {
        let len = rest.len();
        let swing = (v.sequencer.swing(), v.sequencer.grid());
        let beat = v.clock.next_beat_within(len);
        let step = v.sequencer.next_step_within(v.clock, len);
        let melody = v.melody.next_event_within(v.clock, swing.0, swing.1, len);
        let beat_at = beat.map_or(len, |(offset, _)| offset);
        let step_at = step.unwrap_or(len);
        let melody_at = melody.unwrap_or(len);
        // Every offset is < len by construction; clamped anyway so the split can never panic.
        let segment_len = beat_at.min(step_at).min(melody_at).min(len);
        let (segment, tail) = rest.split_at_mut(segment_len);
        let (bus_segment, bus_tail) = bus_rest.split_at_mut(segment_len.min(bus_rest.len()));
        let (bass_segment, bass_tail) = bass_rest.split_at_mut(segment_len.min(bass_rest.len()));
        let (synth_segment, synth_tail) =
            synth_rest.split_at_mut(segment_len.min(synth_rest.len()));
        v.click.render_add(segment);
        v.drums.render_add(bus_segment, 1.0);
        v.bass.render_add(bass_segment);
        v.synth.render_add(synth_segment);
        v.clock.advance(segment_len);
        if let Some((_, beat)) = beat
            && beat_at == segment_len
        {
            if v.metronome {
                v.click.trigger(beat % BEATS_PER_BAR == 0);
            }
            v.clock.consume_beat();
        }
        if step_at == segment_len && step.is_some() {
            let bar_step = usize::from(v.sequencer.step_in_bar());
            if v.sequencer.fire(v.drums, v.bass, v.clock) != 0 {
                let hit = v.clock.position();
                *v.last_hit = Some(hit);
                if let Some(len) = v.roll_pending.take() {
                    v.repeat.engage(hit, len, hit + len as u64);
                }
            }
            let sequencer = &*v.sequencer;
            v.melody.fire_step(
                bar_step,
                v.synth,
                (sequencer.bass().key(), v.chords.transpose()),
                sequencer.style_preset().map(|style| &style.phrases),
                v.clock,
                |len, held| sequencer.gate_of_fired(v.clock, len, held),
            );
            v.looper.fire_step(
                bar_step,
                v.clock.beats(),
                v.synth,
                (
                    sequencer.bass().key(),
                    v.chords.transpose(),
                    v.chords.voicing(),
                ),
            );
        }
        if melody_at == segment_len && melody.is_some() {
            let key = v.sequencer.bass().key();
            v.melody.on_time(v.synth, v.chords, key, v.clock, swing);
        }
        rest = tail;
        bus_rest = bus_tail;
        bass_rest = bass_tail;
        synth_rest = synth_tail;
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)] // tests may panic: that is how they fail
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn render(engine: &mut Engine, frames: usize, channels: usize, block: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * channels];
        for chunk in out.chunks_mut(block * channels) {
            engine.process(chunk, channels);
        }
        out
    }

    /// Indices of samples where a click starts (silence before, sound after).
    fn onsets(mono: &[f32]) -> Vec<usize> {
        mono.windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] == 0.0 && w[1] != 0.0)
            .map(|(i, _)| i)
            .collect()
    }

    #[test]
    fn block_budget_matches_reference_buffer() {
        // 128 frames @ 48 kHz is the reference buffer of the project.
        assert!((block_budget_us(128, 48_000) - 2_666.666).abs() < 0.01);
        assert_eq!(block_budget_us(128, 0), 0.0);
    }

    #[test]
    fn clicks_fall_on_beats_whatever_the_block_size() {
        for block in [64, 128, 333, 512] {
            let mut engine = Engine::new(SR, 256, 120.0);
            engine.set_sequencer(false);
            engine.start();
            let out = render(&mut engine, 3 * 24_000 + 100, 1, block);
            // An onset at i means sample i is silent and i + 1 sounds: a click starting on beat
            // sample b begins with sin(0) = 0, so it is reported at b.
            assert_eq!(
                onsets(&out),
                vec![0, 24_000, 48_000, 72_000],
                "block {block}"
            );
        }
    }

    #[test]
    fn output_is_finite_and_below_ceiling() {
        let mut engine = Engine::new(SR, 128, 180.0);
        engine.start();
        let out = render(&mut engine, SR as usize * 2, 2, 128);
        assert!(
            out.iter()
                .all(|s| s.is_finite() && s.abs() <= limiter::CEILING)
        );
        assert!(
            out.iter().any(|s| s.abs() > 0.1),
            "metronome should be audible"
        );
        assert_eq!(engine.nan_count(), 0);
    }

    #[test]
    fn stopped_engine_is_silent() {
        let mut engine = Engine::new(SR, 128, 120.0);
        let out = render(&mut engine, 10_000, 2, 128);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn drums_sound_at_block_start_even_when_stopped() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.trigger_drum(DrumVoice::Kick, 1.0);
        let out = render(&mut engine, 128, 1, 128);
        assert!(out[0] != 0.0, "the kick click starts on the first sample");
        assert!(out.iter().any(|s| s.abs() > 0.3));
    }

    #[test]
    fn roll_waits_for_the_next_hit_when_the_last_one_is_old() {
        // Density 0 on the built-in pattern A (no kit loaded): kicks on beats 1 and 3 only
        // (samples 0 and 48 000).
        let mut engine = Engine::bare(SR, 128, 120.0);
        engine.set_metronome(false);
        engine.set_param(Param::MasterGain, 0.5);
        engine.set_density(0.0);
        engine.set_bass_mute(true);
        engine.start();
        render(&mut engine, 24_000, 1, 128);
        // Pressed on beat 2: the kick of beat 1 is a whole beat old, longer than a 1/16.
        engine.beat_repeat(Division::from_denominator(16));
        assert!(engine.beat_repeat_engaged());
        let out = render(&mut engine, 48_000, 1, 128);
        // Live until the kick of beat 3 (24 000 samples on), then that kick rolls every 6 000.
        assert!(
            out[..23_000].iter().all(|s| s.abs() < 0.05),
            "no old kick replayed"
        );
        let turn = |k: usize| &out[24_000 + k * 6_000 + 500..24_000 + (k + 1) * 6_000 - 1_000];
        assert!(turn(2).iter().any(|s| s.abs() > 0.1));
        let diff = turn(2)
            .iter()
            .zip(turn(3))
            .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(diff < 1.0e-3, "{diff}");
    }

    #[test]
    fn arpeggio_plays_the_held_chord_and_stops_with_the_transport() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.set_metronome(false);
        engine.set_sequencer(false);
        engine.set_bass_mute(true);
        engine.set_arp(melody::ArpSettings {
            mode: melody::ArpMode::Up,
            rate: melody::ArpRate::Quarter,
            octaves: 1,
        });
        engine.chord(0, true);
        // Stopped: the pad sounds its chord itself.
        assert_eq!(engine.synth().active_voices(), 3);
        engine.start();
        // Playing: the arpeggio takes over, one note per beat; the chord releases.
        let out = render(&mut engine, 24_000 * 3, 1, 128);
        assert!(out[..1_000].iter().any(|s| s.abs() > 0.01));
        let melody = engine.melody();
        assert_eq!(melody.arp().mode, melody::ArpMode::Up);
        engine.stop();
        // Stopped again: the held chord is released by the stop, nothing sounds new.
        assert_eq!(engine.chords().sounding(), 0);
    }

    /// Energy of the melody tail, `(left, left − right)`, one to two seconds after a short chord.
    fn chord_tail(delay_send: f32, reverb_send: f32) -> (f32, f32) {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.set_metronome(false);
        engine.set_bass_mute(true);
        engine.set_param(Param::DelaySend, delay_send);
        engine.set_param(Param::ReverbSend, reverb_send);
        engine.chord(0, true);
        render(&mut engine, 4_800, 2, 128);
        engine.chord(0, false);
        let out = render(&mut engine, SR as usize * 2, 2, 128);
        let tail = &out.as_chunks::<2>().0[SR as usize..];
        let energy = tail.iter().map(|[l, _]| l * l).sum();
        let side = tail.iter().map(|[l, r]| (l - r) * (l - r)).sum();
        (energy, side)
    }

    #[test]
    fn meters_follow_each_bus() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.set_metronome(false);
        engine.set_sequencer(false);
        engine.set_bass_mute(true);
        engine.chord(0, true);
        render(&mut engine, 4_800, 2, 128);
        let [drums, bass, melody, master] = engine.peaks();
        assert_eq!((drums, bass), (0.0, 0.0));
        assert!(melody > 0.01 && master > 0.01, "{melody} {master}");
        assert!(master <= limiter::CEILING);
        assert!(
            (engine.limiter_gain() - 1.0).abs() < 1e-6,
            "a chord alone is not limited"
        );
    }

    #[test]
    fn sends_add_echoes_and_a_stereo_reverb_tail() {
        let (dry, dry_side) = chord_tail(0.0, 0.0);
        let (echo, echo_side) = chord_tail(1.0, 0.0);
        let (wet, wet_side) = chord_tail(0.0, 1.0);
        // Dry: the synth release is over and both channels are the same.
        assert_eq!(dry_side, 0.0);
        assert!(echo > dry * 10.0 + 1e-6, "{echo} {dry}");
        // The delay stays in the center, the reverb is wide.
        assert_eq!(echo_side, 0.0);
        assert!(
            wet > dry * 10.0 + 1e-6 && wet_side > 0.0,
            "{wet} {wet_side}"
        );
    }

    #[test]
    fn delay_time_follows_the_style_until_scrolled() {
        let mut engine = Engine::new(SR, 128, 120.0);
        render(&mut engine, 128, 2, 128);
        engine.cycle_delay_time();
        assert_eq!(engine.delay_time(), style::TEXTS[0].fx.delay.next());
        // The Dub kit brings its delay back and its long reverb (stopped: right away).
        engine.queue_style(2);
        render(&mut engine, 256, 2, 128);
        let dub = style::TEXTS[2].fx;
        assert_eq!(engine.delay_time(), dub.delay);
        assert!((engine.reverb_decay() - dub.reverb_decay).abs() < 1e-6);
        engine.cycle_delay_time();
        assert_eq!(engine.delay_time(), dub.delay.next());
    }

    #[test]
    fn mix_goes_to_first_two_channels_only() {
        let mut engine = Engine::new(SR, 128, 120.0);
        engine.start();
        let out = render(&mut engine, 2_000, 4, 128);
        for frame in out.as_chunks::<4>().0 {
            assert_eq!(frame[0], frame[1]);
            assert_eq!(frame[2], 0.0);
            assert_eq!(frame[3], 0.0);
        }
    }

    #[test]
    fn crossfader_fades_one_deck_at_each_end() {
        assert_eq!(crossfade_gains(0.5), (1.0, 1.0));
        assert_eq!(crossfade_gains(0.0), (1.0, 0.0));
        assert_eq!(crossfade_gains(1.0), (0.0, 1.0));
        let (rhythm, melody) = crossfade_gains(0.75);
        assert!((rhythm - 0.25).abs() < 1e-6 && melody == 1.0);
    }
}
