//! Deck B (melody) after D4, and the gesture HUD of every zone.

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};

use super::rhythm::{pads, pending};
use super::{Hud, HudValue, View, Zone, color, pending_phase, radius, text};
use crate::theme;

/// Degree names of the chord pads (Hot Cue, then Shift + Hot Cue).
const DEGREES: [&str; 8] = ["I", "IV", "V", "vi", "ii", "iii", "vii", "I'"];
/// The same pads in scale order, for the degree row.
const SCALE_ORDER: [usize; 8] = [0, 4, 5, 1, 2, 3, 6, 7];

pub(super) const PADS_B: [(&str, [&str; 4]); 4] = [
    ("Hot Cue · accords", ["I", "IV", "V", "vi"]),
    ("Sampler · phrases", ["P1", "P2", "P3", "P4"]),
    ("FX · arpège", ["Off", "↑", "↑↓", "Aléa"]),
    ("Loop · voicing", ["Latch", "Renv", "Oct", "Ext"]),
];

const EXTENSIONS: [&str; 4] = ["triade", "7e", "sus4", "add9"];

pub(super) fn deck_b(p: &Painter, rect: Rect, v: &View) {
    let cold = theme::ACCENT_COLD;
    let inner = rect.shrink2(Vec2::new(theme::SPACE_5, theme::SPACE_5));
    let top = inner.top() + theme::STROKE_DECK;
    text(
        p,
        Pos2::new(inner.left(), top),
        Align2::LEFT_TOP,
        "B · Mélodie",
        theme::LABEL,
        cold.base,
    );
    let arp = if v.arp_mode == 0 {
        "Arpège off".to_owned()
    } else {
        format!(
            "Arpège {} {} · {} oct.",
            engine::melody::ArpMode::from_index(v.arp_mode).name(),
            engine::melody::ArpRate::from_index(v.arp_rate).name(),
            v.arp_octaves
        )
    };
    text(
        p,
        Pos2::new(inner.right(), top),
        Align2::RIGHT_TOP,
        &arp,
        theme::BODY,
        theme::TEXT_DIM,
    );
    // The chords sounding.
    let row = Rect::from_min_size(
        Pos2::new(inner.left(), top + 28.0),
        Vec2::new(inner.width(), theme::SIZE_BADGE),
    );
    let names: Vec<&str> = DEGREES
        .iter()
        .enumerate()
        .filter(|(i, _)| v.chords & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect();
    let label = if names.is_empty() {
        "—".to_owned()
    } else {
        names.join(" + ")
    };
    let galley_w = super::layout(p, &label, theme::DISPLAY, theme::INK)
        .size()
        .x;
    let badge = Rect::from_min_size(
        row.min,
        Vec2::new(
            (galley_w + 2.0 * theme::SPACE_4).max(theme::SIZE_BADGE),
            theme::SIZE_BADGE,
        ),
    );
    if names.is_empty() {
        p.rect_filled(badge, radius(theme::RADIUS_PAD), color(theme::CELL));
        text(
            p,
            badge.center(),
            Align2::CENTER_CENTER,
            &label,
            theme::DISPLAY,
            theme::TEXT_DIM,
        );
    } else {
        p.rect_filled(badge, radius(theme::RADIUS_PAD), color(cold.base));
        text(
            p,
            badge.center(),
            Align2::CENTER_CENTER,
            &label,
            theme::DISPLAY,
            theme::INK,
        );
    }
    let x = badge.right() + theme::SPACE_3;
    let held = if v.latch { "accord · latch" } else { "accord" };
    text(
        p,
        Pos2::new(x, row.center().y - 11.0),
        Align2::LEFT_CENTER,
        held,
        theme::BODY,
        theme::TEXT_DIM,
    );
    let transpose = if v.transpose == 0 {
        "dans la tonalité".to_owned()
    } else {
        format!("transposé {:+} degré(s)", v.transpose)
    };
    let shown = text(
        p,
        Pos2::new(x, row.center().y + 11.0),
        Align2::LEFT_CENTER,
        &transpose,
        theme::BODY,
        if v.transpose == 0 {
            theme::TEXT
        } else {
            cold.base
        },
    );
    // Phrase: playing, or waiting for the bar.
    let phrase_x = shown.right().max(x + 160.0) + theme::SPACE_4;
    match v.pending_phrase {
        Some(Some((index, variation))) => {
            let name = format!("P{}{}", index + 1, if variation { "'" } else { "" });
            pending(p, Pos2::new(phrase_x, row.center().y), &name, cold, v);
        }
        Some(None) => {
            text(
                p,
                Pos2::new(phrase_x, row.center().y),
                Align2::LEFT_CENTER,
                &format!("phrase arrêtée {}", super::until_bar(v)),
                theme::BODY,
                cold.base,
            );
        }
        None => {}
    }
    // The eight degrees in scale order, the sounding ones lit.
    let deg_top = row.bottom() + theme::SPACE_4;
    text(
        p,
        Pos2::new(inner.left(), deg_top),
        Align2::LEFT_TOP,
        "Degrés",
        theme::LABEL,
        theme::TEXT_DIM,
    );
    let cells_top = deg_top + 20.0;
    let gap = theme::SPACE_1;
    let w = (inner.width() - 7.0 * gap) / 8.0;
    for (slot, &pad) in SCALE_ORDER.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(inner.left() + slot as f32 * (w + gap), cells_top),
            Vec2::new(w, 30.0),
        );
        let on = v.chords & (1 << pad) != 0;
        p.rect_filled(
            r,
            radius(theme::RADIUS_CELL),
            color(if on { cold.base } else { theme::CELL }),
        );
        text(
            p,
            r.center(),
            Align2::CENTER_CENTER,
            DEGREES[pad],
            theme::BODY,
            if on { theme::INK } else { theme::TEXT },
        );
    }
    // Settings of the deck, P2 values.
    let values_y = cells_top + 30.0 + theme::SPACE_4;
    let phrase = match v.phrase {
        Some((index, variation)) => {
            format!(
                "{}{}",
                index + 1,
                if variation { " (variation)" } else { "" }
            )
        }
        None => "—".to_owned(),
    };
    let values = [
        ("Phrase", phrase),
        ("Octave", format!("{:+}", v.octave)),
        ("Renv.", format!("{:+}", v.inversion)),
        (
            "Ext.",
            EXTENSIONS
                .get(usize::from(v.extension))
                .copied()
                .unwrap_or("?")
                .to_owned(),
        ),
        (
            "Delay",
            engine::fx::DelayTime::from_index(v.delay_time)
                .name()
                .to_owned(),
        ),
    ];
    let mut vx = inner.left();
    for (name, value) in values {
        let n = text(
            p,
            Pos2::new(vx, values_y),
            Align2::LEFT_TOP,
            name,
            theme::BODY,
            theme::TEXT_DIM,
        );
        let val = text(
            p,
            Pos2::new(n.right() + theme::SPACE_1, values_y),
            Align2::LEFT_TOP,
            &value,
            theme::BODY,
            theme::TEXT,
        );
        vx = val.right() + theme::SPACE_5;
    }
    pads(
        p,
        inner,
        &PADS_B,
        v.pads[1],
        v.shown_mode[1],
        cold,
        pending_phase(v),
    );
}

/// The value of the control just touched, in its zone (top right), ~1 s; while a control waits
/// for pickup, the value to reach.
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
    let label_size = super::layout(p, hud.label, theme::LABEL, theme::INK).size();
    let value_size = super::layout(p, &value, theme::TITLE, theme::INK).size();
    let hint_w = hint.map_or(0.0, |h| {
        super::layout(p, h, theme::BODY, theme::INK).size().x
    });
    let w = label_size.x.max(value_size.x).max(hint_w) + 2.0 * theme::SPACE_4;
    let h = label_size.y + value_size.y + hint.map_or(0.0, |_| 20.0) + 2.0 * theme::SPACE_3;
    let x = if hud.zone == Zone::Center {
        zone.center().x - w / 2.0
    } else {
        zone.right() - theme::SPACE_5 - w
    };
    let r = Rect::from_min_size(
        Pos2::new(x, zone.top() + theme::SPACE_5 + 48.0),
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
            egui::StrokeKind::Inside,
        );
    }
    let mut y = r.top() + theme::SPACE_3;
    let label = text(
        p,
        Pos2::new(r.left() + theme::SPACE_4, y),
        Align2::LEFT_TOP,
        hud.label,
        theme::LABEL,
        fg,
    );
    y = label.bottom();
    let value_rect = text(
        p,
        Pos2::new(r.left() + theme::SPACE_4, y),
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
