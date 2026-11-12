// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! MailCraft's mark, small, for the title bar.

use egui::{Color32, Painter, Rect};

/// The app icon in small: its tile and the mark on it, drawn from their shapes. `r` is a square.
pub fn app_mark(p: &Painter, r: Rect) {
    // Mailflow: the brand's mark, white on a tile in Vivid Blue (`crate::onboarding::paint_mark`).
    let s = r.width().min(r.height());
    let tile = Color32::from_rgb(0x47, 0x37, 0xFF);
    p.rect_filled(r, s * 0.24, tile);
    crate::onboarding::paint_mark(p, r.shrink(s * 0.2), Color32::WHITE, tile);
}
