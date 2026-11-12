// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! The colour theme picker: one tile for each of Mailflow's themes, each a small picture of the
//! window in that theme (as the window is now, light or dark). Shown in File › Options and in
//! the Welcome window.

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::MailApp;
use crate::theme::{ColorTheme, Tokens, medium, regular};

const TILE: egui::Vec2 = vec2(112.0, 92.0);
const PICTURE: f32 = 64.0;

/// The tiles, wrapping to the width there is. Pressing one runs `ui.colorTheme`.
pub(crate) fn color_theme_tiles(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    let cur = app.ui.color_theme;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
        for theme in ColorTheme::ALL {
            let name = tl!(theme.label());
            let (r, resp) = ui.allocate_exact_size(TILE, Sense::click());
            let on = theme == cur;
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name));
            let picture = Rect::from_min_size(r.min, vec2(r.width(), PICTURE));
            paint_picture(ui, picture, &theme.tokens(t.dark));
            let edge = if on {
                Stroke::new(2.0, t.accent)
            } else if resp.hovered() {
                Stroke::new(1.0, t.text_dim)
            } else {
                Stroke::new(1.0, t.border_strong)
            };
            ui.painter().rect_stroke(picture, 8.0, edge, egui::StrokeKind::Inside);
            if on {
                let disc = pos2(picture.max.x - 13.0, picture.min.y + 13.0);
                ui.painter().circle_filled(disc, 8.0, t.accent);
                let tick = [disc + vec2(-3.5, 0.0), disc + vec2(-1.0, 3.0), disc + vec2(3.5, -3.0)];
                ui.painter().add(egui::Shape::line(tick.to_vec(), Stroke::new(1.8, t.on_accent)));
            }
            let font = if on { medium(12.5) } else { regular(12.5) };
            ui.painter().text(pos2(r.center().x, r.max.y - 12.0), Align2::CENTER_CENTER, name, font, if on { t.text } else { t.text_dim });
            if resp.has_focus() {
                ui.painter().rect_stroke(r.expand(2.0), 8.0, Stroke::new(2.0, t.focus_ring), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                let _ = app.run("ui.colorTheme", json!({"theme": theme.code()}));
            }
        }
    });
}

/// A window in miniature: the module bar, the folder pane with one folder chosen, and a list
/// with an unread message and a button.
fn paint_picture(ui: &Ui, r: Rect, p: &Tokens) {
    let painter = ui.painter().with_clip_rect(r);
    painter.rect_filled(r, 8.0, p.canvas);
    let folders = Rect::from_min_max(pos2(r.min.x + 8.0, r.min.y + 8.0), pos2(r.min.x + 40.0, r.max.y));
    painter.rect_filled(folders, egui::CornerRadius { nw: 4, ne: 0, sw: 0, se: 0 }, p.panel);
    let sheet = Rect::from_min_max(pos2(folders.max.x + 1.0, r.min.y + 8.0), r.max);
    painter.rect_filled(sheet, 0.0, p.surface);
    let bar = |x: f32, y: f32, w: f32, h: f32, c: egui::Color32| {
        painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w, h)), h / 2.0, c);
    };
    let x = folders.min.x + 5.0;
    painter.rect_filled(Rect::from_min_size(pos2(x - 2.0, folders.min.y + 6.0), vec2(26.0, 9.0)), 3.0, p.accent_soft);
    bar(x + 1.0, folders.min.y + 9.0, 16.0, 3.0, p.accent_soft_text);
    bar(x + 1.0, folders.min.y + 22.0, 20.0, 3.0, p.text_dim);
    bar(x + 1.0, folders.min.y + 32.0, 14.0, 3.0, p.text_dim);
    let x = sheet.min.x + 8.0;
    painter.circle_filled(pos2(x + 2.0, sheet.min.y + 10.5), 2.5, p.accent);
    bar(x + 8.0, sheet.min.y + 9.0, 30.0, 3.0, p.text);
    bar(x + 8.0, sheet.min.y + 17.0, 42.0, 3.0, p.text_dim);
    painter.hline(sheet.min.x..=sheet.max.x, sheet.min.y + 26.0, Stroke::new(1.0, p.rule));
    bar(x + 8.0, sheet.min.y + 32.0, 24.0, 3.0, p.text_dim);
    painter.rect_filled(Rect::from_min_size(pos2(x + 8.0, sheet.min.y + 40.0), vec2(28.0, 10.0)), 5.0, p.accent);
    bar(x + 14.0, sheet.min.y + 43.5, 16.0, 3.0, p.on_accent);
}
