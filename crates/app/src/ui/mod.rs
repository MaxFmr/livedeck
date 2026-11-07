//! The window: the D4 layout painted with the D5 tokens (`theme`). 020 is the shell: only the time
//! bar is live, the deck panels are empty frames until the D6 widgets.
//!
//! Runs on: main thread (the macOS event loop). Reads only `View`, which the control thread
//! publishes; writes nothing back except the quit request when the window closes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Key, Painter,
    Pos2, Rect, Stroke, Vec2, ViewportCommand,
};

use crate::theme::{self, Accent, Rgb, TextStyle};

mod melody;
mod rhythm;

/// A pad as its LED shows it on the controller (the window mirrors the hardware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Light {
    #[default]
    Off,
    On,
    /// Blinking on the controller: waiting for the bar or the beat.
    Wait,
}

/// Where a gesture's HUD shows: over the deck of the control, or in the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    A,
    Center,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HudValue {
    /// 0..=1, shown in %.
    Percent(f32),
    Bpm(f64),
}

/// The control just touched: its name, value and, waiting for pickup, the value to reach.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hud {
    pub zone: Zone,
    pub label: &'static str,
    pub value: HudValue,
    pub pickup: Option<HudValue>,
}

/// One step of the bass line as drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BassCell {
    #[default]
    Rest,
    Note {
        degree: u8,
        accent: bool,
        glide: bool,
    },
    /// The previous note still sounds.
    Hold,
}

/// What the window shows, copied from the engine snapshot and the control thread.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct View {
    pub playing: bool,
    pub bpm: f64,
    /// Beats since the transport started (fractional).
    pub beats: f64,
    pub tonic: u8,
    pub scale: u8,
    /// Tonic or scale waiting for the bar (shown after an arrow).
    pub pending_key: Option<(u8, u8)>,
    pub style: u8,
    pub pending_style: Option<u8>,
    pub swing: f32,
    pub controller: bool,
    /// Max callback load of the last report, 0..=1.
    pub load: f32,
    /// An xrun in the last 10 s.
    pub xrun_recent: bool,
    /// Step of the bar (0..16) the sequencer last played.
    pub step: u8,
    pub pattern: u8,
    pub pending_pattern: Option<u8>,
    /// Pattern heard, per voice (`DrumVoice::ALL`) × 16 steps: 0 none, 1 ghost, 2 hit, 3 accent.
    pub grid: [[u8; 16]; 6],
    pub mutes: u8,
    pub density: f32,
    pub fill: Option<(u8, bool)>,
    pub beat_repeat: bool,
    pub bass_line: u8,
    pub pending_bass_line: Option<u8>,
    pub bass_muted: bool,
    pub bass: [BassCell; 16],
    pub bass_cutoff: f32,
    /// Pad LEDs, `[deck][mode][pad]` (modes in `PadMode::ALL` order), and the mode each deck shows.
    pub pads: [[[Light; 4]; 4]; 2],
    pub shown_mode: [u8; 2],
    /// Chord pads sounding (bit `i` = pad `i`: I, IV, V, vi, ii, iii, vii, I').
    pub chords: u8,
    pub transpose: i8,
    pub octave: i8,
    pub inversion: i8,
    /// Index of `chords::Extension::ALL`.
    pub extension: u8,
    pub latch: bool,
    pub arp_mode: u8,
    pub arp_rate: u8,
    pub arp_octaves: u8,
    pub phrase: Option<(u8, bool)>,
    pub pending_phrase: Option<Option<(u8, bool)>>,
    /// Chord pads play single notes; note pads held (bit `i` = pad `i`).
    pub note_mode: bool,
    pub notes_held: u8,
    /// Loop slots (`looper::LoopState::index`), their lengths, the next length, and the progress
    /// (step, steps) of the loop playing or being recorded.
    pub loops: [u8; 4],
    pub loop_bars: [u8; 4],
    pub next_loop_bars: u8,
    pub loop_progress: Option<(u16, u16)>,
    pub delay_time: u8,
    /// The HUD of the last gesture, while it is shown.
    pub hud: Option<Hud>,
    /// Meters 0..=1 (dB scale): drums, bass, melody, master; the limiter working.
    pub meters: [f32; 4],
    pub limiting: bool,
    /// Callback load over 70 % at the last report.
    pub overload: bool,
}

/// The control thread → window link.
#[derive(Default)]
pub struct Shared {
    view: Mutex<View>,
    quit: AtomicBool,
    /// Frames painted since start (the control thread reports the rate).
    frames: AtomicU64,
}

impl Shared {
    /// Stores `view`; `true` if the window must be woken (the caller asks for a repaint): any
    /// change, except while it was already playing (it then repaints at its own pace).
    pub fn publish(&self, view: View) -> bool {
        let mut slot = self.view.lock().unwrap_or_else(PoisonError::into_inner);
        let wake = *slot != view && !(slot.playing && view.playing);
        *slot = view;
        wake
    }

    fn view(&self) -> View {
        *self.view.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The window was closed: the control thread ends the session.
    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::Relaxed);
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Ordering::Relaxed)
    }

    pub fn quit_requested(&self) -> bool {
        self.quit.load(Ordering::Relaxed)
    }
}

/// While playing the window follows the beat at 30 fps (023: half the CPU of 60 fps, a 1/16 at
/// 200 BPM still lasts two frames); stopped, it repaints only when the control
/// thread publishes a change.
pub const FRAME: Duration = Duration::from_millis(33);

pub struct Window {
    shared: Arc<Shared>,
}

impl Window {
    pub fn new(ctx: &egui::Context, shared: Arc<Shared>) -> Self {
        install_fonts(ctx);
        Self { shared }
    }
}

const MEDIUM: &str = "inter-medium";
const BOLD: &str = "inter-bold";

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(
        MEDIUM.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../../assets/fonts/Inter-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        BOLD.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../../assets/fonts/Inter-Bold.ttf"
        ))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.insert(family, vec![MEDIUM.to_owned()]);
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), vec![BOLD.to_owned()]);
    ctx.set_fonts(fonts);
}

/// Saves the next frame of the window as a BMP (development: check a layout without a screen
/// recorder; `sips -s format png` converts it).
pub fn capture(ctx: &egui::Context, path: std::path::PathBuf) {
    ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
        move |image| match std::fs::write(&path, bmp(&image)) {
            Ok(()) => println!("capture : {}", path.display()),
            Err(e) => println!("capture non écrite : {e}"),
        },
    )));
    ctx.request_repaint();
}

/// 32-bit top-down BMP of an image.
fn bmp(image: &egui::ColorImage) -> Vec<u8> {
    let [w, h] = image.size;
    let data = w * h * 4;
    let mut out = Vec::with_capacity(54 + data);
    let u32le = |out: &mut Vec<u8>, v: usize| out.extend_from_slice(&(v as u32).to_le_bytes());
    out.extend_from_slice(b"BM");
    u32le(&mut out, 54 + data);
    u32le(&mut out, 0);
    u32le(&mut out, 54);
    u32le(&mut out, 40);
    u32le(&mut out, w);
    // Negative height: rows top to bottom.
    out.extend_from_slice(&(-(h as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    for v in [0, data, 2835, 2835, 0, 0] {
        u32le(&mut out, v);
    }
    for px in &image.pixels {
        out.extend_from_slice(&[px.b(), px.g(), px.r(), px.a()]);
    }
    out
}

pub(super) fn radius(r: f32) -> CornerRadius {
    CornerRadius::same(r as u8)
}

/// Lit phase of a pending item: an eighth note on, an eighth off, at the tempo; steady when
/// stopped (nothing moves).
pub(super) fn pending_phase(v: &View) -> bool {
    !v.playing || ((v.beats.max(0.0) / theme::BLINK_BEATS) as u64).is_multiple_of(2)
}

/// "dans 3 temps" before the next bar ("au démarrage" when stopped).
pub(super) fn until_bar(v: &View) -> String {
    if !v.playing {
        return "au démarrage".to_owned();
    }
    let left = engine::BEATS_PER_BAR - (v.beats.max(0.0) as u64 % engine::BEATS_PER_BAR);
    format!("dans {left} temps")
}

/// Dashed outline of a pending item (`STROKE_STATE`, `DASH`).
pub(super) fn dashed_rect(p: &Painter, r: Rect, c: Rgb) {
    let r = r.shrink(theme::STROKE_STATE / 2.0);
    let corners = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    p.extend(egui::Shape::dashed_line(
        &corners,
        Stroke::new(theme::STROKE_STATE, color(c)),
        theme::DASH[0],
        theme::DASH[1],
    ));
}

pub(super) fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c.0, c.1, c.2)
}

fn font(style: TextStyle) -> FontId {
    let family = if style.weight >= 600 {
        FontFamily::Name(BOLD.into())
    } else {
        FontFamily::Proportional
    };
    FontId::new(style.size, family)
}

pub(super) fn layout(p: &Painter, s: &str, style: TextStyle, c: Rgb) -> Arc<egui::Galley> {
    let s = if style.caps {
        s.to_uppercase()
    } else {
        s.to_owned()
    };
    let mut job = LayoutJob::default();
    job.append(
        &s,
        0.0,
        TextFormat {
            font_id: font(style),
            extra_letter_spacing: style.tracking * style.size,
            color: color(c),
            ..TextFormat::default()
        },
    );
    p.layout_job(job)
}

/// Text in a D5 style, placed by `anchor` at `pos`; returns its rectangle.
pub(super) fn text(
    p: &Painter,
    pos: Pos2,
    anchor: Align2,
    s: &str,
    style: TextStyle,
    c: Rgb,
) -> Rect {
    let galley = layout(p, s, style, c);
    let rect = anchor.anchor_size(pos, galley.size());
    p.galley(rect.min, galley, color(c));
    rect
}

/// Digits in fixed cells (Inter's figures are proportional and egui has no `tnum`), so the BPM
/// does not shift as it changes. `pos` is the left center.
fn tabular(p: &Painter, pos: Pos2, s: &str, style: TextStyle, c: Rgb) -> Rect {
    let cell = p
        .layout_no_wrap("0".to_owned(), font(style), color(c))
        .size()
        .x;
    let mut x = pos.x;
    let mut rect = Rect::from_min_size(pos, Vec2::ZERO);
    for ch in s.chars() {
        let advance = if ch.is_ascii_digit() {
            cell
        } else {
            p.layout_no_wrap(ch.to_string(), font(style), color(c))
                .size()
                .x
        };
        let r = text(
            p,
            Pos2::new(x + advance / 2.0, pos.y),
            Align2::CENTER_CENTER,
            &ch.to_string(),
            style,
            c,
        );
        rect = rect.union(r);
        x += advance;
    }
    rect
}

fn panel(p: &Painter, rect: Rect, accent: Option<Accent>) {
    p.rect_filled(
        rect,
        CornerRadius::same(theme::RADIUS_PANEL as u8),
        color(theme::SURFACE),
    );
    if let Some(a) = accent {
        let band = Rect::from_min_size(rect.min, Vec2::new(rect.width(), theme::STROKE_DECK));
        p.rect_filled(
            band,
            CornerRadius {
                nw: theme::RADIUS_PANEL as u8,
                ne: theme::RADIUS_PANEL as u8,
                sw: 0,
                se: 0,
            },
            color(a.base),
        );
    }
}

/// Footer: the meters (drums, bass, melody, master) and the limiter; scenes come in phase 6.
fn footer_bar(p: &Painter, rect: Rect, v: &View) {
    panel(p, rect, None);
    let mid = rect.center().y;
    text(
        p,
        Pos2::new(rect.left() + theme::SPACE_5, mid),
        Align2::LEFT_CENTER,
        "F : plein écran",
        theme::BODY,
        theme::TEXT_DIM,
    );
    let meters = [
        ("Batterie", theme::ACCENT_WARM.base),
        ("Basse", theme::ACCENT_AMBER.base),
        ("Mélodie", theme::ACCENT_COLD.base),
        ("Master", theme::TEXT),
    ];
    let mut x = rect.right() - theme::SPACE_5;
    // The limiter, rightmost: lit while it reduces the gain.
    let lim = text(
        p,
        Pos2::new(x, mid),
        Align2::RIGHT_CENTER,
        "Limiteur",
        theme::LABEL,
        if v.limiting { theme::TEXT } else { theme::MARK },
    );
    let dot = Rect::from_center_size(
        Pos2::new(lim.left() - theme::SPACE_2 - 4.0, mid),
        Vec2::splat(8.0),
    );
    p.rect_filled(
        dot,
        radius(theme::RADIUS_CELL),
        color(if v.limiting { theme::TEXT } else { theme::LINE }),
    );
    x = dot.left() - theme::SPACE_6;
    for ((name, c), &level) in meters.iter().zip(v.meters.iter()).rev() {
        let track = Rect::from_min_size(
            Pos2::new(x - 96.0, mid - theme::SIZE_BAR / 2.0),
            Vec2::new(96.0, theme::SIZE_BAR),
        );
        let pill = radius(theme::SIZE_BAR / 2.0);
        p.rect_filled(track, pill, color(theme::LINE));
        let filled =
            Rect::from_min_size(track.min, Vec2::new(track.width() * level, track.height()));
        p.rect_filled(filled, pill, color(*c));
        let label = text(
            p,
            Pos2::new(track.left() - theme::SPACE_2, mid),
            Align2::RIGHT_CENTER,
            name,
            theme::BODY,
            theme::TEXT_DIM,
        );
        x = label.left() - theme::SPACE_5;
    }
}

fn time_bar(p: &Painter, rect: Rect, v: &View) {
    panel(p, rect, None);
    let mid = rect.center().y;
    let mut x = rect.left() + theme::SPACE_5;
    // Play / stop.
    let icon = Rect::from_center_size(Pos2::new(x + 11.0, mid), Vec2::splat(22.0));
    if v.playing {
        let pts = vec![
            Pos2::new(icon.left() + 3.0, icon.top() + 2.0),
            Pos2::new(icon.right() - 2.0, mid),
            Pos2::new(icon.left() + 3.0, icon.bottom() - 2.0),
        ];
        p.add(egui::Shape::convex_polygon(
            pts,
            color(theme::TEXT),
            Stroke::NONE,
        ));
    } else {
        p.rect_filled(icon.shrink(4.0), 0, color(theme::TEXT_DIM));
    }
    x += 22.0 + theme::SPACE_3;
    let bpm = tabular(
        p,
        Pos2::new(x, mid),
        &format!("{:.0}", v.bpm),
        theme::DISPLAY,
        theme::TEXT,
    );
    text(
        p,
        Pos2::new(bpm.right() + theme::SPACE_2, mid + 4.0),
        Align2::LEFT_CENTER,
        "BPM",
        theme::LABEL,
        theme::TEXT_DIM,
    );
    x = rect.left() + 210.0;
    // Four beats: the current one lit while playing, the first a landmark.
    let beat = (v.beats.max(0.0) as u64 % engine::BEATS_PER_BAR) as usize;
    for i in 0..4 {
        let r = Rect::from_min_size(Pos2::new(x, mid - 14.0), Vec2::splat(28.0));
        let fill = if v.playing && i == beat {
            theme::TEXT
        } else if i == 0 {
            theme::MARK
        } else {
            theme::LINE
        };
        p.rect_filled(r, CornerRadius::same(theme::RADIUS_PAD as u8), color(fill));
        x += 28.0 + theme::SPACE_2;
    }
    x += theme::SPACE_4;
    let bar = (v.beats.max(0.0) / engine::BEATS_PER_BAR as f64) as u64;
    tabular(
        p,
        Pos2::new(x, mid - 10.0),
        &format!("Mesure {}", bar + 1),
        theme::BODY,
        theme::TEXT,
    );
    tabular(
        p,
        Pos2::new(x, mid + 12.0),
        &format!("Phrase {} / 8", bar % 8 + 1),
        theme::BODY,
        theme::TEXT_DIM,
    );
    x += 150.0;
    let key = |tonic: u8, scale: u8| {
        format!(
            "{} {}",
            engine::bass::tonic_name(tonic),
            engine::bass::Scale::from_index(scale).map_or("?", engine::bass::Scale::name)
        )
    };
    let key_rect = text(
        p,
        Pos2::new(x, mid - 9.0),
        Align2::LEFT_CENTER,
        &key(v.tonic, v.scale),
        theme::TITLE,
        theme::TEXT,
    );
    if let Some((tonic, scale)) = v.pending_key {
        text(
            p,
            Pos2::new(key_rect.right() + theme::SPACE_3, mid - 9.0),
            Align2::LEFT_CENTER,
            &format!("→ {} à la mesure", key(tonic, scale)),
            theme::BODY,
            theme::TEXT_DIM,
        );
    }
    let style = match v.pending_style {
        Some(next) => format!(
            "{} → {} à la mesure · swing {:.0} %",
            engine::style::name(v.style),
            engine::style::name(next),
            v.swing * 100.0
        ),
        None => format!(
            "{} · swing {:.0} %",
            engine::style::name(v.style),
            v.swing * 100.0
        ),
    };
    text(
        p,
        Pos2::new(x, mid + 14.0),
        Align2::LEFT_CENTER,
        &style,
        theme::BODY,
        theme::TEXT_DIM,
    );
    // Health, at the right: an alert takes the place of the quiet status.
    let right = Pos2::new(rect.right() - theme::SPACE_5, mid);
    let alert = if !v.controller {
        Some("Contrôleur absent")
    } else if v.xrun_recent {
        Some("Xrun")
    } else if v.overload {
        Some("Surcharge CPU")
    } else {
        None
    };
    match alert {
        Some(label) => {
            let style = TextStyle {
                caps: true,
                weight: 700,
                ..theme::BODY
            };
            let pad = Vec2::new(14.0, 8.0);
            let size = layout(p, label, style, theme::INK).size() + 2.0 * pad;
            let badge = Align2::RIGHT_CENTER.anchor_size(right, size);
            p.rect_filled(
                badge,
                CornerRadius::same(theme::RADIUS_PAD as u8),
                color(theme::ACCENT_ALERT.base),
            );
            text(
                p,
                badge.center(),
                Align2::CENTER_CENTER,
                label,
                style,
                theme::INK,
            );
        }
        None => {
            let status = text(
                p,
                right,
                Align2::RIGHT_CENTER,
                &format!("DJControl Mix · charge {:.0} %", v.load * 100.0),
                theme::BODY,
                theme::TEXT_DIM,
            );
            p.circle_filled(
                Pos2::new(status.left() - theme::SPACE_2 - 4.0, mid),
                4.0,
                color(theme::OK),
            );
        }
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) {
            self.shared.request_quit();
        }
        if ctx.input(|i| i.key_pressed(Key::F)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
        }
        self.shared.frames.fetch_add(1, Ordering::Relaxed);
        let v = self.shared.view();
        let p = ui.painter();
        let full = ui.max_rect();
        p.rect_filled(full, 0, color(theme::BG));
        // D4 layout: time bar, three columns (A · bass · B), footer.
        let inner = full.shrink2(Vec2::new(theme::SPACE_6, theme::SPACE_5));
        let gap = theme::SPACE_4;
        let header = Rect::from_min_size(inner.min, Vec2::new(inner.width(), theme::SIZE_HEADER));
        let footer = Rect::from_min_max(
            Pos2::new(inner.left(), inner.bottom() - theme::SIZE_FOOTER),
            inner.max,
        );
        let main = Rect::from_min_max(
            Pos2::new(inner.left(), header.bottom() + gap),
            Pos2::new(inner.right(), footer.top() - gap),
        );
        let bass_w = 196.0;
        let deck_w = (main.width() - bass_w - 2.0 * gap) / 2.0;
        let deck_a = Rect::from_min_size(main.min, Vec2::new(deck_w, main.height()));
        let bass = Rect::from_min_size(
            Pos2::new(deck_a.right() + gap, main.top()),
            Vec2::new(bass_w, main.height()),
        );
        let deck_b = Rect::from_min_max(Pos2::new(bass.right() + gap, main.top()), main.max);
        time_bar(p, header, &v);
        panel(p, deck_a, Some(theme::ACCENT_WARM));
        rhythm::deck_a(p, deck_a, &v);
        panel(p, bass, Some(theme::ACCENT_AMBER));
        rhythm::bass_column(p, bass, &v);
        panel(p, deck_b, Some(theme::ACCENT_COLD));
        melody::deck_b(p, deck_b, &v);
        footer_bar(p, footer, &v);
        if let Some(h) = &v.hud {
            melody::hud(
                p,
                [deck_a, Rect::from_min_max(deck_a.min, deck_b.max), deck_b],
                h,
            );
        }
        if v.playing {
            ctx.request_repaint_after(FRAME);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_restart_wakes_the_window() {
        let shared = Shared::default();
        let stopped = View::default();
        let playing = View {
            playing: true,
            ..View::default()
        };
        assert!(!shared.publish(stopped), "nothing changed");
        assert!(shared.publish(playing), "play after stop");
        let later = View {
            beats: 1.0,
            ..playing
        };
        assert!(!shared.publish(later), "playing: the window paces itself");
        assert!(shared.publish(stopped), "stop");
    }
}
