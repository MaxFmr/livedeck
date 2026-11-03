//! Send effects: a tempo-synced delay and a stereo reverb (8-line feedback delay network).
//!
//! Runs on: audio thread. Every buffer is allocated in `new`; changing the delay time crossfades
//! two read heads instead of moving one (no pitch glide, no click).

use crate::dsp::{Svf, soft_clip};

/// Delay times, in the order the Shift + PFL B button scrolls them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelayTime {
    #[default]
    DottedEighth,
    Quarter,
    Eighth,
    DottedQuarter,
    Sixteenth,
}

impl DelayTime {
    pub const ALL: [Self; 5] = [
        Self::DottedEighth,
        Self::Quarter,
        Self::Eighth,
        Self::DottedQuarter,
        Self::Sixteenth,
    ];

    #[must_use]
    pub fn beats(self) -> f64 {
        match self {
            Self::DottedEighth => 0.75,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::DottedQuarter => 1.5,
            Self::Sixteenth => 0.25,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::DottedEighth => "1/8 pointé",
            Self::Quarter => "1/4",
            Self::Eighth => "1/8",
            Self::DottedQuarter => "1/4 pointé",
            Self::Sixteenth => "1/16",
        }
    }

    #[must_use]
    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as u8
    }

    #[must_use]
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(usize::from(index) % Self::ALL.len())
            .copied()
            .unwrap_or_default()
    }

    /// The next time of the scroll (back to the first after the last).
    #[must_use]
    pub fn next(self) -> Self {
        Self::from_index(self.index() + 1)
    }
}

/// Longest delay line: the longest time (1/4 dotted) at the slowest tempo, plus margin.
pub const MAX_DELAY_SECONDS: f32 = 2.0;
/// Highest feedback (knob fully up): long dub tails that still die out.
pub const MAX_FEEDBACK: f32 = 0.95;
const CROSSFADE_SECONDS: f32 = 0.04;

/// Mono delay with filtered, softly saturated feedback.
#[derive(Debug, Clone)]
pub struct Delay {
    buffer: Vec<f32>,
    write: usize,
    /// Delay of the head heard, in samples.
    current: f32,
    /// Head faded in while `fade` < 1.
    next: f32,
    fade: f32,
    fade_step: f32,
    /// Time asked for while a crossfade runs: it follows that one.
    pending: Option<f32>,
    feedback: f32,
    lowpass: Svf,
    highpass: Svf,
}

impl Delay {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let len = (sr * MAX_DELAY_SECONDS).ceil() as usize + 2;
        let mut lowpass = Svf::default();
        lowpass.set(sr, 3_500.0, 0.6);
        let mut highpass = Svf::default();
        highpass.set(sr, 150.0, 0.6);
        let start = sr * 0.375;
        Self {
            buffer: vec![0.0; len],
            write: 0,
            current: start,
            next: start,
            fade: 1.0,
            fade_step: 1.0 / (CROSSFADE_SECONDS * sr),
            pending: None,
            feedback: 0.0,
            lowpass,
            highpass,
        }
    }

    /// Delay time in samples (from the tempo); a change crossfades to a new head.
    pub fn set_length(&mut self, samples: f32) {
        let max = (self.buffer.len() - 2) as f32;
        let samples = if samples.is_finite() {
            samples.clamp(1.0, max)
        } else {
            return;
        };
        if self.fade < 1.0 {
            if (samples - self.next).abs() >= 0.5 {
                self.pending = Some(samples);
            }
        } else if (samples - self.current).abs() >= 0.5 {
            self.next = samples;
            self.fade = 0.0;
        }
    }

    /// Delay time heard (the target of a running crossfade), in samples.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.pending.unwrap_or(self.next)
    }

    /// Feedback 0..=`MAX_FEEDBACK`.
    pub fn set_feedback(&mut self, feedback: f32) {
        if feedback.is_finite() {
            self.feedback = feedback.clamp(0.0, MAX_FEEDBACK);
        }
    }

    fn read(&self, delay: f32) -> f32 {
        let len = self.buffer.len();
        let pos = self.write as f32 - delay;
        let pos = if pos < 0.0 { pos + len as f32 } else { pos };
        let i0 = (pos as usize).min(len - 1);
        let frac = pos - i0 as f32;
        let i1 = if i0 + 1 >= len { 0 } else { i0 + 1 };
        let a = self.buffer.get(i0).copied().unwrap_or(0.0);
        let b = self.buffer.get(i1).copied().unwrap_or(0.0);
        a + (b - a) * frac
    }

    /// One sample in, the delayed signal out.
    pub fn tick(&mut self, input: f32) -> f32 {
        let mut out = self.read(self.current);
        if self.fade < 1.0 {
            let incoming = self.read(self.next);
            out += (incoming - out) * self.fade;
            self.fade += self.fade_step;
            if self.fade >= 1.0 {
                self.current = self.next;
                self.fade = 1.0;
                if let Some(samples) = self.pending.take() {
                    self.set_length(samples);
                }
            }
        }
        let filtered = self.highpass.process(self.lowpass.process(out).lp).hp;
        let looped = soft_clip(filtered * self.feedback);
        if let Some(slot) = self.buffer.get_mut(self.write) {
            *slot = input + looped;
        }
        self.write += 1;
        if self.write >= self.buffer.len() {
            self.write = 0;
        }
        out
    }
}

/// A fixed-length delay line (reverb building block).
#[derive(Debug, Clone)]
struct Line {
    buffer: Vec<f32>,
    pos: usize,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            pos: 0,
        }
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    /// The sample written `len` samples ago, then stores `input`.
    fn tick(&mut self, input: f32) -> f32 {
        let out = self.buffer.get(self.pos).copied().unwrap_or(0.0);
        if let Some(slot) = self.buffer.get_mut(self.pos) {
            *slot = input;
        }
        self.pos += 1;
        if self.pos >= self.buffer.len() {
            self.pos = 0;
        }
        out
    }
}

/// Schroeder allpass: smears the input into a dense onset before the network.
#[derive(Debug, Clone)]
struct Allpass {
    line: Line,
}

const DIFFUSION: f32 = 0.6;

impl Allpass {
    fn tick(&mut self, input: f32) -> f32 {
        let delayed = self.line.buffer.get(self.line.pos).copied().unwrap_or(0.0);
        let v = input + DIFFUSION * delayed;
        self.line.tick(v);
        delayed - DIFFUSION * v
    }
}

/// Lengths of the network lines in ms, mutually prime-ish (no ringing modes).
const LINE_MS: [f32; 8] = [29.7, 37.1, 41.1, 43.7, 53.0, 59.9, 67.7, 73.1];
const DIFFUSER_MS: [f32; 2] = [4.7, 3.6];
/// Signs of each line in the left and right outputs: decorrelated, so the tail is wide.
const LEFT: [f32; 8] = [1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0];
const RIGHT: [f32; 8] = [1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const DEFAULT_DECAY_SECONDS: f32 = 2.5;
const DAMPING_HZ: f32 = 5_500.0;

/// 8-line feedback delay network, Hadamard mixing, damped lines; mono in, stereo out.
#[derive(Debug, Clone)]
pub struct Reverb {
    sample_rate: f32,
    lines: [Line; 8],
    gains: [f32; 8],
    damp_state: [f32; 8],
    damp_coef: f32,
    diffusers: [Allpass; 2],
    decay: f32,
}

impl Reverb {
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let samples = |ms: f32| (ms * 0.001 * sr).round() as usize;
        let mut reverb = Self {
            sample_rate: sr,
            lines: LINE_MS.map(|ms| Line::new(samples(ms))),
            gains: [0.0; 8],
            damp_state: [0.0; 8],
            damp_coef: 1.0 - (-std::f32::consts::TAU * DAMPING_HZ / sr).exp(),
            diffusers: DIFFUSER_MS.map(|ms| Allpass {
                line: Line::new(samples(ms)),
            }),
            decay: 0.0,
        };
        reverb.set_decay(DEFAULT_DECAY_SECONDS);
        reverb
    }

    /// Time for the tail to fall by 60 dB (RT60), in seconds.
    pub fn set_decay(&mut self, seconds: f32) {
        if !seconds.is_finite() {
            return;
        }
        let seconds = seconds.clamp(0.3, 8.0);
        self.decay = seconds;
        let sr = self.sample_rate;
        for (gain, line) in self.gains.iter_mut().zip(self.lines.iter()) {
            *gain = 10.0_f32.powf(-3.0 * line.len() as f32 / (seconds * sr));
        }
    }

    #[must_use]
    pub fn decay(&self) -> f32 {
        self.decay
    }

    /// One sample in, (left, right) out.
    pub fn tick(&mut self, input: f32) -> (f32, f32) {
        let mut x = input;
        for diffuser in &mut self.diffusers {
            x = diffuser.tick(x);
        }
        let mut outs = [0.0_f32; 8];
        for ((out, line), state) in outs
            .iter_mut()
            .zip(self.lines.iter())
            .zip(self.damp_state.iter_mut())
        {
            let delayed = line.buffer.get(line.pos).copied().unwrap_or(0.0);
            *state += (delayed - *state) * self.damp_coef;
            *out = *state;
        }
        let mixed = hadamard(outs);
        for (((line, &m), &gain), sign) in self
            .lines
            .iter_mut()
            .zip(mixed.iter())
            .zip(self.gains.iter())
            .zip(LEFT.iter())
        {
            line.tick(x * sign + m * gain);
        }
        let (mut left, mut right) = (0.0, 0.0);
        for ((&out, &l), &r) in outs.iter().zip(LEFT.iter()).zip(RIGHT.iter()) {
            left += out * l;
            right += out * r;
        }
        (left * 0.35, right * 0.35)
    }
}

/// Normalized 8-point Hadamard transform (energy preserving, so the gains alone set the decay).
fn hadamard(a: [f32; 8]) -> [f32; 8] {
    let [a0, a1, a2, a3, a4, a5, a6, a7] = a;
    let (b0, b1, b2, b3) = (a0 + a1, a0 - a1, a2 + a3, a2 - a3);
    let (b4, b5, b6, b7) = (a4 + a5, a4 - a5, a6 + a7, a6 - a7);
    let (c0, c1, c2, c3) = (b0 + b2, b1 + b3, b0 - b2, b1 - b3);
    let (c4, c5, c6, c7) = (b4 + b6, b5 + b7, b4 - b6, b5 - b7);
    let s = std::f32::consts::FRAC_1_SQRT_2 * 0.5;
    [
        (c0 + c4) * s,
        (c1 + c5) * s,
        (c2 + c6) * s,
        (c3 + c7) * s,
        (c0 - c4) * s,
        (c1 - c5) * s,
        (c2 - c6) * s,
        (c3 - c7) * s,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn delay_repeats_an_impulse_after_its_length() {
        let mut delay = Delay::new(SR);
        delay.set_length(1_000.0);
        // Let the crossfade to the new head end on silence.
        for _ in 0..4_000 {
            delay.tick(0.0);
        }
        let out: Vec<f32> = (0..3_000)
            .map(|i| delay.tick(if i == 0 { 1.0 } else { 0.0 }))
            .collect();
        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(i, _)| i);
        assert_eq!(peak, Some(1_000));
        // No feedback: a single repeat.
        assert!(out.iter().skip(1_010).all(|s| s.abs() < 1e-3));
    }

    #[test]
    fn feedback_repeats_decay_and_stay_bounded() {
        let mut delay = Delay::new(SR);
        delay.set_length(2_400.0);
        delay.set_feedback(1.0);
        for _ in 0..4_000 {
            delay.tick(0.0);
        }
        let mut peak_late: f32 = 0.0;
        for i in 0..(SR as usize * 20) {
            let s = delay.tick(if i < 2_400 { 0.9 } else { 0.0 });
            assert!(s.is_finite() && s.abs() < 2.0);
            if i > SR as usize * 18 {
                peak_late = peak_late.max(s.abs());
            }
        }
        // 0.95 with the loop filters: the tail is gone after 20 s.
        assert!(peak_late < 0.05, "{peak_late}");
    }

    #[test]
    fn time_change_crossfades_without_a_jump() {
        let mut delay = Delay::new(SR);
        delay.set_length(1_000.0);
        // A steady tone fills the line, then the time changes mid-signal.
        let tone = |i: usize| (i as f32 * 0.01).sin() * 0.5;
        let mut last = 0.0;
        let mut biggest_step: f32 = 0.0;
        for i in 0..20_000 {
            if i == 10_000 {
                delay.set_length(1_700.0);
            }
            let s = delay.tick(tone(i));
            if i > 6_000 {
                biggest_step = biggest_step.max((s - last).abs());
            }
            last = s;
        }
        // A sine at 0.01 rad/sample moves by ≤ 0.005 per sample; a jump would be ~0.5.
        assert!(biggest_step < 0.02, "{biggest_step}");
        assert!((delay.length() - 1_700.0).abs() < 1e-3);
    }

    #[test]
    fn delay_time_scroll_wraps() {
        assert_eq!(DelayTime::Sixteenth.next(), DelayTime::DottedEighth);
        assert_eq!(DelayTime::from_index(3), DelayTime::DottedQuarter);
        assert!((DelayTime::DottedEighth.beats() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn reverb_tail_is_stereo_and_follows_its_decay() {
        let mut reverb = Reverb::new(SR);
        reverb.set_decay(1.0);
        let mut energy_early = 0.0;
        let mut energy_late = 0.0;
        let mut difference = 0.0;
        for i in 0..(SR as usize * 2) {
            let (l, r) = reverb.tick(if i == 0 { 1.0 } else { 0.0 });
            assert!(l.is_finite() && r.is_finite());
            if (4_800..14_400).contains(&i) {
                energy_early += l * l + r * r;
                difference += (l - r) * (l - r);
            }
            if (52_800..62_400).contains(&i) {
                energy_late += l * l + r * r;
            }
        }
        assert!(energy_early > 0.0);
        // Left and right differ (a wide tail, not mono).
        assert!(
            difference > energy_early * 0.3,
            "{difference} {energy_early}"
        );
        // One second later, about -60 dB (damping makes it a bit faster): well below -40 dB.
        let drop_db = 10.0 * (energy_late / energy_early).log10();
        assert!(drop_db < -40.0, "{drop_db}");
    }

    #[test]
    fn reverb_stays_bounded_on_loud_noise() {
        let mut reverb = Reverb::new(SR);
        reverb.set_decay(8.0);
        let mut noise = crate::dsp::Noise::new(7);
        for _ in 0..(SR as usize * 5) {
            let (l, r) = reverb.tick(noise.sample());
            assert!(l.is_finite() && r.is_finite() && l.abs() < 20.0 && r.abs() < 20.0);
        }
    }
}
