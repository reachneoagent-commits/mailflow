// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow's colour themes: the brand's own look and five alternatives, each for a light and a
//! dark window. A theme is a short table of seed colours (`Seed`); the rest of [`Tokens`] is
//! worked out from them the same way for every theme, so the themes differ in colour and in
//! nothing else (one typeface, one set of shapes and spacings). The seeds of the five
//! alternatives started from community themes on tweakcn.com (see `design/themes/themes.json`)
//! and were then adjusted until every pair in `theme_contrast_tests.rs` passed.
//!
//! High contrast is not another look of each theme: it is upstream's high-contrast palette, in
//! the owner's accent, which choosing a theme sets to the one nearest it
//! ([`ColorTheme::contrast_accent`]).

use egui::{Color32, Stroke};

use super::{Accent, AccentInk, Tokens};

fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The colour theme of the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorTheme {
    /// The brand: Vivid Blue on Paper White, and on Deep Blue in the dark.
    #[default]
    Mailflow,
    Ember,
    Evergreen,
    Stillwater,
    RosePine,
    Graphite,
}

/// The colours a theme is written in, for one of light and dark.
struct Seed {
    /// Behind the panes: title bar, module bar, status bar.
    window: u32,
    /// The folder pane.
    panel: u32,
    /// The message list and the reading pane.
    surface: u32,
    /// The ribbon's sheet, menus and dialogs.
    ribbon: u32,
    border: u32,
    text: u32,
    text_dim: u32,
    hover: u32,
    accent: u32,
    accent_text: u32,
    on_accent: u32,
    soft: u32,
    soft_text: u32,
    selection: u32,
    today: u32,
}

/// The relative luminance of a colour (WCAG 2).
fn luminance(c: Color32) -> f32 {
    let ch = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
}

/// The contrast ratio of two colours (WCAG 2).
fn ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `ink` made as quiet as it can be (mixed towards the first of `sheets`) while it keeps `min`
/// against every one of them.
fn quiet(ink: Color32, sheets: &[Color32], min: f32) -> Color32 {
    let Some(first) = sheets.first() else { return ink };
    for step in (1..=12u8).rev() {
        let candidate = ink.lerp_to_gamma(*first, f32::from(step) * 0.05);
        if sheets.iter().all(|s| ratio(candidate, *s) >= min) {
            return candidate;
        }
    }
    ink
}

impl ColorTheme {
    /// In picker order: the brand first.
    pub const ALL: [ColorTheme; 6] =
        [ColorTheme::Mailflow, ColorTheme::Ember, ColorTheme::Evergreen, ColorTheme::Stillwater, ColorTheme::RosePine, ColorTheme::Graphite];

    /// The value used in `ui.json` and by the `ui.colorTheme` command.
    pub fn code(self) -> &'static str {
        match self {
            ColorTheme::Mailflow => "mailflow",
            ColorTheme::Ember => "ember",
            ColorTheme::Evergreen => "evergreen",
            ColorTheme::Stillwater => "stillwater",
            ColorTheme::RosePine => "rose-pine",
            ColorTheme::Graphite => "graphite",
        }
    }

    pub fn from_code(code: &str) -> Option<ColorTheme> {
        ColorTheme::ALL.into_iter().find(|t| t.code().eq_ignore_ascii_case(code.trim()))
    }

    /// The English name (translated with `tl!` where shown).
    pub fn label(self) -> &'static str {
        match self {
            ColorTheme::Mailflow => "Mailflow",
            ColorTheme::Ember => "Ember",
            ColorTheme::Evergreen => "Evergreen",
            ColorTheme::Stillwater => "Stillwater",
            ColorTheme::RosePine => "Rosé Pine",
            ColorTheme::Graphite => "Graphite",
        }
    }

    /// The upstream accent whose high-contrast palette stands in for this theme.
    pub fn contrast_accent(self) -> Accent {
        match self {
            ColorTheme::Mailflow | ColorTheme::RosePine => Accent::Violet,
            ColorTheme::Evergreen => Accent::Teal,
            ColorTheme::Stillwater => Accent::Blue,
            ColorTheme::Ember | ColorTheme::Graphite => Accent::Graphite,
        }
    }

    fn seed(self, dark: bool) -> Seed {
        #[rustfmt::skip]
        let seed = match (self, dark) {
            (ColorTheme::Mailflow, false) => Seed { window: 0xF4F4F8, panel: 0xF7F7FB, surface: 0xFFFFFF, ribbon: 0xFFFFFF, border: 0xE3E2EE, text: 0x202030, text_dim: 0x5E5E6A, hover: 0xECEBF5, accent: 0x4737FF, accent_text: 0x3529BF, on_accent: 0xFFFFFF, soft: 0xE6E3FF, soft_text: 0x3529BF, selection: 0xF1F0FF, today: 0xECEAFF },
            (ColorTheme::Mailflow, true) => Seed { window: 0x101018, panel: 0x16161F, surface: 0x1B1B28, ribbon: 0x202030, border: 0x2E2E42, text: 0xF4F3FF, text_dim: 0xA3A3B5, hover: 0x2A2A3E, accent: 0x675AFF, accent_text: 0xACA5FF, on_accent: 0xFFFFFF, soft: 0x2A2666, soft_text: 0xD1CDFF, selection: 0x232345, today: 0x25234D },
            (ColorTheme::Ember, false) => Seed { window: 0xF0EEE6, panel: 0xF5F4EE, surface: 0xFAF9F5, ribbon: 0xFFFFFF, border: 0xDAD9D4, text: 0x3D3929, text_dim: 0x66645E, hover: 0xE9E6DC, accent: 0xB85735, accent_text: 0x9A4526, on_accent: 0xFFFFFF, soft: 0xF3DFD5, soft_text: 0x7E3519, selection: 0xF6ECE5, today: 0xF8EDE6 },
            (ColorTheme::Ember, true) => Seed { window: 0x1F1E1D, panel: 0x232321, surface: 0x262624, ribbon: 0x2C2C2B, border: 0x3E3E38, text: 0xF1F1EF, text_dim: 0xB7B5A9, hover: 0x353532, accent: 0xD97757, accent_text: 0xE8977B, on_accent: 0x141413, soft: 0x4A2E22, soft_text: 0xF6D2C4, selection: 0x332A25, today: 0x3A2C25 },
            (ColorTheme::Evergreen, false) => Seed { window: 0xF3F3F3, panel: 0xF8F8F8, surface: 0xFCFCFC, ribbon: 0xFFFFFF, border: 0xDFDFDF, text: 0x171717, text_dim: 0x5C5C5C, hover: 0xEDEDED, accent: 0x00804A, accent_text: 0x006B3D, on_accent: 0xFFFFFF, soft: 0xD9F2E5, soft_text: 0x005C35, selection: 0xEAF7F0, today: 0xE4F4EB },
            (ColorTheme::Evergreen, true) => Seed { window: 0x0E0E0E, panel: 0x121212, surface: 0x171717, ribbon: 0x1F1F1F, border: 0x292929, text: 0xE2E8F0, text_dim: 0xA2A2A2, hover: 0x2A2A2A, accent: 0x3ECF8E, accent_text: 0x5FDDA5, on_accent: 0x06241A, soft: 0x123524, soft_text: 0xBFF0D8, selection: 0x17271F, today: 0x182C22 },
            (ColorTheme::Stillwater, false) => Seed { window: 0xEEF0F3, panel: 0xF1F3F6, surface: 0xFFFFFF, ribbon: 0xFFFFFF, border: 0xDBE0E6, text: 0x1A2530, text_dim: 0x5A6573, hover: 0xE4E9EE, accent: 0x1A6289, accent_text: 0x0F4866, on_accent: 0xFFFFFF, soft: 0xD9E6EE, soft_text: 0x0F4866, selection: 0xEAF1F6, today: 0xE4EEF4 },
            (ColorTheme::Stillwater, true) => Seed { window: 0x0B0F14, panel: 0x0F1419, surface: 0x161C24, ribbon: 0x1C232D, border: 0x28313D, text: 0xE5E9EE, text_dim: 0x96A1B0, hover: 0x232C38, accent: 0x7AB0D0, accent_text: 0xA8CFE3, on_accent: 0x0F1419, soft: 0x1D3545, soft_text: 0xCFE6F2, selection: 0x1A2733, today: 0x1B2A38 },
            (ColorTheme::RosePine, false) => Seed { window: 0xF2E9E1, panel: 0xFAF4ED, surface: 0xFFFAF3, ribbon: 0xFFFAF3, border: 0xDFDAD9, text: 0x575279, text_dim: 0x656180, hover: 0xEDE3DA, accent: 0x7A6396, accent_text: 0x5F4A7C, on_accent: 0xFFFAF3, soft: 0xEFE6F1, soft_text: 0x56416F, selection: 0xF4ECF0, today: 0xF1E8EE },
            (ColorTheme::RosePine, true) => Seed { window: 0x191724, panel: 0x1C1A29, surface: 0x1F1D2E, ribbon: 0x26233A, border: 0x403D52, text: 0xE0DEF4, text_dim: 0xA6A2BE, hover: 0x2E2B45, accent: 0xC4A7E7, accent_text: 0xD3BDF0, on_accent: 0x191724, soft: 0x3A3157, soft_text: 0xE6DAF7, selection: 0x2A2640, today: 0x2D2847 },
            (ColorTheme::Graphite, false) => Seed { window: 0xF5F5F5, panel: 0xFAFAFA, surface: 0xFFFFFF, ribbon: 0xFFFFFF, border: 0xE5E5E5, text: 0x0A0A0A, text_dim: 0x5C5C5C, hover: 0xEDEDED, accent: 0x171717, accent_text: 0x171717, on_accent: 0xFAFAFA, soft: 0xE5E5E5, soft_text: 0x171717, selection: 0xF0F0F0, today: 0xEFEFEF },
            (ColorTheme::Graphite, true) => Seed { window: 0x0F0F0F, panel: 0x141414, surface: 0x171717, ribbon: 0x1F1F1F, border: 0x2E2E2E, text: 0xFAFAFA, text_dim: 0xA1A1A1, hover: 0x2A2A2A, accent: 0xE5E5E5, accent_text: 0xE5E5E5, on_accent: 0x171717, soft: 0x333333, soft_text: 0xFAFAFA, selection: 0x242424, today: 0x262626 },
        };
        seed
    }

    /// Every token of this theme, for a light or a dark window.
    pub fn tokens(self, dark: bool) -> Tokens {
        let s = self.seed(dark);
        let (window, panel, surface, ribbon) = (hex(s.window), hex(s.panel), hex(s.surface), hex(s.ribbon));
        let (border, text, text_dim, hover) = (hex(s.border), hex(s.text), hex(s.text_dim), hex(s.hover));
        let sheets = [surface, ribbon, window, panel];
        let outline = quiet(text_dim, &sheets, 3.2);
        let pressed = hover.lerp_to_gamma(text, 0.10);
        let faint = surface.lerp_to_gamma(border, 0.6);
        let flag = hex(0xD13B3B);

        // Upstream's palette for what a theme leaves alone: the status colours, the icon
        // colours, shadows, the scrim, the search highlight, avatars.
        let mut t = if dark { Tokens::dark() } else { Tokens::light() };
        t.title_bar = window;
        t.tab_strip = window;
        t.nav = window;
        t.canvas = window;
        t.status_bar = window;
        t.lane = window;
        t.panel = panel;
        t.surface = surface;
        t.ribbon = ribbon;
        t.menu = ribbon;
        t.input = surface;
        t.border = border;
        t.border_strong = border.lerp_to_gamma(text_dim, 0.35);
        t.text = text;
        t.text_dim = text_dim;
        t.group_label = text_dim;
        t.text_disabled = outline;
        t.icon = text.lerp_to_gamma(text_dim, 0.5);
        t.hover = hover;
        t.pressed = pressed;
        t.track = pressed;
        t.pill_quiet = hover;
        t.input_border = outline;
        t.field_edge = outline;
        t.rule = faint;
        t.border_faint = faint;
        t.ribbon_edge = if dark { Stroke::NONE } else { Stroke::new(1.0, surface.lerp_to_gamma(border, 0.8)) };
        t.off_hours = surface.lerp_to_gamma(window, 0.7);
        t.flagged_row = surface.lerp_to_gamma(flag, if dark { 0.14 } else { 0.07 });
        t.selection_unfocused = surface.lerp_to_gamma(text, 0.05);
        t.selection_edge_unfocused = t.border_strong;
        t.backstage_column = panel;
        t.backstage_ink = text;
        t.ink_on_dark = if dark { text } else { surface };
        t.ink_on_light = if dark { surface } else { text };
        t.hover_subtle = hover.linear_multiply(0.6);
        t.hover_nav = panel.lerp_to_gamma(hover, 0.7);
        t.surface_sunken = surface.linear_multiply(0.55);
        t.with_accent(AccentInk {
            accent: hex(s.accent),
            text: hex(s.accent_text),
            on_accent: hex(s.on_accent),
            soft: hex(s.soft),
            soft_text: hex(s.soft_text),
            selection: hex(s.selection),
            today: hex(s.today),
        })
    }
}

/// An unknown saved value reads as the default rather than discarding the whole `ui.json`.
impl<'de> serde::Deserialize<'de> for ColorTheme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
        Ok(v.as_str().and_then(ColorTheme::from_code).unwrap_or_default())
    }
}

impl serde::Serialize for ColorTheme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}
