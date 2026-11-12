// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow's setup, shown over the window at the first start and again from File › Options:
//! Welcome, Connect email, Appearance, Finish. Each section has its own Skip, which goes on to the
//! next one (there is no skipping of the whole setup at once); what was skipped is remembered
//! and offered again in the folder pane ([`setup_card`]) until it is done or the card is closed.
//! Leaving at any point leaves a window that works. It collects nothing itself: each choice is a
//! command (`ui.colorTheme`, `ui.theme`, the Add Account dialog), so what it sets is what
//! Options sets. The AI pages of the plan (provider, model, writing preferences) take their
//! places between Connect email and Appearance when there is an AI service to set up.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::MailApp;
use crate::theme::{Appearance, Tokens, medium, regular, semibold};
use crate::widgets::{plain_button, primary_button};

/// A page of the setup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Welcome,
    Email,
    /// Help with writing: where it runs, or not at all.
    Ai,
    /// The owner's writing preferences, when help with writing is on.
    Writing,
    Appearance,
    Finish,
}

impl Page {
    /// In the order they are shown.
    pub const ALL: [Page; 6] = [Page::Welcome, Page::Email, Page::Ai, Page::Writing, Page::Appearance, Page::Finish];

    /// The value the `ui.onboarding` command takes and gives.
    pub fn code(self) -> &'static str {
        match self {
            Page::Welcome => "welcome",
            Page::Email => "email",
            Page::Ai => "ai",
            Page::Writing => "writing",
            Page::Appearance => "appearance",
            Page::Finish => "finish",
        }
    }

    pub fn from_code(code: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.code().eq_ignore_ascii_case(code.trim()))
    }

    fn index(self) -> usize {
        Page::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    /// Whether the page is shown: the writing preferences only once help with writing is on.
    fn shown(self, app: &MailApp) -> bool {
        self != Page::Writing || app.ui.ai.enabled
    }

    /// The page after this one, of those shown.
    pub fn next(self, app: &MailApp) -> Option<Page> {
        Page::ALL.into_iter().skip(self.index() + 1).find(|p| p.shown(app))
    }

    /// The page before this one, of those shown.
    pub fn back(self, app: &MailApp) -> Option<Page> {
        Page::ALL.into_iter().take(self.index()).rev().find(|p| p.shown(app))
    }
}

/// `onboarded` as `ui.json` has it: only an explicit `false` asks for the setup, so a file from
/// before it existed, or one with something else there, doesn't bring it up.
pub fn lenient_done<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
    Ok(v.as_bool().unwrap_or(true))
}

/// The skipped sections as `ui.json` has them: the codes of sections that can be skipped, once
/// each; anything else there is passed over, and something that isn't a list is none.
pub fn lenient_pages<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let v = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
    let mut pages: Vec<String> = Vec::new();
    for code in v.as_array().into_iter().flatten().take(64).filter_map(|c| c.as_str()) {
        if let Some(page) = Page::from_code(code).filter(|p| p.skippable())
            && !pages.iter().any(|c| c == page.code())
        {
            pages.push(page.code().to_string());
        }
    }
    Ok(pages)
}

impl Page {
    /// A section the owner may skip: a choice to make, not the first page or the last.
    pub fn skippable(self) -> bool {
        matches!(self, Page::Email | Page::Ai | Page::Writing | Page::Appearance)
    }
}

/// The sections still to do: no account yet, or a section that was skipped. In page order.
pub fn remaining(app: &MailApp) -> Vec<Page> {
    let skipped = |p: Page| app.ui.setup_skipped.iter().any(|c| c == p.code());
    let mut left = Vec::new();
    if app.needs_setup() {
        left.push(Page::Email);
    }
    // Help with writing that was skipped and is still off. (Its preferences are never "left":
    // the defaults will do.)
    if skipped(Page::Ai) && !app.ui.ai.enabled {
        left.push(Page::Ai);
    }
    if skipped(Page::Appearance) {
        left.push(Page::Appearance);
    }
    left
}

/// `ui.onboarding`: one of `page` (open the setup at it), `skip` (skip that section and go on),
/// `next` (that section is done: go on), `done` (close the setup) and `reminder` (show or close
/// the card of what is left); nothing opens the setup at its first page.
pub fn command(app: &mut MailApp, p: &serde_json::Value) -> Result<serde_json::Value, String> {
    const KEYS: [&str; 5] = ["page", "skip", "next", "done", "reminder"];
    const USAGE: &str =
        "give one of `page`, `skip`, `next` (welcome, email, ai, writing, appearance or finish), `done` (true) or `reminder` (true or false)";
    let Some(map) = p.as_object() else { return Err(USAGE.into()) };
    let given: Vec<&str> = KEYS.into_iter().filter(|k| map.contains_key(*k)).collect();
    let page_of = |key: &str| {
        map.get(key)
            .and_then(|v| v.as_str())
            .and_then(Page::from_code)
            .ok_or_else(|| format!("`{key}` is welcome, email, ai, writing, appearance or finish"))
    };
    let open_at = |app: &mut MailApp, page: Page| {
        app.ui.onboarded = false;
        app.ui.backstage = false;
        app.onboarding_page = page;
    };
    match given.as_slice() {
        [] => open_at(app, Page::default()),
        ["page"] => {
            let page = page_of("page")?;
            open_at(app, page);
        }
        ["skip"] => {
            let page = page_of("skip")?;
            let Some(next) = page.next(app).filter(|_| page.skippable()) else {
                return Err("only email, ai, writing and appearance can be skipped".into());
            };
            if !app.ui.setup_skipped.iter().any(|c| c == page.code()) {
                app.ui.setup_skipped.push(page.code().to_string());
            }
            // What was skipped is offered again in the folder pane.
            app.ui.setup_reminder_hidden = false;
            open_at(app, next);
        }
        ["next"] => {
            let page = page_of("next")?;
            let Some(next) = page.next(app) else { return Err("finish is the last page: give `done`".into()) };
            app.ui.setup_skipped.retain(|c| c != page.code());
            open_at(app, next);
        }
        ["done"] => {
            if map.get("done").and_then(serde_json::Value::as_bool) != Some(true) {
                return Err("`done` is true".into());
            }
            app.ui.onboarded = true;
            app.onboarding_page = Page::default();
            // Add Account was offered in the setup: it isn't put up again after it.
            app.offer_setup = false;
        }
        ["reminder"] => match map.get("reminder").and_then(serde_json::Value::as_bool) {
            Some(shown) => app.ui.setup_reminder_hidden = !shown,
            None => return Err("`reminder` is true or false".into()),
        },
        _ => return Err(USAGE.into()),
    }
    let left: Vec<&str> = remaining(app).into_iter().map(Page::code).collect();
    Ok(json!({"open": !app.ui.onboarded, "page": app.onboarding_page.code(), "remaining": left, "reminder": !app.ui.setup_reminder_hidden}))
}

/// The brand gradient (Deep Blue to Vivid Blue to Pale Blue), at `f` of the way across.
fn gradient(f: f32) -> Color32 {
    const STOPS: [(f32, [u8; 3]); 4] = [(0.0, [0x10, 0x10, 0x18]), (0.42, [0x35, 0x29, 0xBF]), (0.68, [0x47, 0x37, 0xFF]), (1.0, [0xAC, 0xA5, 0xFF])];
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    let mut from = STOPS[0];
    for to in STOPS {
        if f <= to.0 {
            let span = (to.0 - from.0).max(f32::EPSILON);
            let k = ((f - from.0) / span).clamp(0.0, 1.0);
            let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * k).round() as u8;
            return Color32::from_rgb(mix(from.1[0], to.1[0]), mix(from.1[1], to.1[1]), mix(from.1[2], to.1[2]));
        }
        from = to;
    }
    Color32::from_rgb(from.1[0], from.1[1], from.1[2])
}

/// A card in the brand gradient with round corners: columns of two triangles, each column's top
/// and bottom following the corner it is under.
fn paint_gradient(ui: &Ui, r: Rect, radius: f32) {
    // A column a point under the corners, where the edge curves, and wider ones between.
    let radius = radius.clamp(0.0, r.width().min(r.height()) / 2.0);
    let corner = radius.ceil() as u32;
    let between = 48u32;
    let mut xs: Vec<f32> = (0..=corner).map(|i| r.min.x + (i as f32).min(radius)).collect();
    let (left, right) = (r.min.x + radius, r.max.x - radius);
    xs.extend((1..between).map(|i| left + (right - left) * i as f32 / between as f32));
    xs.extend((0..=corner).rev().map(|i| r.max.x - (i as f32).min(radius)));
    let mut mesh = egui::Mesh::default();
    for (i, x) in xs.into_iter().enumerate() {
        let i = i as u32;
        let f = (x - r.min.x) / r.width().max(1.0);
        let from_edge = (x - r.min.x).min(r.max.x - x);
        let inset = if from_edge < radius { radius - (radius * radius - (radius - from_edge).powi(2)).max(0.0).sqrt() } else { 0.0 };
        let colour = gradient(f);
        mesh.colored_vertex(pos2(x, r.min.y + inset), colour);
        mesh.colored_vertex(pos2(x, r.max.y - inset), colour);
        if i > 0 {
            let at = i * 2;
            mesh.add_triangle(at - 2, at - 1, at);
            mesh.add_triangle(at - 1, at, at + 1);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// The Mailflow mark (an envelope with a spark over it), drawn in `ink` to fill `r`; `paper` is
/// what is behind it, for the line that parts the flap. Our own drawing, after `brand/logo only.svg`.
pub(crate) fn paint_mark(painter: &egui::Painter, r: Rect, ink: Color32, paper: Color32) {
    // The drawing is 41.52 by 42.99.
    let k = (r.width() / 41.52).min(r.height() / 42.99);
    let origin = r.center() - vec2(41.52, 42.99) * k * 0.5;
    let at = |x: f32, y: f32| origin + vec2(x, y) * k;
    let fill = |points: Vec<egui::Pos2>| {
        painter.add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
    };
    // The envelope: its two shoulders and its body.
    fill(vec![at(0.0, 11.3), at(1.6, 10.9), at(20.2, 28.6), at(0.0, 28.6)]);
    fill(vec![at(41.52, 11.3), at(41.52, 28.6), at(20.2, 28.6), at(39.9, 10.9)]);
    let body = Rect::from_min_max(at(0.0, 28.0), at(41.52, 42.99));
    let corner = (6.2 * k).round().clamp(0.0, 255.0) as u8;
    painter.rect_filled(body, CornerRadius { nw: 0, ne: 0, sw: corner, se: corner }, ink);
    painter.line_segment([at(19.6, 27.6), at(10.3, 36.9)], Stroke::new((1.15 * k).max(1.0), paper));
    // The spark: four points round a square.
    let (cx, cy, tip, waist) = (21.48, 10.66, 10.6, 2.7);
    fill(vec![at(cx - waist, cy - waist), at(cx, cy - tip), at(cx + waist, cy - waist), at(cx, cy)]);
    fill(vec![at(cx + waist, cy - waist), at(cx + tip, cy), at(cx + waist, cy + waist), at(cx, cy)]);
    fill(vec![at(cx + waist, cy + waist), at(cx, cy + tip), at(cx - waist, cy + waist), at(cx, cy)]);
    fill(vec![at(cx - waist, cy + waist), at(cx - tip, cy), at(cx - waist, cy - waist), at(cx, cy)]);
    fill(vec![at(cx - waist, cy - waist), at(cx + waist, cy - waist), at(cx + waist, cy + waist), at(cx - waist, cy + waist)]);
}

/// The height of the tallest page, the Welcome.
const PAGE_HEIGHT: f32 = 350.0;

fn layer() -> egui::LayerId {
    crate::keys::modal_layer(egui::Id::unique("mailflow-onboarding"))
}

/// What a press in the setup asks for.
enum Step {
    To(Page),
    Next(Page),
    Skip(Page),
    Done,
    AddAccount,
}

/// The setup over the window, while it isn't done and no dialog is up (Add Account is one: the
/// setup waits behind it and comes back when it closes).
pub fn show(app: &mut MailApp, ctx: &egui::Context) {
    if app.ui.onboarded || app.dialog.is_some() {
        return;
    }
    let t = Tokens::get(ctx);
    let page = app.onboarding_page;
    let frame = egui::Frame::window(&ctx.global_style()).fill(t.surface).inner_margin(egui::Margin::same(14));
    let width = (ctx.content_rect().width() - 48.0).clamp(280.0, 736.0);
    let mut step = None;
    egui::Modal::new(layer().id).frame(frame).backdrop_color(t.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        let top = ui.cursor().min.y;
        match page {
            Page::Welcome => welcome(ui, &t),
            Page::Email => email(app, ui, &t, &mut step),
            Page::Ai => ai_page(app, ui, &t),
            Page::Writing => writing(app, ui, &t),
            Page::Appearance => appearance(app, ui, &t),
            Page::Finish => finish(app, ui, &t),
        }
        // Every page is as tall as the tallest, so the buttons stay where they are from page to
        // page (in a window with the room for it).
        let room = (ctx.content_rect().height() - 140.0).clamp(0.0, PAGE_HEIGHT);
        ui.add_space((room - (ui.cursor().min.y - top)).max(10.0));
        let waits = (page == Page::Email && app.needs_setup()) || (page == Page::Ai && !app.ui.ai.enabled);
        footer(ui, &t, page, page.next(app), page.back(app), waits, &mut step);
    });
    match step {
        Some(Step::To(p)) => {
            let _ = app.run("ui.onboarding", json!({"page": p.code()}));
        }
        Some(Step::Next(p)) => {
            let _ = app.run("ui.onboarding", json!({"next": p.code()}));
        }
        Some(Step::Skip(p)) => {
            let _ = app.run("ui.onboarding", json!({"skip": p.code()}));
        }
        Some(Step::Done) => {
            let _ = app.run("ui.onboarding", json!({"done": true}));
        }
        Some(Step::AddAccount) => {
            let _ = app.run("ui.dialog", json!({"name": "account"}));
        }
        None => {}
    }
}

fn title(ui: &mut Ui, t: &Tokens, heading: &str, text: &str) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(heading).font(semibold(20.0)).color(t.text));
    ui.label(egui::RichText::new(text).font(regular(13.0)).color(t.text_dim));
    ui.add_space(6.0);
}

/// A line of the Welcome page: a dot in the accent, what it is, and a word about it.
fn point(ui: &mut Ui, t: &Tokens, what: &str, about: &str) {
    ui.horizontal_top(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 4.0, t.accent);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(egui::RichText::new(what).font(medium(13.0)).color(t.text));
            ui.label(egui::RichText::new(about).font(regular(12.5)).color(t.text_dim));
        });
    });
}

fn welcome(ui: &mut Ui, t: &Tokens) {
    let (card, _) = ui.allocate_exact_size(vec2(ui.available_width(), 132.0), Sense::hover());
    paint_gradient(ui, card, 10.0);
    let mark = Rect::from_center_size(pos2(card.min.x + 62.0, card.center().y), vec2(52.0, 54.0));
    let f = ((mark.center().x - card.min.x) / card.width().max(1.0)).clamp(0.0, 1.0);
    paint_mark(ui.painter(), mark, Color32::WHITE, gradient(f));
    ui.painter().text(pos2(mark.max.x + 16.0, card.center().y), Align2::LEFT_CENTER, "Mailflow", medium(30.0), Color32::WHITE);
    title(ui, t, tl!("Welcome to Mailflow"), tl!("A few choices and your mail is ready. You can skip any of them and come back to it later."));
    point(ui, t, tl!("Mail first"), tl!("Mail, calendar, people and tasks work on their own."));
    point(ui, t, tl!("Help with writing, when you want it"), tl!("An assistant you choose and can leave off. Nothing is sent without you."));
    point(ui, t, tl!("Yours"), tl!("Your mail stays on this computer and with your mail provider."));
}

fn email(app: &MailApp, ui: &mut Ui, t: &Tokens, step: &mut Option<Step>) {
    title(
        ui,
        t,
        tl!("Connect your email"),
        tl!("Add the account you want to read and send mail with. You sign in on your provider's own page where it has one."),
    );
    let addresses: Vec<String> = app
        .session
        .mailbox
        .accounts
        .iter()
        .map(|a| a.email.trim())
        .filter(|e| !e.is_empty())
        .take(6)
        .map(|e| e.chars().filter(|c| !c.is_control()).take(120).collect())
        .collect();
    if addresses.is_empty() {
        if primary_button(ui, tl!("Add an Account…"), true).clicked() {
            *step = Some(Step::AddAccount);
        }
        ui.label(egui::RichText::new(tl!("No account yet. You can add one later from File › Add Account.")).font(regular(12.5)).color(t.text_dim));
    } else {
        for address in &addresses {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 7.0, t.accent_soft);
                let tick = [r.center() + vec2(-3.5, 0.0), r.center() + vec2(-1.0, 3.0), r.center() + vec2(3.5, -3.0)];
                ui.painter().add(egui::Shape::line(tick.to_vec(), Stroke::new(1.6, t.accent_soft_text)));
                ui.label(egui::RichText::new(address.as_str()).font(medium(13.0)).color(t.text));
            });
        }
        if plain_button(ui, tl!("Add Another Account…")).clicked() {
            *step = Some(Step::AddAccount);
        }
    }
}

/// Help with writing: where it runs (this computer, a service with free use, the owner's own
/// key), or not now. Choosing one shows its providers; nothing is sent until Test is pressed.
fn ai_page(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    use mailcraft_model::ai::Group;
    title(
        ui,
        t,
        tl!("Help with writing"),
        tl!(
            "Mailflow can rewrite, shorten or tidy what you write, when you ask it to. It is optional: mail works the same without it, and you can set it up later."
        ),
    );
    let chosen = app.ui.ai.preset().map(|p| p.group);
    let choices: [(&str, &str, &[Group]); 3] = [
        (tl!("On this computer"), tl!("Private: a small model runs here. Needs a free program, and some memory."), &[Group::Local]),
        (tl!("A free service"), tl!("A provider's free allowance, with a key from your own account there."), &[Group::FreeTier]),
        (tl!("My own API key"), tl!("A provider you pay for what you use, or your own server."), &[Group::Paid, Group::Custom]),
    ];
    let mut groups: Option<&[Group]> = choices.iter().find(|(_, _, g)| chosen.is_some_and(|c| g.contains(&c))).map(|(_, _, g)| *g);
    let picked = egui::Id::unique("mailflow-ai-choice");
    if let Some(i) = ui.ctx().data(|d| d.get_temp::<usize>(picked)) {
        groups = choices.get(i).map(|(_, _, g)| *g);
    }
    for (i, (name, about, g)) in choices.iter().enumerate() {
        let on = groups.is_some_and(|cur| cur == *g);
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, *name));
        let fill = if on {
            t.accent_soft
        } else if resp.hovered() {
            t.hover
        } else {
            t.surface
        };
        ui.painter().rect_filled(r, 8.0, fill);
        ui.painter().rect_stroke(r, 8.0, Stroke::new(1.0, if on { t.accent } else { t.border_strong }), egui::StrokeKind::Inside);
        ui.painter().text(
            pos2(r.min.x + 12.0, r.min.y + 14.0),
            Align2::LEFT_CENTER,
            *name,
            medium(13.0),
            if on { t.accent_soft_text } else { t.text },
        );
        ui.painter().text(pos2(r.min.x + 12.0, r.min.y + 31.0), Align2::LEFT_CENTER, *about, regular(12.0), t.text_dim);
        if resp.has_focus() {
            ui.painter().rect_stroke(r.expand(2.0), 10.0, Stroke::new(2.0, t.focus_ring), egui::StrokeKind::Outside);
        }
        if resp.clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(picked, i));
            groups = Some(*g);
        }
    }
    if let Some(groups) = groups {
        ui.add_space(4.0);
        crate::ai_ui::provider_panel(app, ui, t, groups);
    } else {
        ui.label(egui::RichText::new(tl!("Not now? Skip this step: it waits under Finish setting up.")).font(regular(12.5)).color(t.text_dim));
    }
}

/// How the owner likes their writing to sound, for the help they just turned on.
fn writing(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    title(ui, t, tl!("How you like to write"), tl!("Writing help follows these. All optional, and all can be changed later."));
    crate::ai_ui::prefs_panel(app, ui, t);
}

fn appearance(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    title(
        ui,
        t,
        tl!("Make it yours"),
        tl!("Pick a colour theme, and whether the window is light, dark or follows your computer. The whole window changes as you choose."),
    );
    ui.label(egui::RichText::new(tl!("Colour theme")).font(medium(12.5)).color(t.text));
    crate::theme_picker::color_theme_tiles(app, ui, t);
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("Light or dark")).font(medium(12.5)).color(t.text));
    let cur = app.ui.theme;
    ui.horizontal(|ui| {
        for a in Appearance::ALL {
            let name = tl!(a.label());
            let on = a == cur;
            let w = ui.fonts_mut(|f| f.layout_no_wrap(name.to_string(), regular(12.5), t.text).size().x) + 28.0;
            let (r, resp) = ui.allocate_exact_size(vec2(w.max(84.0), 28.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name));
            let fill = if on {
                t.accent_soft
            } else if resp.hovered() {
                t.hover
            } else {
                t.surface
            };
            ui.painter().rect_filled(r, 6.0, fill);
            ui.painter().rect_stroke(r, 6.0, Stroke::new(1.0, if on { t.accent } else { t.input_border }), egui::StrokeKind::Inside);
            let ink = if on { t.accent_soft_text } else { t.text };
            ui.painter().text(r.center(), Align2::CENTER_CENTER, name, if on { medium(12.5) } else { regular(12.5) }, ink);
            if resp.has_focus() {
                ui.painter().rect_stroke(r.expand(2.0), 8.0, Stroke::new(2.0, t.focus_ring), egui::StrokeKind::Outside);
            }
            if resp.clicked() {
                let _ = app.run("ui.theme", json!({"value": a.code()}));
            }
        }
    });
}

fn finish(app: &MailApp, ui: &mut Ui, t: &Tokens) {
    let left = remaining(app);
    let text = if left.is_empty() {
        tl!("Everything here can be changed in File › Options.")
    } else {
        tl!("What you skipped waits under Finish setting up, at the foot of the folder pane. Help › Run Setup brings these pages back at any time.")
    };
    title(ui, t, tl!("You're set"), text);
    let account = if left.contains(&Page::Email) {
        tl!("Skipped. Add an account from Finish setting up, or from File › Add Account.")
    } else {
        tl!("Your account is connected.")
    };
    point(ui, t, tl!("Email"), account);
    let look = if left.contains(&Page::Appearance) {
        tl!("Skipped. Mailflow's own colours for now; choose others from Finish setting up, or in File › Options.")
    } else {
        tl!(app.ui.color_theme.label())
    };
    point(ui, t, tl!("Appearance"), look);
    let help = if app.ui.ai.ready() {
        crate::i18n::fmt(
            tl!("On, with {model}. {where}"),
            &[("model", app.ui.ai.model.as_str()), ("where", &crate::ai_ui::where_it_goes(&app.ui.ai))],
        )
    } else {
        tl!("Off. Mail works without it. Set it up from Finish setting up, or in File › Options.").to_string()
    };
    point(ui, t, tl!("Writing help"), &help);
}

/// The sections still to do, at the foot of the folder pane: a row for each, which opens the
/// setup at it, and a cross that closes the card (Help › Run Setup still brings the setup back).
/// Nothing while the setup is up, when nothing is left, or once the card was closed.
pub(crate) fn setup_card(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    let left = remaining(app);
    if !app.ui.onboarded || app.ui.setup_reminder_hidden || left.is_empty() {
        return;
    }
    let mut run: Option<serde_json::Value> = None;
    egui::Frame::NONE
        .outer_margin(egui::Margin::same(8))
        .inner_margin(egui::Margin::same(10))
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(8)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Finish setting up")).font(medium(12.5)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    let name = tl!("Close");
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                    if resp.hovered() || resp.has_focus() {
                        ui.painter().rect_filled(r, 4.0, t.hover);
                    }
                    let (c, h) = (r.center(), 3.5);
                    ui.painter().line_segment([c + vec2(-h, -h), c + vec2(h, h)], Stroke::new(1.3, t.text_dim));
                    ui.painter().line_segment([c + vec2(-h, h), c + vec2(h, -h)], Stroke::new(1.3, t.text_dim));
                    if resp.on_hover_text(tl!("Hide this. Help › Run Setup brings the setup back.")).clicked() {
                        run = Some(json!({"reminder": false}));
                    }
                });
            });
            for page in left {
                let name = match page {
                    Page::Email => tl!("Connect your email"),
                    Page::Ai => tl!("Set up writing help"),
                    _ => tl!("Choose your look"),
                };
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
                if resp.hovered() || resp.has_focus() {
                    ui.painter().rect_filled(r, 5.0, t.hover);
                }
                ui.painter().circle_stroke(pos2(r.min.x + 10.0, r.center().y), 5.0, Stroke::new(1.4, t.accent));
                ui.painter().text(pos2(r.min.x + 24.0, r.center().y), Align2::LEFT_CENTER, name, regular(12.5), t.accent_text);
                let (c, h) = (pos2(r.max.x - 10.0, r.center().y), 3.5);
                ui.painter().add(egui::Shape::line(
                    vec![c + vec2(-h * 0.5, -h), c + vec2(h * 0.5, 0.0), c + vec2(-h * 0.5, h)],
                    Stroke::new(1.3, t.text_dim),
                ));
                if resp.clicked() {
                    run = Some(json!({"page": page.code()}));
                }
            }
        });
    if let Some(params) = run {
        let _ = app.run("ui.onboarding", params);
    }
}

/// Where the setup is (a dot a page), and its buttons: Back, Skip for a section that can be
/// skipped, and what goes on (Get Started, Continue, Open Mailflow). Skip skips its own section
/// and nothing else. Connect email with no account yet has no Continue: its way on is to add an
/// account or to skip.
fn footer(ui: &mut Ui, t: &Tokens, page: Page, next: Option<Page>, back: Option<Page>, waits: bool, step: &mut Option<Step>) {
    ui.horizontal(|ui| {
        let (dots, _) = ui.allocate_exact_size(vec2(14.0 * Page::ALL.len() as f32, 26.0), Sense::hover());
        for (i, p) in Page::ALL.into_iter().enumerate() {
            let centre = pos2(dots.min.x + 5.0 + 14.0 * i as f32, dots.center().y);
            ui.painter().circle_filled(centre, if p == page { 4.0 } else { 3.0 }, if p == page { t.accent } else { t.border_strong });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            match next {
                Some(_) if waits => {}
                Some(next) if page == Page::Welcome => {
                    if primary_button(ui, tl!("Get Started"), true).clicked() {
                        *step = Some(Step::To(next));
                    }
                }
                Some(_) => {
                    if primary_button(ui, tl!("Continue"), true).clicked() {
                        *step = Some(Step::Next(page));
                    }
                }
                None => {
                    if primary_button(ui, tl!("Open Mailflow"), true).clicked() {
                        *step = Some(Step::Done);
                    }
                }
            }
            // A section that is done (an account is connected, help with writing is on) has
            // nothing to skip.
            let done = matches!(page, Page::Email | Page::Ai) && !waits;
            if page.skippable() && !done && plain_button(ui, tl!("Skip This Step")).clicked() {
                *step = Some(Step::Skip(page));
            }
            if let Some(back) = back
                && plain_button(ui, tl!("Back")).clicked()
            {
                *step = Some(Step::To(back));
            }
        });
    });
}
