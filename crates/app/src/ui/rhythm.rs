//! Deck A (rhythm) and the bass column, after D4: pattern badge and what waits for the bar, the step
//! grid of every voice, the pads of the shown mode with a preview of the three others.

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};

use super::{BassCell, Light, View, color, dashed_rect, pending_phase, radius, text, until_bar};
use crate::theme::{self, Accent};

/// Row names of the step grid, in `DrumVoice::ALL` order.
const VOICES: [&str; 6] = ["Kick", "Snare", "CH", "OH", "Clap", "Perc"];

/// Pad labels of each mode (Hot Cue, Sampler, FX, Loop), after `config/mapping.toml`.
pub(super) const PADS_A: [(&str, [&str; 4]); 4] = [
    ("Hot Cue · patterns", ["A", "B", "C", "D"]),
    ("Sampler · fills", ["F1", "F2", "F3", "F4"]),
    ("FX · mutes", ["Kick", "Snare", "Hats", "Perc"]),
    ("Loop · repeat", ["1/4", "1/8", "1/16", "1/32"]),
];

pub(super) fn deck_a(p: &Painter, rect: Rect, v: &View) {
    let warm = theme::ACCENT_WARM;
    let inner = rect.shrink2(Vec2::new(theme::SPACE_5, theme::SPACE_5));
    let top = inner.top() + theme::STROKE_DECK;
    text(
        p,
        Pos2::new(inner.left(), top),
        Align2::LEFT_TOP,
        "A · Rythme",
        theme::LABEL,
        warm.base,
    );
    let density = if v.beat_repeat {
        format!("Densité {:.0} % · repeat", v.density * 100.0)
    } else {
        format!("Densité {:.0} %", v.density * 100.0)
    };
    text(
        p,
        Pos2::new(inner.right(), top),
        Align2::RIGHT_TOP,
        &density,
        theme::BODY,
        theme::TEXT_DIM,
    );
    // Now / next.
    let row = Rect::from_min_size(
        Pos2::new(inner.left(), top + 28.0),
        Vec2::new(inner.width(), theme::SIZE_BADGE),
    );
    let letter = |i: u8| char::from(b'A' + i).to_string();
    let badge = Rect::from_min_size(row.min, Vec2::splat(theme::SIZE_BADGE));
    p.rect_filled(badge, radius(theme::RADIUS_PAD), color(warm.base));
    text(
        p,
        badge.center(),
        Align2::CENTER_CENTER,
        &letter(v.pattern),
        theme::DISPLAY,
        theme::INK,
    );
    let x = badge.right() + theme::SPACE_3;
    text(
        p,
        Pos2::new(x, row.center().y - 11.0),
        Align2::LEFT_CENTER,
        "pattern en cours",
        theme::BODY,
        theme::TEXT_DIM,
    );
    let name = text(
        p,
        Pos2::new(x, row.center().y + 11.0),
        Align2::LEFT_CENTER,
        &format!("Groove {}", letter(v.pattern)),
        theme::TITLE,
        theme::TEXT,
    );
    if let Some(next) = v.pending_pattern {
        pending(
            p,
            Pos2::new(name.right().max(x + 150.0) + theme::SPACE_4, row.center().y),
            &letter(next),
            warm,
            v,
        );
    } else if let Some((fill, started)) = v.fill {
        let label = if started {
            format!("Fill {} en cours", fill + 1)
        } else {
            format!("Fill {} au prochain temps", fill + 1)
        };
        text(
            p,
            Pos2::new(name.right().max(x + 150.0) + theme::SPACE_4, row.center().y),
            Align2::LEFT_CENTER,
            &label,
            theme::BODY,
            warm.base,
        );
    }
    // Step grid.
    let grid_top = row.bottom() + theme::SPACE_4;
    let label_w = 44.0;
    let cell_gap = theme::SPACE_1;
    let row_h = 20.0;
    let cells_left = inner.left() + label_w + theme::SPACE_2;
    let cell_w = (inner.right() - cells_left - 15.0 * cell_gap) / 16.0;
    for (voice, (name, steps)) in VOICES.iter().zip(v.grid.iter()).enumerate() {
        let y = grid_top + voice as f32 * (row_h + 5.0);
        let muted = v.mutes & (1 << voice) != 0;
        let label = text(
            p,
            Pos2::new(inner.left(), y + row_h / 2.0),
            Align2::LEFT_CENTER,
            name,
            theme::LABEL,
            if muted { theme::TEXT_DIM } else { theme::TEXT },
        );
        if muted {
            // Struck through: muted is never told by color alone.
            p.line_segment(
                [
                    Pos2::new(label.left(), label.center().y),
                    Pos2::new(label.right(), label.center().y),
                ],
                Stroke::new(theme::STROKE_HAIR, color(theme::TEXT_DIM)),
            );
        }
        for (step, &hit) in steps.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(cells_left + step as f32 * (cell_w + cell_gap), y),
                Vec2::new(cell_w, row_h),
            );
            let now = v.playing && usize::from(v.step) == step;
            let fill = match (hit, muted, now) {
                (0, _, true) => theme::MARK,
                (0, _, false) if step % 4 == 0 => theme::LINE,
                (0, _, false) => theme::CELL,
                (_, true, _) => warm.muted,
                (_, false, true) => warm.bright,
                (1, false, false) => warm.dim,
                (_, false, false) => warm.base,
            };
            p.rect_filled(r, radius(theme::RADIUS_CELL), color(fill));
            if hit == 3 && !muted {
                // Accent: a notch at the top of the cell.
                let notch = Rect::from_min_size(r.min, Vec2::new(r.width(), 3.0));
                p.rect_filled(notch, radius(theme::RADIUS_CELL), color(theme::INK));
            }
        }
    }
    pads(
        p,
        inner,
        &PADS_A,
        v.pads[0],
        v.shown_mode[0],
        warm,
        pending_phase(v),
    );
}

pub(super) fn bass_column(p: &Painter, rect: Rect, v: &View) {
    let amber = theme::ACCENT_AMBER;
    let inner = rect.shrink2(Vec2::new(theme::SPACE_4, theme::SPACE_5));
    let top = inner.top() + theme::STROKE_DECK;
    text(
        p,
        Pos2::new(inner.left(), top),
        Align2::LEFT_TOP,
        "Basse",
        theme::LABEL,
        if v.bass_muted {
            theme::TEXT_DIM
        } else {
            amber.base
        },
    );
    let badge = Rect::from_min_size(Pos2::new(inner.left(), top + 28.0), Vec2::splat(40.0));
    p.rect_filled(
        badge,
        radius(theme::RADIUS_PAD),
        color(if v.bass_muted {
            amber.muted
        } else {
            amber.base
        }),
    );
    text(
        p,
        badge.center(),
        Align2::CENTER_CENTER,
        &(v.bass_line + 1).to_string(),
        theme::TITLE,
        if v.bass_muted {
            theme::TEXT_DIM
        } else {
            theme::INK
        },
    );
    let status = match (v.bass_muted, v.pending_bass_line) {
        (true, _) => "muette".to_owned(),
        (false, Some(next)) => format!("→ {} à la mesure", next + 1),
        (false, None) => format!("ligne {}", v.bass_line + 1),
    };
    text(
        p,
        Pos2::new(badge.right() + theme::SPACE_2, badge.center().y),
        Align2::LEFT_CENTER,
        &status,
        theme::BODY,
        theme::TEXT_DIM,
    );
    // 16 steps, 4 × 4, the degree written in the note.
    let grid_top = badge.bottom() + theme::SPACE_4;
    let gap = theme::SPACE_1;
    let w = (inner.width() - 3.0 * gap) / 4.0;
    let h = 30.0;
    for (step, cell) in v.bass.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(
                inner.left() + (step % 4) as f32 * (w + gap),
                grid_top + (step / 4) as f32 * (h + gap),
            ),
            Vec2::new(w, h),
        );
        let now = v.playing && usize::from(v.step) == step;
        let lit = if v.bass_muted {
            amber.muted
        } else if now {
            amber.bright
        } else {
            amber.base
        };
        match *cell {
            BassCell::Rest => {
                p.rect_filled(
                    r,
                    radius(theme::RADIUS_CELL),
                    color(if now { theme::MARK } else { theme::CELL }),
                );
            }
            BassCell::Hold => {
                p.rect_filled(r, radius(theme::RADIUS_CELL), color(theme::CELL));
                let bar = Rect::from_center_size(r.center(), Vec2::new(r.width(), 6.0));
                p.rect_filled(bar, 0, color(lit));
            }
            BassCell::Note {
                degree,
                accent,
                glide,
            } => {
                p.rect_filled(r, radius(theme::RADIUS_CELL), color(lit));
                let label = format!("{}{degree}", if glide { "~" } else { "" });
                text(
                    p,
                    r.center(),
                    Align2::CENTER_CENTER,
                    &label,
                    if accent { theme::TITLE } else { theme::BODY },
                    if v.bass_muted {
                        theme::TEXT_DIM
                    } else {
                        theme::INK
                    },
                );
            }
        }
    }
    text(
        p,
        Pos2::new(inner.left(), grid_top + 4.0 * (h + gap) + theme::SPACE_1),
        Align2::LEFT_TOP,
        "degrés · ~ glide",
        theme::LABEL,
        theme::TEXT_DIM,
    );
    // Filter at the bottom.
    let track = Rect::from_min_size(
        Pos2::new(inner.left(), inner.bottom() - theme::SIZE_BAR),
        Vec2::new(inner.width(), theme::SIZE_BAR),
    );
    text(
        p,
        Pos2::new(inner.left(), track.top() - theme::SPACE_2),
        Align2::LEFT_BOTTOM,
        &format!("Filtre basse {:.0} %", v.bass_cutoff * 100.0),
        theme::BODY,
        theme::TEXT_DIM,
    );
    bar(p, track, v.bass_cutoff, amber);
}

/// A level or setting bar, 0..=1.
pub(super) fn bar(p: &Painter, track: Rect, value: f32, accent: Accent) {
    let pill = radius(theme::SIZE_BAR / 2.0);
    p.rect_filled(track, pill, color(theme::LINE));
    let filled = Rect::from_min_size(
        track.min,
        Vec2::new(track.width() * value.clamp(0.0, 1.0), track.height()),
    );
    p.rect_filled(filled, pill, color(accent.base));
}

/// "→ [B] dans 2 temps": the item waiting for the bar, outlined and blinking at the eighth.
pub(super) fn pending(p: &Painter, left_center: Pos2, label: &str, accent: Accent, v: &View) {
    let arrow_y = left_center.y;
    let x = left_center.x;
    let stroke = Stroke::new(theme::STROKE_STATE, color(theme::TEXT_DIM));
    p.line_segment(
        [Pos2::new(x, arrow_y), Pos2::new(x + 16.0, arrow_y)],
        stroke,
    );
    p.line_segment(
        [
            Pos2::new(x + 10.0, arrow_y - 5.0),
            Pos2::new(x + 16.0, arrow_y),
        ],
        stroke,
    );
    p.line_segment(
        [
            Pos2::new(x + 10.0, arrow_y + 5.0),
            Pos2::new(x + 16.0, arrow_y),
        ],
        stroke,
    );
    let r = Rect::from_center_size(Pos2::new(x + 24.0 + 26.0, arrow_y), Vec2::splat(52.0));
    let lit = pending_phase(v);
    p.rect_filled(
        r,
        radius(theme::RADIUS_PAD),
        color(if lit { accent.wait } else { theme::SURFACE }),
    );
    dashed_rect(p, r, if lit { accent.base } else { accent.dim });
    text(
        p,
        r.center(),
        Align2::CENTER_CENTER,
        label,
        theme::TITLE,
        accent.base,
    );
    text(
        p,
        Pos2::new(r.right() + theme::SPACE_3, arrow_y),
        Align2::LEFT_CENTER,
        &until_bar(v),
        theme::BODY,
        accent.base,
    );
}

/// The four pads of the mode the controller shows, large, then the three other modes small.
pub(super) fn pads(
    p: &Painter,
    inner: Rect,
    labels: &[(&str, [&str; 4]); 4],
    lights: [[Light; 4]; 4],
    shown: u8,
    accent: Accent,
    phase: bool,
) {
    let shown = usize::from(shown.min(3));
    let big_w = 172.0;
    let pad_w = (big_w - theme::SPACE_2) / 2.0;
    let pad_h = (theme::SIZE_PAD - theme::SPACE_2) / 2.0 + 10.0;
    let block_h = 2.0 * pad_h + theme::SPACE_2;
    let bottom = inner.bottom();
    let top = bottom - block_h;
    let (title, names) = labels[shown];
    text(
        p,
        Pos2::new(inner.left(), top - theme::SPACE_2),
        Align2::LEFT_BOTTOM,
        title,
        theme::LABEL,
        theme::TEXT,
    );
    for (i, (&light, name)) in lights[shown].iter().zip(names.iter()).enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(
                inner.left() + (i % 2) as f32 * (pad_w + theme::SPACE_2),
                top + (i / 2) as f32 * (pad_h + theme::SPACE_2),
            ),
            Vec2::new(pad_w, pad_h),
        );
        pad(p, r, name, light, accent, phase);
    }
    // Previews of the other modes.
    let mut x = inner.left() + big_w + theme::SPACE_5;
    for (mode, (title, _)) in labels.iter().enumerate() {
        if mode == shown {
            continue;
        }
        let short = title.split(" · ").next().unwrap_or(title);
        text(
            p,
            Pos2::new(x, top - theme::SPACE_2),
            Align2::LEFT_BOTTOM,
            short,
            theme::LABEL,
            theme::TEXT_DIM,
        );
        let mini = theme::SIZE_MINI_PAD;
        for (i, &light) in lights[mode].iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(
                    x + (i % 2) as f32 * (mini + theme::SPACE_1),
                    top + (i / 2) as f32 * (mini + theme::SPACE_1),
                ),
                Vec2::splat(mini),
            );
            let fill = match light {
                Light::Off => theme::CELL,
                Light::On => accent.base,
                Light::Wait if phase => accent.wait,
                Light::Wait => theme::SURFACE,
            };
            p.rect_filled(r, radius(theme::RADIUS_CELL), color(fill));
            match light {
                Light::Off => {
                    p.rect_stroke(
                        r,
                        radius(theme::RADIUS_CELL),
                        Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
                        egui::StrokeKind::Inside,
                    );
                }
                Light::Wait => dashed_rect(p, r, if phase { accent.base } else { accent.dim }),
                Light::On => {}
            }
        }
        x += 2.0 * mini + theme::SPACE_1 + theme::SPACE_5 + 12.0;
    }
}

fn pad(p: &Painter, r: Rect, name: &str, light: Light, accent: Accent, phase: bool) {
    match light {
        Light::On => {
            p.rect_filled(r, radius(theme::RADIUS_PAD), color(accent.base));
            text(
                p,
                r.center(),
                Align2::CENTER_CENTER,
                name,
                theme::TITLE,
                theme::INK,
            );
        }
        Light::Wait => {
            // Blinks at the eighth; the label stays readable in both phases.
            let (fill, edge) = if phase {
                (accent.wait, accent.base)
            } else {
                (theme::SURFACE, accent.dim)
            };
            p.rect_filled(r, radius(theme::RADIUS_PAD), color(fill));
            dashed_rect(p, r, edge);
            text(
                p,
                r.center(),
                Align2::CENTER_CENTER,
                name,
                theme::TITLE,
                accent.base,
            );
        }
        Light::Off => {
            p.rect_filled(r, radius(theme::RADIUS_PAD), color(theme::CELL));
            p.rect_stroke(
                r,
                radius(theme::RADIUS_PAD),
                Stroke::new(theme::STROKE_HAIR, color(theme::LINE)),
                egui::StrokeKind::Inside,
            );
            text(
                p,
                r.center(),
                Align2::CENTER_CENTER,
                name,
                theme::TITLE,
                theme::TEXT_DIM,
            );
        }
    }
}
