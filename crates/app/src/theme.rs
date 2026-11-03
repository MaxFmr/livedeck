//! Design tokens (D5, docs/design/D5-system.md): every color, text style, spacing, radius, stroke,
//! size and duration of the interface. No widget writes a value of its own.
//!
//! The mockups use the same names as CSS variables (`css()`: `ACCENT_WARM.bright` is
//! `--warm-bright`), so moving a value from a mockup to egui is mechanical. Plain data for now:
//! the egui conversions come with the UI shell (phase 5).

/// An sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// `0xRRGGBB`.
    #[must_use]
    pub const fn hex(v: u32) -> Self {
        Self((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// `#RRGGBB`.
    #[must_use]
    pub fn css(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    /// Relative luminance (WCAG 2).
    #[must_use]
    pub fn luminance(self) -> f32 {
        let channel = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.0) + 0.7152 * channel(self.1) + 0.0722 * channel(self.2)
    }

    /// WCAG contrast ratio, 1 ..= 21.
    #[must_use]
    pub fn contrast(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// `self` moved by `t` (0..=1) toward `other`, rounded per channel.
    #[must_use]
    pub fn mix(self, other: Self, t: f32) -> Self {
        let m = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Self(m(self.0, other.0), m(self.1, other.1), m(self.2, other.2))
    }
}

// --- Greys: five steps, one role each ---

/// Background of the window.
pub const BG: Rgb = Rgb::hex(0x0A0A0B);
/// Panels (time bar, decks, bass, footer).
pub const SURFACE: Rgb = Rgb::hex(0x121214);
/// An unlit cell: empty step, pad off, unplayed bass step.
pub const CELL: Rgb = Rgb::hex(0x1A1A1D);
/// An unlit line: pad border, empty bar track, beat off.
pub const LINE: Rgb = Rgb::hex(0x2A2A2E);
/// A landmark: first beat of the bar, every 4th step, separators.
pub const MARK: Rgb = Rgb::hex(0x3A3A40);
/// Text and the "now" white (beat lit, playhead, HUD).
pub const TEXT: Rgb = Rgb::hex(0xE8E8EA);
/// Secondary text (P2 values, labels of unlit things).
pub const TEXT_DIM: Rgb = Rgb::hex(0x8A8A90);
/// Text on a lit (filled) color.
pub const INK: Rgb = BG;

/// A color family and its states. Derived from `base` by fixed rules (tested below):
/// `bright` = 68 % toward white, `muted` = 72 % toward `BG`, `wait` = 87 % toward `SURFACE`,
/// `dim` = 45 % toward `SURFACE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accent {
    /// CSS prefix (`--warm`, `--warm-bright`…).
    pub name: &'static str,
    /// Lit: active pattern, chord, pad; text of a pending item.
    pub base: Rgb,
    /// Hit under the playhead.
    pub bright: Rgb,
    /// Muted step or voice (still there, not playing).
    pub muted: Rgb,
    /// Fill of a pending item, blink phase on.
    pub wait: Rgb,
    /// Border of a pending item, blink phase off.
    pub dim: Rgb,
}

/// Deck A, rhythm.
pub const ACCENT_WARM: Accent = Accent {
    name: "warm",
    base: Rgb::hex(0xFF6A2B),
    bright: Rgb::hex(0xFFCFBB),
    muted: Rgb::hex(0x4F2514),
    wait: Rgb::hex(0x311D17),
    dim: Rgb::hex(0x944221),
};
/// The bass, between the decks (driven from deck A).
pub const ACCENT_AMBER: Accent = Accent {
    name: "amber",
    base: Rgb::hex(0xE8A33D),
    bright: Rgb::hex(0xF8E2C1),
    muted: Rgb::hex(0x483519),
    wait: Rgb::hex(0x2E2519),
    dim: Rgb::hex(0x88622B),
};
/// Deck B, melody.
pub const ACCENT_COLD: Accent = Accent {
    name: "cold",
    base: Rgb::hex(0x2BC8FF),
    bright: Rgb::hex(0xBBEDFF),
    muted: Rgb::hex(0x133F4F),
    wait: Rgb::hex(0x152A33),
    dim: Rgb::hex(0x207695),
};
/// The only alert color (controller missing, CPU overload, xruns): never a deck color.
pub const ACCENT_ALERT: Accent = Accent {
    name: "alert",
    base: Rgb::hex(0xFF3DA8),
    bright: Rgb::hex(0xFFC1E3),
    muted: Rgb::hex(0x4F1837),
    wait: Rgb::hex(0x311827),
    dim: Rgb::hex(0x942A65),
};
/// Health dot when everything is fine (small, never text).
pub const OK: Rgb = Rgb::hex(0x5BD18B);

pub const ACCENTS: [Accent; 4] = [ACCENT_WARM, ACCENT_AMBER, ACCENT_COLD, ACCENT_ALERT];

// --- Type: Inter, tabular figures, four sizes ---

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub name: &'static str,
    /// Points (= CSS px at 1×).
    pub size: f32,
    pub weight: u16,
    /// Capitals with letter spacing, in em.
    pub caps: bool,
    pub tracking: f32,
}

/// P1 at a glance: BPM, active pattern, chord.
pub const DISPLAY: TextStyle = TextStyle {
    name: "display",
    size: 36.0,
    weight: 700,
    caps: false,
    tracking: -0.02,
};
/// Pads, key and scale, pending item.
pub const TITLE: TextStyle = TextStyle {
    name: "title",
    size: 22.0,
    weight: 700,
    caps: false,
    tracking: 0.0,
};
/// Values (P2), HUD hint.
pub const BODY: TextStyle = TextStyle {
    name: "body",
    size: 14.0,
    weight: 500,
    caps: false,
    tracking: 0.0,
};
/// Zone and mode names ("A · RYTHME", "HOT CUE"), step-row labels.
pub const LABEL: TextStyle = TextStyle {
    name: "label",
    size: 12.0,
    weight: 700,
    caps: true,
    tracking: 0.12,
};

pub const TEXT_STYLES: [TextStyle; 4] = [DISPLAY, TITLE, BODY, LABEL];

// --- Spacing (4 px grid), radii, strokes, sizes ---

pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 20.0;
pub const SPACE_6: f32 = 24.0;
pub const SPACE_8: f32 = 32.0;

/// Steps and small cells.
pub const RADIUS_CELL: f32 = 2.0;
/// Pads, badges, HUD.
pub const RADIUS_PAD: f32 = 4.0;
/// Panels.
pub const RADIUS_PANEL: f32 = 6.0;

/// Unlit borders.
pub const STROKE_HAIR: f32 = 1.0;
/// Pending (dashed) and pickup outlines.
pub const STROKE_STATE: f32 = 2.0;
/// Color band at the top of a deck panel.
pub const STROKE_DECK: f32 = 3.0;
/// Dash of a pending outline: on, off.
pub const DASH: [f32; 2] = [4.0, 3.0];

pub const SIZE_HEADER: f32 = 64.0;
pub const SIZE_FOOTER: f32 = 56.0;
/// Height of a large pad (mode shown on the controller).
pub const SIZE_PAD: f32 = 80.0;
/// Height of a preview pad (the three other modes).
pub const SIZE_MINI_PAD: f32 = 24.0;
/// Active pattern / chord badge.
pub const SIZE_BADGE: f32 = 56.0;
/// Level and filter bars.
pub const SIZE_BAR: f32 = 6.0;
/// The layout must hold at this size (MacBook Air), and full screen.
pub const MIN_WINDOW: [f32; 2] = [1280.0, 800.0];

// --- Durations ---

/// A touched control shows its value this long after the last movement.
pub const HUD_MS: u64 = 1_000;
/// A control waiting for pickup keeps its HUD until caught, or this long without movement.
pub const PICKUP_HUD_MS: u64 = 2_500;
/// Pending items blink at the tempo: lit for an eighth note, then off for one (like the LEDs);
/// nothing blinks when stopped.
pub const BLINK_BEATS: f64 = 0.5;

/// Every token as CSS custom properties, the block the mockups paste in their `<style>`.
#[must_use]
pub fn css() -> String {
    let mut out = String::from(":root{\n");
    let mut color = |name: &str, c: Rgb| out.push_str(&format!("--{name}:{};\n", c.css()));
    for (name, c) in [
        ("bg", BG),
        ("surface", SURFACE),
        ("cell", CELL),
        ("line", LINE),
        ("mark", MARK),
        ("text", TEXT),
        ("text-dim", TEXT_DIM),
        ("ink", INK),
        ("ok", OK),
    ] {
        color(name, c);
    }
    for a in ACCENTS {
        color(a.name, a.base);
        color(&format!("{}-bright", a.name), a.bright);
        color(&format!("{}-muted", a.name), a.muted);
        color(&format!("{}-wait", a.name), a.wait);
        color(&format!("{}-dim", a.name), a.dim);
    }
    for t in TEXT_STYLES {
        out.push_str(&format!("--font-{}:{}px;\n", t.name, t.size));
    }
    for (name, px) in [
        ("space-1", SPACE_1),
        ("space-2", SPACE_2),
        ("space-3", SPACE_3),
        ("space-4", SPACE_4),
        ("space-5", SPACE_5),
        ("space-6", SPACE_6),
        ("space-8", SPACE_8),
        ("radius-cell", RADIUS_CELL),
        ("radius-pad", RADIUS_PAD),
        ("radius-panel", RADIUS_PANEL),
        ("stroke-hair", STROKE_HAIR),
        ("stroke-state", STROKE_STATE),
        ("stroke-deck", STROKE_DECK),
        ("size-header", SIZE_HEADER),
        ("size-footer", SIZE_FOOTER),
        ("size-pad", SIZE_PAD),
        ("size-mini-pad", SIZE_MINI_PAD),
        ("size-badge", SIZE_BADGE),
        ("size-bar", SIZE_BAR),
    ] {
        out.push_str(&format!("--{name}:{px}px;\n"));
    }
    out.push('}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgb = Rgb::hex(0xFFFFFF);

    #[test]
    fn accents_follow_their_derivation_rules() {
        for a in ACCENTS {
            assert_eq!(a.bright, a.base.mix(WHITE, 0.68), "{}", a.name);
            assert_eq!(a.muted, a.base.mix(BG, 0.72), "{}", a.name);
            assert_eq!(a.wait, a.base.mix(SURFACE, 0.87), "{}", a.name);
            assert_eq!(a.dim, a.base.mix(SURFACE, 0.45), "{}", a.name);
        }
    }

    #[test]
    fn every_text_reaches_4_5_to_1() {
        for ground in [BG, SURFACE, CELL] {
            assert!(TEXT.contrast(ground) >= 4.5);
            assert!(TEXT_DIM.contrast(ground) >= 4.5);
        }
        for a in ACCENTS {
            // Accent text on panels and on a pending fill, ink on lit pads and hits.
            assert!(a.base.contrast(SURFACE) >= 4.5, "{}", a.name);
            assert!(a.base.contrast(a.wait) >= 4.5, "{}", a.name);
            assert!(INK.contrast(a.base) >= 4.5, "{}", a.name);
            assert!(INK.contrast(a.bright) >= 4.5, "{}", a.name);
        }
        assert!(INK.contrast(TEXT) >= 4.5);
    }

    #[test]
    fn greys_climb_one_step_at_a_time() {
        let ramp = [BG, SURFACE, CELL, LINE, MARK, TEXT_DIM, TEXT];
        for pair in ramp.windows(2) {
            assert!(pair[1].luminance() > pair[0].luminance());
        }
    }

    #[test]
    fn deck_colors_differ_in_lightness_not_only_hue() {
        // Colors told apart differ in lightness (D1: never the color alone).
        let l = |a: Accent| a.base.luminance();
        assert!((l(ACCENT_WARM) - l(ACCENT_COLD)).abs() > 0.1);
        assert!((l(ACCENT_WARM) - l(ACCENT_AMBER)).abs() > 0.1);
    }

    #[test]
    fn spacing_sits_on_the_4_px_grid() {
        for px in [
            SPACE_1,
            SPACE_2,
            SPACE_3,
            SPACE_4,
            SPACE_5,
            SPACE_6,
            SPACE_8,
            SIZE_HEADER,
            SIZE_FOOTER,
            SIZE_PAD,
            SIZE_MINI_PAD,
            SIZE_BADGE,
        ] {
            assert_eq!(px % 4.0, 0.0, "{px}");
        }
    }

    #[test]
    fn the_d5_board_carries_these_tokens() {
        let board = include_str!("../../../design/mockups/project/D5.dc.html");
        assert!(
            board.contains(&css()),
            "paste theme::css() into the D5 board"
        );
    }
}
