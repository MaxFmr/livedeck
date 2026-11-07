//! The groovebox window (R5, maquette D8 « écran central ») : a deck panel on each side — four
//! rings named after the control that sets them, the pad mode the controller shows and its four
//! pads as their LEDs show them — and, in the middle, the screen: rhythm, bass and phrase under one
//! playhead, what changes at the bar, the mixer and the health of the instrument.

use egui::{Align2, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

use super::{
    Hud, HudValue, Light, View, Zone, color, dashed_rect, layout, pending_phase, radius, tabular,
    text, until_bar,
};
use crate::theme::{self, Accent, Rgb};

const MODES: [&str; 4] = ["Hot Cue", "Sampler", "FX", "Loop"];

/// Ring names and the control that sets them: `[deck][shift]`, in `View::knobs` order.
const RINGS: [[[(&str, &str); 4]; 2]; 2] = [
    [
        [
            ("Coupure", "Filter A"),
            ("Résonance", "Bass A"),
            ("Decay", "Tempo A"),
            ("Rythme", "Volume A"),
        ],
        [
            ("Coupure", "Filter A"),
            ("Env.", "+ Bass A"),
            ("Note", "+ Tempo A"),
            ("Basse", "+ Volume A"),
        ],
    ],
    [
        [
            ("Densité", "Tempo B"),
            ("Brillance", "Filter B"),
            ("Delay", "Bass B"),
            ("Mélodie", "Volume B"),
        ],
        [
            ("Densité", "Tempo B"),
            ("Brillance", "Filter B"),
            ("Répét.", "+ Bass B"),
            ("Métro.", "+ Volume B"),
        ],
    ],
];

/// Pad labels and their role: `[deck][mode][shift]`.
type PadLabels = ([&'static str; 4], [&'static str; 4]);
const PADS: [[[PadLabels; 2]; 4]; 2] = [
    [
        [
            (["1", "2", "3", "4"], ["PATTERN"; 4]),
            (["L1", "L2", "L3", "L4"], ["LIGNE BASSE"; 4]),
        ],
        [
            (["F1", "F2", "F3", "F4"], ["REMPLISSAGE"; 4]),
            (
                ["808", "909", "Dub", "Hasard"],
                ["KIT", "KIT", "KIT", "LIGNE BASSE"],
            ),
        ],
        [
            (["Kick", "Rim", "Hats", "Perc"], ["MUTE"; 4]),
            (["Kick", "Rim", "Hats", "Perc"], ["MUTE"; 4]),
        ],
        [
            (["1/4", "1/8", "1/16", "1/32"], ["REPEAT TENU"; 4]),
            (["50", "54", "58", "62"], ["SWING %"; 4]),
        ],
    ],
    [
        [
            (["I", "IV", "V", "vi"], ["ACCORD"; 4]),
            (["ii", "iii", "vii", "I'"], ["ACCORD"; 4]),
        ],
        [
            (["Générer", "Muter", "Figer", "Effacer"], ["PHRASE"; 4]),
            (["Générer", "Muter", "Figer", "Effacer"], ["PHRASE"; 4]),
        ],
        [
            (
                ["Delay", "Réverb", "Temps", "Arp"],
                ["THROW TENU", "THROW TENU", "DELAY", "ARPÈGE"],
            ),
            (["1", "2", "3", "4"], ["OCTAVES ARP"; 4]),
        ],
        [
            (
                ["Rec", "Latch", "Oct.", "Ext."],
                ["TENU", "ACCORDS", "OCTAVE", "EXTENSION"],
            ),
            (
                ["Rec", "Renv.", "Oct. −", "Triade"],
                ["TENU", "RENVERSEMENT", "OCTAVE", "EXTENSION"],
            ),
        ],
    ],
];

const HINTS: [[&str; 4]; 2] = [
    [
        "Pattern à la mesure · Shift : basse",
        "Fill · Shift : kit, basse au hasard",
        "Allumé = la voix joue",
        "Beat-repeat tenu · Shift : swing",
    ],
    [
        "Accord tenu · Shift : 4 autres",
        "Générer, muter, effacer : à la mesure",
        "Throw tant que le pad est tenu",
        "Rec tenu : accords dans la phrase",
    ],
];

const JOGS: [&str; 2] = [
    "Jog : densité · Shift + jog : tempo",
    "Jog : transposition · Shift + jog : gamme",
];

fn label(p: &Painter, pos: Pos2, anchor: Align2, s: &str, c: Rgb) -> Rect {
    text(p, pos, anchor, s, theme::LABEL, c)
}

/// A ring: track, then the arc of `value` (none: the kit's sound, track only).
fn ring(p: &Painter, center: Pos2, r: f32, value: Option<f32>, c: Rgb) {
    let width = 6.0;
    p.circle_stroke(center, r, Stroke::new(width, color(theme::CELL)));
    let Some(v) = value else {
        return;
    };
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0 {
        return;
    }
    let n = (48.0 * v).ceil() as usize + 1;
    let start = -std::f32::consts::FRAC_PI_2;
    let points: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = start + std::f32::consts::TAU * v * i as f32 / n as f32;
            center + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect();
    p.add(Shape::line(points, Stroke::new(width, color(c))));
}

/// One deck: rings, pad mode, pads, hints.
pub(super) fn side(p: &Painter, rect: Rect, v: &View, deck: usize) {
    let accent = if deck == 0 {
        theme::ACCENT_WARM
    } else {
        theme::ACCENT_COLD
    };
    let shift = usize::from(v.shift);
    let inner = rect.shrink(theme::SPACE_5);
    let mut y = inner.top() + theme::STROKE_DECK;
    label(
        p,
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        if deck == 0 {
            "A · Rythme + basse"
        } else {
            "B · Accords + phrase"
        },
        accent.base,
    );
    if v.shift {
        let badge = text(
            p,
            Pos2::new(inner.right(), y),
            Align2::RIGHT_TOP,
            "SHIFT",
            theme::LABEL,
            theme::INK,
        );
        p.rect_filled(
            badge.expand(4.0),
            radius(theme::RADIUS_CELL),
            color(theme::TEXT),
        );
        text(
            p,
            badge.min,
            Align2::LEFT_TOP,
            "SHIFT",
            theme::LABEL,
            theme::INK,
        );
    }
    y += 32.0;

    // Rings, 2 × 2.
    let cell_w = inner.width() / 2.0;
    for (i, (name, ctl)) in RINGS
        .get(deck)
        .and_then(|d| d.get(shift))
        .copied()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let x = inner.left() + (i % 2) as f32 * cell_w;
        let top = y + (i / 2) as f32 * 76.0;
        let value = v.knobs.get(deck * 8 + shift * 4 + i).copied().flatten();
        let c = if deck == 0 && i < 3 {
            theme::ACCENT_AMBER.base
        } else {
            accent.base
        };
        ring(p, Pos2::new(x + 20.0, top + 26.0), 17.0, value, c);
        let shown = value.map_or("kit".to_owned(), |v| format!("{:.0} %", v * 100.0));
        text(
            p,
            Pos2::new(x + 46.0, top + 4.0),
            Align2::LEFT_TOP,
            name,
            theme::BODY,
            theme::TEXT,
        );
        text(
            p,
            Pos2::new(x + 46.0, top + 22.0),
            Align2::LEFT_TOP,
            &shown,
            theme::BODY,
            theme::TEXT,
        );
        text(
            p,
            Pos2::new(x + 46.0, top + 40.0),
            Align2::LEFT_TOP,
            ctl,
            theme::BODY,
            theme::TEXT_DIM,
        );
    }
    y += 2.0 * 76.0 + theme::SPACE_2;
    p.line_segment(
        [Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)],
        Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
    );
    y += theme::SPACE_4;

    // The pad mode the controller shows.
    let mode = usize::from(v.shown_mode.get(deck).copied().unwrap_or(0)).min(3);
    let tab_w = (inner.width() - 3.0 * theme::SPACE_1) / 4.0;
    for (i, name) in MODES.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(inner.left() + i as f32 * (tab_w + theme::SPACE_1), y),
            Vec2::new(tab_w, 30.0),
        );
        let on = i == mode;
        p.rect_filled(
            r,
            radius(theme::RADIUS_CELL),
            color(if on { accent.base } else { theme::CELL }),
        );
        text(
            p,
            r.center(),
            Align2::CENTER_CENTER,
            name,
            theme::BODY,
            if on { theme::INK } else { theme::TEXT_DIM },
        );
    }
    y += 30.0 + theme::SPACE_3;

    // Its four pads, 2 × 2, as the LEDs show them.
    let (names, roles) = PADS
        .get(deck)
        .and_then(|d| d.get(mode))
        .and_then(|m| m.get(shift))
        .copied()
        .unwrap_or((["", "", "", ""], ["", "", "", ""]));
    let pad_w = (inner.width() - theme::SPACE_3) / 2.0;
    let pad_h = 78.0;
    for i in 0..4 {
        let r = Rect::from_min_size(
            Pos2::new(
                inner.left() + (i % 2) as f32 * (pad_w + theme::SPACE_3),
                y + (i / 2) as f32 * (pad_h + theme::SPACE_3),
            ),
            Vec2::new(pad_w, pad_h),
        );
        let light = v
            .pads
            .get(deck)
            .and_then(|d| d.get(mode))
            .and_then(|m| m.get(i))
            .copied()
            .unwrap_or_default();
        pad(p, r, names[i % 4], roles[i % 4], light, accent, v);
    }
    y += 2.0 * pad_h + theme::SPACE_3 + theme::SPACE_3;
    text(
        p,
        Pos2::new(inner.left(), y),
        Align2::LEFT_TOP,
        HINTS
            .get(deck)
            .and_then(|h| h.get(mode))
            .copied()
            .unwrap_or(""),
        theme::BODY,
        theme::TEXT_DIM,
    );
    text(
        p,
        Pos2::new(inner.left(), inner.bottom()),
        Align2::LEFT_BOTTOM,
        JOGS.get(deck).copied().unwrap_or(""),
        theme::BODY,
        theme::TEXT_DIM,
    );
}

fn pad(p: &Painter, r: Rect, name: &str, role: &str, light: Light, accent: Accent, v: &View) {
    let (fill, fg) = match light {
        Light::On => (accent.base, theme::INK),
        Light::Wait => (accent.wait, theme::TEXT),
        Light::Off => (theme::CELL, theme::TEXT),
    };
    p.rect_filled(r, radius(theme::RADIUS_PAD), color(fill));
    if light == Light::Wait && pending_phase(v) {
        dashed_rect(p, r, accent.base);
    }
    let inner = r.shrink2(Vec2::new(theme::SPACE_3, theme::SPACE_2));
    text(
        p,
        inner.left_top(),
        Align2::LEFT_TOP,
        name,
        theme::TITLE,
        fg,
    );
    text(
        p,
        inner.left_bottom(),
        Align2::LEFT_BOTTOM,
        role,
        theme::LABEL,
        if light == Light::On {
            theme::INK
        } else {
            theme::TEXT_DIM
        },
    );
}

/// The centre: header, the three tracks, what changes at the bar, the mixer and the health.
pub(super) fn screen(p: &Painter, rect: Rect, v: &View) {
    p.rect_stroke(
        rect,
        radius(theme::RADIUS_PANEL),
        Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(theme::SPACE_6);
    let mut y = inner.top();
    header(
        p,
        Rect::from_min_size(inner.min, Vec2::new(inner.width(), 48.0)),
        v,
    );
    y += 48.0 + theme::SPACE_4;
    hair(p, inner, y);
    y += theme::SPACE_4;

    let lanes_w = inner.width();
    // Rhythm: four lanes.
    let fill = match v.fill {
        Some((_, true)) => " · remplissage",
        Some((_, false)) => " · remplissage à venir",
        None => "",
    };
    let repeat = if v.beat_repeat { " · beat-repeat" } else { "" };
    y = track_title(
        p,
        inner.left(),
        y,
        lanes_w,
        "Rythme",
        &format!(
            "pattern {} · densité {:.0} %{fill}{repeat}",
            v.pattern + 1,
            v.density * 100.0
        ),
        theme::ACCENT_WARM.base,
    );
    let drums = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(lanes_w, 80.0));
    rhythm_box(p, drums, v);
    y = drums.bottom() + theme::SPACE_4;

    // Bass.
    let bass_info = if v.bass_muted {
        "muette".to_owned()
    } else if v.pending_random_bass {
        format!("ligne {} → hasard à la mesure", v.bass_line + 1)
    } else {
        format!("ligne {}", v.bass_line + 1)
    };
    y = track_title(
        p,
        inner.left(),
        y,
        lanes_w,
        "Basse",
        &format!("{bass_info} · coupure {:.0} %", v.bass_cutoff * 100.0),
        theme::ACCENT_AMBER.base,
    );
    let bass = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(lanes_w, 72.0));
    bass_box(p, bass, v);
    y = bass.bottom() + theme::SPACE_4;

    // Phrase.
    let count = v.phrase_events.count;
    let state = if v.phrase_rec {
        "● rec"
    } else if count == 0 {
        "vide"
    } else if v.phrase_frozen {
        "figée"
    } else {
        "libre"
    };
    y = track_title(
        p,
        inner.left(),
        y,
        lanes_w,
        "Phrase",
        &format!(
            "{state} · {count} évén. · densité {:.0} % · transpo {:+}",
            v.phrase_density * 100.0,
            v.transpose
        ),
        theme::ACCENT_COLD.base,
    );
    let bottom_h = 84.0;
    let phrase_h = (inner.bottom() - bottom_h - 52.0 - y).max(80.0);
    let phrase = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(lanes_w, phrase_h));
    phrase_box(p, phrase, v);
    y = phrase.bottom() + theme::SPACE_4;

    at_the_bar(
        p,
        Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(lanes_w, 32.0)),
        v,
    );
    let mix = Rect::from_min_max(
        Pos2::new(inner.left(), inner.bottom() - bottom_h + 8.0),
        inner.max,
    );
    hair(p, inner, mix.top() - theme::SPACE_3);
    mixer(p, mix, v);
}

fn hair(p: &Painter, inner: Rect, y: f32) {
    p.line_segment(
        [Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)],
        Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
    );
}

fn track_title(p: &Painter, x: f32, y: f32, w: f32, name: &str, info: &str, c: Rgb) -> f32 {
    label(p, Pos2::new(x, y), Align2::LEFT_TOP, name, c);
    text(
        p,
        Pos2::new(x + w, y),
        Align2::RIGHT_TOP,
        info,
        theme::BODY,
        theme::TEXT_DIM,
    );
    y + 22.0
}

fn header(p: &Painter, rect: Rect, v: &View) {
    let mid = rect.center().y;
    let mut x = rect.left();
    let icon = Rect::from_center_size(Pos2::new(x + 11.0, mid), Vec2::splat(22.0));
    if v.playing {
        let pts = vec![
            Pos2::new(icon.left() + 3.0, icon.top() + 2.0),
            Pos2::new(icon.right() - 2.0, mid),
            Pos2::new(icon.left() + 3.0, icon.bottom() - 2.0),
        ];
        p.add(Shape::convex_polygon(pts, color(theme::TEXT), Stroke::NONE));
    } else {
        p.rect_filled(icon.shrink(4.0), 0, color(theme::TEXT_DIM));
    }
    x += 22.0 + theme::SPACE_4;
    let bpm = tabular(
        p,
        Pos2::new(x, mid),
        &format!("{:.0}", v.bpm),
        theme::DISPLAY,
        theme::TEXT,
    );
    let bpm_label = label(
        p,
        Pos2::new(bpm.right() + theme::SPACE_2, mid + 4.0),
        Align2::LEFT_CENTER,
        "BPM",
        theme::TEXT_DIM,
    );
    x = bpm_label.right() + theme::SPACE_5;
    let beats = v.beats.max(0.0);
    let bar = (beats / engine::BEATS_PER_BAR as f64) as u64 + 1;
    let beat = (beats as u64 % engine::BEATS_PER_BAR) as usize;
    let pos = tabular(
        p,
        Pos2::new(x, mid),
        &format!("{bar}.{}", beat + 1),
        theme::TITLE,
        theme::TEXT,
    );
    x = pos.right() + theme::SPACE_4;
    for i in 0..4 {
        let r = Rect::from_min_size(Pos2::new(x, mid - 7.0), Vec2::splat(14.0));
        let fill = if v.playing && i == beat {
            theme::TEXT
        } else {
            theme::LINE
        };
        p.rect_filled(r, radius(theme::RADIUS_CELL), color(fill));
        x += 14.0 + theme::SPACE_1;
    }
    let key = |tonic: u8, scale: u8| {
        format!(
            "{} {}",
            engine::bass::tonic_name(tonic),
            engine::bass::Scale::from_index(scale).map_or("?", engine::bass::Scale::name)
        )
    };
    text(
        p,
        Pos2::new(rect.right(), rect.top()),
        Align2::RIGHT_TOP,
        &key(v.tonic, v.scale),
        theme::TITLE,
        theme::TEXT,
    );
    text(
        p,
        Pos2::new(rect.right(), rect.bottom()),
        Align2::RIGHT_BOTTOM,
        &format!(
            "kit {} · swing {:.0} %",
            engine::style::name(v.style),
            v.swing * 100.0
        ),
        theme::BODY,
        theme::TEXT_DIM,
    );
}

/// A step lane geometry: x of step `s` and the step width.
fn step_x(r: Rect, s: f32) -> f32 {
    r.left() + r.width() * s / 16.0
}

fn lane_back(p: &Painter, r: Rect, v: &View) {
    p.rect_filled(r, radius(theme::RADIUS_PAD), color(theme::BG));
    for beat in [4.0, 8.0, 12.0] {
        let x = step_x(r, beat);
        p.line_segment(
            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
            Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
        );
    }
    if v.playing {
        let x = step_x(r, f32::from(v.step) + 0.5);
        p.line_segment(
            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
            Stroke::new(theme::STROKE_STATE, color(theme::TEXT)),
        );
    }
}

fn rhythm_box(p: &Painter, r: Rect, v: &View) {
    lane_back(p, r, v);
    // Lanes: kick · rim (snare + clap) · hats (closed + open) · perc; voices in DrumVoice::ALL.
    const LANES: [&[usize]; 4] = [&[0], &[1, 4], &[2, 3], &[5]];
    let lane_h = (r.height() - 10.0) / 4.0;
    let warm = theme::ACCENT_WARM;
    for (lane, voices) in LANES.iter().enumerate() {
        let muted = voices.iter().all(|&i| v.mutes & (1 << i) != 0);
        let top = r.top() + 5.0 + lane as f32 * lane_h;
        for s in 0..16 {
            let level = voices
                .iter()
                .filter_map(|&i| v.grid.get(i).and_then(|g| g.get(s)).copied())
                .max()
                .unwrap_or(0);
            if level == 0 {
                continue;
            }
            let c = if muted {
                theme::MARK
            } else if level >= 2 {
                if lane == 0 { warm.base } else { warm.dim }
            } else {
                warm.muted
            };
            let cell = Rect::from_min_max(
                Pos2::new(step_x(r, s as f32) + 2.0, top + 2.0),
                Pos2::new(step_x(r, s as f32 + 1.0) - 2.0, top + lane_h - 2.0),
            );
            p.rect_filled(cell, radius(theme::RADIUS_CELL), color(c));
        }
    }
}

fn bass_box(p: &Painter, r: Rect, v: &View) {
    lane_back(p, r, v);
    let amber = theme::ACCENT_AMBER;
    let rows = 8.0;
    let row_h = (r.height() - 8.0) / rows;
    let mut last: Option<(usize, u8)> = None;
    let mut spans: Vec<(usize, usize, u8, bool)> = Vec::new();
    for (s, cell) in v.bass.iter().enumerate() {
        match cell {
            super::BassCell::Note { degree, accent, .. } => {
                spans.push((s, s + 1, *degree, *accent));
                last = Some((spans.len() - 1, *degree));
            }
            super::BassCell::Hold => {
                if let Some((i, _)) = last
                    && let Some(span) = spans.get_mut(i)
                {
                    span.1 = s + 1;
                }
            }
            super::BassCell::Rest => last = None,
        }
    }
    for (from, to, degree, accent) in spans {
        let row = f32::from(degree.clamp(1, 8)) - 1.0;
        let top = r.bottom() - 4.0 - (row + 1.0) * row_h;
        let cell = Rect::from_min_max(
            Pos2::new(step_x(r, from as f32) + 2.0, top + 1.0),
            Pos2::new(step_x(r, to as f32) - 2.0, top + row_h - 1.0),
        );
        let c = if v.bass_muted {
            theme::MARK
        } else if accent {
            amber.bright
        } else {
            amber.base
        };
        p.rect_filled(cell, radius(theme::RADIUS_CELL), color(c));
    }
}

fn phrase_box(p: &Painter, r: Rect, v: &View) {
    lane_back(p, r, v);
    let cold = theme::ACCENT_COLD;
    let rows = 10.0;
    let row_h = (r.height() - 8.0) / rows;
    for e in v.phrase_events.iter() {
        let from = f32::from(e.start);
        let to = (from + f32::from(e.len.max(1))).min(16.0);
        if e.chord {
            // A chord: a tall, soft block across the lower half.
            let cell = Rect::from_min_max(
                Pos2::new(step_x(r, from) + 2.0, r.center().y),
                Pos2::new(step_x(r, to) - 2.0, r.bottom() - 4.0),
            );
            p.rect_filled(cell, radius(theme::RADIUS_CELL), color(cold.muted));
            p.rect_stroke(
                cell,
                radius(theme::RADIUS_CELL),
                Stroke::new(theme::STROKE_HAIR, color(cold.base)),
                StrokeKind::Inside,
            );
            continue;
        }
        let row = f32::from(e.degree.clamp(0, 9));
        let top = r.bottom() - 4.0 - (row + 1.0) * row_h;
        let cell = Rect::from_min_max(
            Pos2::new(step_x(r, from) + 2.0, top + 1.0),
            Pos2::new(step_x(r, to) - 2.0, top + row_h - 1.0),
        );
        let c = if e.accent { cold.bright } else { cold.base };
        p.rect_filled(cell, radius(theme::RADIUS_CELL), color(c));
    }
    if v.phrase_events.count == 0 {
        text(
            p,
            r.center(),
            Align2::CENTER_CENTER,
            "Sampler B · Générer",
            theme::BODY,
            theme::MARK,
        );
    }
}

/// What waits for the bar, as dashed chips.
fn at_the_bar(p: &Painter, rect: Rect, v: &View) {
    let mid = rect.center().y;
    let title = label(
        p,
        Pos2::new(rect.left(), mid),
        Align2::LEFT_CENTER,
        "À la mesure",
        theme::TEXT_DIM,
    );
    let mut chips: Vec<(String, Rgb)> = Vec::new();
    if let Some(next) = v.pending_pattern {
        chips.push((format!("pattern {}", next + 1), theme::ACCENT_WARM.base));
    }
    if let Some(next) = v.pending_style {
        chips.push((
            format!("kit {}", engine::style::name(next)),
            theme::ACCENT_WARM.base,
        ));
    }
    if let Some(next) = v.pending_bass_line {
        chips.push((format!("ligne {}", next + 1), theme::ACCENT_AMBER.base));
    }
    if v.pending_random_bass {
        chips.push(("basse au hasard".to_owned(), theme::ACCENT_AMBER.base));
    }
    if v.pending_key.is_some() {
        chips.push(("tonalité".to_owned(), theme::TEXT));
    }
    match engine::phrase::Pending::from_index(v.phrase_pending) {
        engine::phrase::Pending::Generate => {
            chips.push(("phrase générée".to_owned(), theme::ACCENT_COLD.base));
        }
        engine::phrase::Pending::Mutate => {
            chips.push(("phrase mutée".to_owned(), theme::ACCENT_COLD.base));
        }
        engine::phrase::Pending::Clear => {
            chips.push(("phrase effacée".to_owned(), theme::ACCENT_COLD.base));
        }
        engine::phrase::Pending::None => {}
    }
    let mut x = title.right() + theme::SPACE_3;
    if chips.is_empty() {
        text(
            p,
            Pos2::new(x, mid),
            Align2::LEFT_CENTER,
            "rien en attente",
            theme::BODY,
            theme::MARK,
        );
        return;
    }
    for (chip, c) in &chips {
        let size = layout(p, chip, theme::BODY, *c).size() + Vec2::new(20.0, 10.0);
        let r = Rect::from_min_size(Pos2::new(x, mid - size.y / 2.0), size);
        if pending_phase(v) {
            dashed_rect(p, r, *c);
        }
        text(p, r.center(), Align2::CENTER_CENTER, chip, theme::BODY, *c);
        x = r.right() + theme::SPACE_2;
    }
    text(
        p,
        Pos2::new(rect.right(), mid),
        Align2::RIGHT_CENTER,
        &until_bar(v),
        theme::BODY,
        theme::TEXT_DIM,
    );
}

/// Crossfader, master, reverb, throws, meters and the health of the instrument, in two rows laid
/// out left to right so nothing overlaps whatever the width.
fn mixer(p: &Painter, rect: Rect, v: &View) {
    let row1 = rect.top() + 10.0;
    let row2 = rect.top() + 58.0;
    // Crossfader.
    let cf_w = (rect.width() * 0.38).min(260.0);
    label(
        p,
        Pos2::new(rect.left(), row1 - 10.0),
        Align2::LEFT_TOP,
        "Rythme",
        theme::TEXT_DIM,
    );
    label(
        p,
        Pos2::new(rect.left() + cf_w, row1 - 10.0),
        Align2::RIGHT_TOP,
        "Mélodie",
        theme::TEXT_DIM,
    );
    let track = Rect::from_min_size(Pos2::new(rect.left(), row1 + 12.0), Vec2::new(cf_w, 6.0));
    p.rect_filled(track, radius(3.0), color(theme::CELL));
    let x = track.left() + track.width() * v.crossfade.clamp(0.0, 1.0);
    p.rect_filled(
        Rect::from_center_size(Pos2::new(x, track.center().y), Vec2::new(6.0, 18.0)),
        radius(theme::RADIUS_CELL),
        color(theme::TEXT),
    );
    // Master and reverb, after the crossfader.
    let mut x = rect.left() + cf_w + theme::SPACE_6;
    for (name, value) in [("Master", v.master), ("Réverb", v.reverb)] {
        ring(
            p,
            Pos2::new(x + 12.0, row1 + 14.0),
            11.0,
            Some(value),
            theme::TEXT,
        );
        let t = text(
            p,
            Pos2::new(x + 30.0, row1 + 14.0),
            Align2::LEFT_CENTER,
            &format!("{name} {:.0} %", value * 100.0),
            theme::BODY,
            theme::TEXT,
        );
        x = t.right() + theme::SPACE_5;
    }
    // Row 2: throws held, meters, limiter, then the health at the right.
    let mut x = rect.left();
    for (name, on) in [("Throw delay", v.throws.0), ("Throw réverb", v.throws.1)] {
        let size = layout(p, name, theme::LABEL, theme::INK).size() + Vec2::new(16.0, 10.0);
        let r = Rect::from_min_size(Pos2::new(x, row2 - size.y / 2.0), size);
        p.rect_filled(
            r,
            radius(theme::RADIUS_CELL),
            color(if on { theme::TEXT } else { theme::CELL }),
        );
        label(
            p,
            r.center(),
            Align2::CENTER_CENTER,
            name,
            if on { theme::INK } else { theme::TEXT_DIM },
        );
        x = r.right() + theme::SPACE_2;
    }
    x += theme::SPACE_3;
    let meters = [
        theme::ACCENT_WARM.base,
        theme::ACCENT_AMBER.base,
        theme::ACCENT_COLD.base,
        theme::TEXT,
    ];
    for (c, level) in meters.iter().zip(v.meters.iter()) {
        let r = Rect::from_min_size(Pos2::new(x, row2 - 3.0), Vec2::new(32.0, 6.0));
        p.rect_filled(r, radius(3.0), color(theme::LINE));
        p.rect_filled(
            Rect::from_min_size(r.min, Vec2::new(r.width() * level, r.height())),
            radius(3.0),
            color(*c),
        );
        x = r.right() + theme::SPACE_1;
    }
    p.circle_filled(
        Pos2::new(x + 8.0, row2),
        4.0,
        color(if v.limiting { theme::TEXT } else { theme::LINE }),
    );
    // Health: an alert takes the place of the quiet status.
    let alert = if !v.controller {
        Some("Contrôleur absent")
    } else if v.xrun_recent {
        Some("Xrun")
    } else if v.overload {
        Some("Surcharge CPU")
    } else {
        None
    };
    let at = Pos2::new(rect.right(), row2);
    match alert {
        Some(a) => {
            let r = text(p, at, Align2::RIGHT_CENTER, a, theme::LABEL, theme::INK);
            p.rect_filled(
                r.expand(5.0),
                radius(theme::RADIUS_CELL),
                color(theme::ACCENT_ALERT.base),
            );
            text(p, at, Align2::RIGHT_CENTER, a, theme::LABEL, theme::INK);
        }
        None => {
            text(
                p,
                at,
                Align2::RIGHT_CENTER,
                &format!("charge {:.0} %", v.load * 100.0),
                theme::BODY,
                theme::TEXT_DIM,
            );
        }
    }
}

/// The value of the control just touched, in its zone (top), ~1 s; waiting for pickup, the value
/// to reach.
pub(super) fn hud(p: &Painter, zones: [Rect; 3], hud: &Hud) {
    let zone = match hud.zone {
        Zone::A => zones[0],
        Zone::Center => zones[1],
        Zone::B => zones[2],
    };
    let format = |value: HudValue| match value {
        HudValue::Percent(x) => format!("{:.0} %", x * 100.0),
        HudValue::Bpm(bpm) => format!("{bpm:.0} BPM"),
    };
    let (value, hint) = match hud.pickup {
        Some(target) => (
            format!("{} → {}", format(hud.value), format(target)),
            Some("rejoindre la valeur pour reprendre la main"),
        ),
        None => (format(hud.value), None),
    };
    let label_size = layout(p, hud.label, theme::LABEL, theme::INK).size();
    let value_size = layout(p, &value, theme::TITLE, theme::INK).size();
    let hint_w = hint.map_or(0.0, |h| layout(p, h, theme::BODY, theme::INK).size().x);
    let w = label_size.x.max(value_size.x).max(hint_w) + 2.0 * theme::SPACE_4;
    let h = label_size.y + value_size.y + hint.map_or(0.0, |_| 20.0) + 2.0 * theme::SPACE_3;
    let r = Rect::from_min_size(
        Pos2::new(
            zone.center().x - w / 2.0,
            zone.top() + theme::SPACE_5 + 40.0,
        ),
        Vec2::new(w, h),
    );
    let (fg, bg) = if hud.pickup.is_some() {
        (theme::TEXT, theme::BG)
    } else {
        (theme::INK, theme::TEXT)
    };
    p.rect_filled(r, radius(theme::RADIUS_PAD), color(bg));
    if hud.pickup.is_some() {
        p.rect_stroke(
            r,
            radius(theme::RADIUS_PAD),
            Stroke::new(theme::STROKE_STATE, color(theme::TEXT)),
            StrokeKind::Inside,
        );
    }
    let label_rect = text(
        p,
        Pos2::new(r.left() + theme::SPACE_4, r.top() + theme::SPACE_3),
        Align2::LEFT_TOP,
        hud.label,
        theme::LABEL,
        fg,
    );
    let value_rect = text(
        p,
        Pos2::new(r.left() + theme::SPACE_4, label_rect.bottom()),
        Align2::LEFT_TOP,
        &value,
        theme::TITLE,
        fg,
    );
    if let Some(hint) = hint {
        text(
            p,
            Pos2::new(r.left() + theme::SPACE_4, value_rect.bottom() + 2.0),
            Align2::LEFT_TOP,
            hint,
            theme::BODY,
            fg,
        );
    }
}
