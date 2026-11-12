// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow: help with writing in the window (`mailcraft_model::ai`): choosing a provider and a
//! model and testing them, the owner's writing preferences, and the Writing Help window over a
//! message being written, which shows the text before and after and changes nothing until the
//! owner accepts.
//!
//! - **Nothing is sent by itself.** A request to a provider starts only from the owner's press of
//!   a button here (Test, Refresh, Download, Generate). The commands (`ui.ai`, `ui.aiAssist`)
//!   change settings and open the window; they never send anything, so no agent can have the
//!   owner's text sent anywhere.
//! - **The window says where the text goes** before it goes: to which provider, or that it stays
//!   on this computer.
//! - **The draft is the owner's.** The answer is shown beside the original; Accept puts it in as
//!   one edit that Undo takes back, Discard leaves the draft as it was.
//! - **The key is the owner's secret**: typed into a field that hides it, handed to the host,
//!   kept in the system's credential store, never in the settings or a log.

use std::sync::mpsc::Receiver;

use egui::{Align2, Sense, Stroke, Ui, vec2};
use mailcraft_model::ai::{self, Action, Call, Failure, Group, Job, Length, Outcome, Settings, Tone};
use serde_json::{Value, json};

use crate::MailApp;
use crate::theme::{Tokens, medium, regular, semibold};
use crate::widgets::{plain_button, primary_button};

/// What a request under way is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Test,
    Models,
    Pull,
    Assist,
}

/// The Writing Help window's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assist {
    pub action: Action,
    /// What the owner typed for [`Action::Custom`].
    pub wish: String,
    /// The part of the draft that is worked on, in characters, and its text.
    pub from: usize,
    pub to: usize,
    pub original: String,
    /// The answer, or why there is none.
    pub result: Option<Result<String, String>>,
}

/// What the window keeps for help with writing. None of it is saved.
#[derive(Default)]
pub struct State {
    pub(crate) pending: Option<(Purpose, Receiver<Result<Outcome, Failure>>)>,
    /// The provider's models, once listed.
    pub models: Vec<String>,
    /// How the last test, listing or download ended: whether it went well, and what to say.
    pub note: Option<(bool, String)>,
    /// A key being typed. Handed to the host with the next request and then forgotten here.
    pub(crate) key_field: String,
    pub assist: Option<Assist>,
}

impl State {
    pub fn busy(&self) -> Option<Purpose> {
        self.pending.as_ref().map(|(purpose, _)| *purpose)
    }
}

/// `ui.ai`: change settings for help with writing. Sends nothing anywhere.
pub fn command(app: &mut MailApp, p: &Value) -> Result<Value, String> {
    const USAGE: &str = "give any of `enabled` (true or false), `provider`, `model`, `baseUrl`, `tone`, `length`, `language`, `instructions`";
    let Some(map) = p.as_object() else { return Err(USAGE.into()) };
    if map.is_empty()
        || map.keys().any(|k| !["enabled", "provider", "model", "baseUrl", "tone", "length", "language", "instructions"].contains(&k.as_str()))
    {
        return Err(USAGE.into());
    }
    let mut next = app.ui.ai.clone();
    let text = |key: &str, max: usize| match map.get(key) {
        None => Ok(None),
        Some(v) => v.as_str().map(|s| Some(ai::clean(s, max))).ok_or_else(|| format!("`{key}` is text")),
    };
    if let Some(code) = text("provider", 40)? {
        let Some(preset) = ai::preset(&code) else {
            return Err("`provider` is ollama, lmstudio, groq, gemini, openrouter, openai, anthropic, nvidia or custom".into());
        };
        if preset.code != next.provider {
            // Another provider: its own model and key, and nothing listed yet.
            next.provider = preset.code.to_string();
            next.model = preset.suggested_model.to_string();
            next.key_saved = false;
            next.enabled = false;
            app.ai.models.clear();
            app.ai.note = None;
            app.ai.key_field.clear();
        }
    }
    if let Some(model) = text("model", ai::MAX_NAME)? {
        next.model = model.replace('\n', "").trim().to_string();
    }
    if let Some(url) = text("baseUrl", ai::MAX_URL)? {
        next.base_url = url.replace('\n', "").trim().to_string();
    }
    if let Some(tone) = text("tone", 40)? {
        next.tone = Tone::from_code(&tone).ok_or("`tone` is natural, friendly, professional, formal or direct")?;
    }
    if let Some(length) = text("length", 40)? {
        next.length = Length::from_code(&length).ok_or("`length` is concise, balanced or detailed")?;
    }
    if let Some(language) = text("language", 60)? {
        next.language = language.replace('\n', " ").trim().to_string();
    }
    if let Some(instructions) = text("instructions", ai::MAX_INSTRUCTIONS)? {
        next.instructions = instructions;
    }
    if let Some(on) = map.get("enabled") {
        let on = on.as_bool().ok_or("`enabled` is true or false")?;
        if on && next.preset().is_none() {
            return Err("choose a provider first".into());
        }
        next.enabled = on;
    }
    app.ui.ai = next;
    Ok(json!({"enabled": app.ui.ai.enabled, "provider": app.ui.ai.provider, "model": app.ui.ai.model, "ready": app.ui.ai.ready()}))
}

/// `ui.aiAssist`: open the Writing Help window on the message being written, with `action`
/// chosen. Sends nothing: the owner presses Generate.
pub fn open_assist(app: &mut MailApp, p: &Value) -> Result<Value, String> {
    let action = match p.get("action") {
        None => Action::Rewrite,
        Some(v) => v
            .as_str()
            .and_then(Action::from_code)
            .ok_or("`action` is rewrite, humanize, grammar, professional, friendly, concise, elaborate, clarity or custom")?,
    };
    let Some(doc) = app.session.compose.as_ref() else { return Err("no message is being written".into()) };
    let plain = doc.body.plain();
    let selection = doc.body.selection();
    let chars = plain.chars().count();
    // What is selected, or else what the owner wrote: not the signature, not a quoted message.
    let (from, to) = if selection.is_empty() {
        let range = ai::own_text(&plain);
        let before = plain.get(..range.start).map_or(0, |s| s.chars().count());
        let within = plain.get(range.clone()).map_or(0, |s| s.chars().count());
        (before, before + within)
    } else {
        (selection.start().min(chars), selection.end().min(chars))
    };
    let original: String = plain.chars().skip(from).take(to.saturating_sub(from)).collect();
    if original.trim().is_empty() {
        return Err("there is no text to work on yet: write something first".into());
    }
    app.ai.assist = Some(Assist { action, wish: String::new(), from, to, original, result: None });
    Ok(json!({"open": true, "action": action.code(), "ready": app.ui.ai.ready(), "characters": to - from}))
}

fn call(app: &MailApp, job: Job) -> Call {
    let s = &app.ui.ai;
    let typed = app.ai.key_field.trim();
    Call {
        kind: s.preset().map_or(ai::Kind::OpenAi, |p| p.kind),
        base: s.base(),
        model: s.model.trim().to_string(),
        key: (!typed.is_empty()).then(|| typed.to_string()),
        key_name: s.key_name(),
        remember_key: !typed.is_empty(),
        job,
    }
}

/// Start a request, on the owner's press. One at a time.
fn start(app: &mut MailApp, purpose: Purpose, job: Job) {
    if app.ai.pending.is_some() {
        return;
    }
    let call = call(app, job);
    let Some(run) = app.services.ai.as_ref() else {
        let said = (false, tl!("Writing help needs the desktop program: it isn't available here.").to_string());
        match purpose {
            Purpose::Assist => {
                if let Some(a) = app.ai.assist.as_mut() {
                    a.result = Some(Err(said.1));
                }
            }
            _ => app.ai.note = Some(said),
        }
        return;
    };
    app.ai.pending = Some((purpose, run(call)));
    if purpose != Purpose::Assist {
        app.ai.note = None;
    }
}

/// Take in what a request brought back. Called once a pass.
pub fn poll(app: &mut MailApp) {
    let Some((purpose, rx)) = app.ai.pending.as_ref() else { return };
    let got = match rx.try_recv() {
        Ok(got) => got,
        Err(std::sync::mpsc::TryRecvError::Empty) => return,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(Failure::Unreachable),
    };
    let purpose = *purpose;
    app.ai.pending = None;
    let typed_key = !app.ai.key_field.trim().is_empty();
    if got.is_ok() && typed_key {
        // The host kept it in the credential store: the window forgets it.
        app.ai.key_field.clear();
        app.ui.ai.key_saved = true;
    }
    let words = |f: &Failure| crate::i18n::t(&f.text()).to_string();
    match (purpose, got) {
        (Purpose::Test, Ok(_)) => {
            app.ui.ai.enabled = true;
            app.ai.note = Some((true, tl!("It works: the model answered. Writing help is on.").to_string()));
        }
        (Purpose::Models, Ok(Outcome::Models(models))) => {
            let suggested = app.ui.ai.preset().map_or("", |p| p.suggested_model);
            if app.ui.ai.model.trim().is_empty() || !models.contains(&app.ui.ai.model) {
                // The one suggested when the provider has it; else the owner picks.
                if let Some(pick) = models.iter().find(|m| m.as_str() == suggested).or(if models.len() == 1 { models.first() } else { None }) {
                    app.ui.ai.model = pick.clone();
                }
            }
            app.ai.note = Some(if models.is_empty() {
                (false, tl!("The server answered, and has no models yet.").to_string())
            } else {
                (true, tln!(models.len(), "{n} model found.", "{n} models found."))
            });
            app.ai.models = models;
        }
        (Purpose::Pull, Ok(_)) => {
            app.ui.ai.model = ai::STARTER_MODEL.to_string();
            app.ai.note = Some((true, tl!("The model is downloaded. Press Test to try it.").to_string()));
        }
        (Purpose::Assist, Ok(Outcome::Text(text))) => {
            if let Some(a) = app.ai.assist.as_mut() {
                a.result = Some(Ok(text));
            }
        }
        (Purpose::Assist, Err(f)) => {
            if let Some(a) = app.ai.assist.as_mut() {
                a.result = Some(Err(words(&f)));
            }
        }
        (Purpose::Assist, Ok(_)) => {
            if let Some(a) = app.ai.assist.as_mut() {
                a.result = Some(Err(words(&Failure::Garbled)));
            }
        }
        (_, Err(f)) => app.ai.note = Some((false, words(&f))),
        (_, Ok(_)) => app.ai.note = Some((false, words(&Failure::Garbled))),
    }
}

fn set(app: &mut MailApp, key: &str, value: &str) {
    let _ = app.run("ui.ai", json!({ key: value }));
}

fn chip(ui: &mut Ui, t: &Tokens, name: &str, on: bool) -> egui::Response {
    let w = ui.fonts_mut(|f| f.layout_no_wrap(name.to_string(), regular(12.5), t.text).size().x) + 24.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
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
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        name,
        if on { medium(12.5) } else { regular(12.5) },
        if on { t.accent_soft_text } else { t.text },
    );
    if resp.has_focus() {
        ui.painter().rect_stroke(r.expand(2.0), 8.0, Stroke::new(2.0, t.focus_ring), egui::StrokeKind::Outside);
    }
    resp
}

fn quiet(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(text).font(regular(12.0)).color(t.text_dim));
}

fn field_label(ui: &mut Ui, t: &Tokens, text: &str) {
    ui.label(egui::RichText::new(text).font(medium(12.5)).color(t.text));
}

/// Where what is asked goes, in a sentence.
pub fn where_it_goes(s: &Settings) -> String {
    match s.preset() {
        None => tl!("No provider is chosen.").to_string(),
        Some(_) if s.is_local() => tl!("Runs on this computer. Your text does not leave it.").to_string(),
        Some(p) if p.group == Group::Custom => tl!("Your text is sent to the server at the address you gave.").to_string(),
        Some(p) => crate::i18n::fmt(tl!("Your text is sent to {provider} to be worked on. Its terms decide what it keeps."), &[("provider", p.name)]),
    }
}

/// Choosing a provider of one of `groups`, its key and model, and testing them.
pub fn provider_panel(app: &mut MailApp, ui: &mut Ui, t: &Tokens, groups: &[Group]) {
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    let current = app.ui.ai.provider.clone();
    ui.horizontal_wrapped(|ui| {
        for p in ai::PRESETS.iter().filter(|p| groups.contains(&p.group)) {
            if chip(ui, t, tl!(p.name), p.code == current).clicked() {
                set(app, "provider", p.code);
            }
        }
    });
    let Some(preset) = app.ui.ai.preset().filter(|p| groups.contains(&p.group)) else {
        quiet(ui, t, tl!("Choose one to go on."));
        return;
    };
    quiet(ui, t, &where_it_goes(&app.ui.ai));
    match preset.group {
        Group::FreeTier => {
            quiet(ui, t, tl!("It offered free use when this was written. Limits and terms are the provider's and can change: check its page."))
        }
        Group::Paid => quiet(ui, t, tl!("Billed by the provider for what you use, on your own account.")),
        Group::Local => quiet(ui, t, tl!("Needs the provider's program running on this computer. Free; uses this computer's memory and processor.")),
        Group::Custom => {}
    }
    let busy = app.ai.busy();
    if preset.group == Group::Custom {
        field_label(ui, t, tl!("Server address"));
        let mut url = app.ui.ai.base_url.clone();
        if ui.add(egui::TextEdit::singleline(&mut url).hint_text("https://example.org/v1").desired_width(360.0)).changed() {
            set(app, "baseUrl", &url);
        }
    }
    if preset.group != Group::Local {
        field_label(ui, t, tl!("API key"));
        ui.horizontal(|ui| {
            let hint = if app.ui.ai.key_saved { tl!("A key is saved. Type another to replace it.") } else { tl!("Paste your key") };
            ui.add(egui::TextEdit::singleline(&mut app.ai.key_field).password(true).hint_text(hint).desired_width(300.0));
            if !preset.key_page.is_empty() {
                ui.hyperlink_to(tl!("Get a key"), preset.key_page);
            }
        });
        quiet(ui, t, tl!("Kept in this computer's password store, never in Mailflow's settings."));
        if app.ui.ai.key_saved && plain_button(ui, tl!("Remove Saved Key")).clicked() {
            if let Some(store) = app.services.secrets.as_ref() {
                store.delete(&app.ui.ai.key_name());
            }
            app.ui.ai.key_saved = false;
            app.ui.ai.enabled = false;
            app.ai.note = Some((true, tl!("The key was removed.").to_string()));
        }
    }
    field_label(ui, t, tl!("Model"));
    ui.horizontal(|ui| {
        let mut model = app.ui.ai.model.clone();
        if ui.add(egui::TextEdit::singleline(&mut model).hint_text(tl!("Pick from the list, or type a model's name")).desired_width(260.0)).changed()
        {
            set(app, "model", &model);
        }
        if !app.ai.models.is_empty() {
            let mut picked = None;
            egui::ComboBox::from_id_salt("ai_models").selected_text(tl!("Choose…")).width(120.0).show_ui(ui, |ui| {
                for m in &app.ai.models {
                    if ui.selectable_label(*m == app.ui.ai.model, m.as_str()).clicked() {
                        picked = Some(m.clone());
                    }
                }
            });
            if let Some(m) = picked {
                set(app, "model", &m);
            }
        }
        if ui
            .add_enabled(busy.is_none(), egui::Button::new(if busy == Some(Purpose::Models) { tl!("Listing…") } else { tl!("List Models") }))
            .clicked()
        {
            start(app, Purpose::Models, Job::Models);
        }
    });
    if preset.code == "ollama" {
        ui.horizontal_wrapped(|ui| {
            let label = if busy == Some(Purpose::Pull) { tl!("Downloading…") } else { tl!("Download Starter Model") };
            if ui.add_enabled(busy.is_none(), egui::Button::new(label)).clicked() {
                let _ = app.run("ui.ai", json!({"model": ai::STARTER_MODEL}));
                start(app, Purpose::Pull, Job::Pull);
            }
            ui.hyperlink_to(tl!("Get Ollama"), "https://ollama.com/download");
        });
        quiet(
            ui,
            t,
            &crate::i18n::fmt(
                tl!(
                    "The starter model is Qwen2.5 0.5B ({size}, Apache-2.0 licence), fetched by Ollama from its own library. It is small and quick; a larger model writes better if this computer has the memory."
                ),
                &[("size", ai::STARTER_SIZE)],
            ),
        );
        if busy == Some(Purpose::Pull) {
            quiet(ui, t, tl!("This can take a few minutes. Ollama keeps what it has if it is interrupted."));
        }
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let can = busy.is_none() && !app.ui.ai.model.trim().is_empty() && !app.ui.ai.base().is_empty();
        if primary_button(ui, if busy == Some(Purpose::Test) { tl!("Testing…") } else { tl!("Test Connection") }, can).clicked() {
            // A request that says nothing of the owner: one invented sentence to tidy.
            let (system, user) = ai::prompt(Action::Grammar, &Settings::default(), "Thanks for you're help yesterday.", "");
            start(app, Purpose::Test, Job::Chat { system, user });
        }
        if app.ui.ai.enabled {
            if plain_button(ui, tl!("Turn Writing Help Off")).clicked() {
                let _ = app.run("ui.ai", json!({"enabled": false}));
            }
        } else {
            quiet(ui, t, tl!("The test sends one made-up sentence, nothing of yours. Writing help turns on when it passes."));
        }
    });
    if let Some((ok, said)) = app.ai.note.clone() {
        ui.label(egui::RichText::new(said).font(regular(12.5)).color(if ok { t.green } else { t.red }));
    }
}

/// The owner's writing preferences: tone, length, language, and their own instructions.
pub fn prefs_panel(app: &mut MailApp, ui: &mut Ui, t: &Tokens) {
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    field_label(ui, t, tl!("Tone"));
    let tone = app.ui.ai.tone;
    ui.horizontal_wrapped(|ui| {
        for choice in Tone::ALL {
            if chip(ui, t, tl!(choice.label()), choice == tone).clicked() {
                set(app, "tone", choice.code());
            }
        }
    });
    field_label(ui, t, tl!("Length"));
    let length = app.ui.ai.length;
    ui.horizontal_wrapped(|ui| {
        for choice in Length::ALL {
            if chip(ui, t, tl!(choice.label()), choice == length).clicked() {
                set(app, "length", choice.code());
            }
        }
    });
    field_label(ui, t, tl!("Language"));
    let mut language = app.ui.ai.language.clone();
    if ui
        .add(egui::TextEdit::singleline(&mut language).hint_text(tl!("Leave empty to keep the language of what you wrote")).desired_width(360.0))
        .changed()
    {
        set(app, "language", &language);
    }
    field_label(ui, t, tl!("Your own instructions"));
    let mut own = app.ui.ai.instructions.clone();
    let hint = tl!("For example: Make it sound like me. Friendly and direct, never overly formal.");
    if ui.add(egui::TextEdit::multiline(&mut own).hint_text(hint).desired_rows(3).desired_width(f32::INFINITY)).changed() {
        set(app, "instructions", &own);
    }
    quiet(ui, t, tl!("These go with every request for writing help, and can be changed in File › Options."));
}

fn layer() -> egui::LayerId {
    crate::keys::modal_layer(egui::Id::unique("mailflow-writing-help"))
}

/// What a press in the Writing Help window asks for.
enum Press {
    Generate,
    Accept(String),
    Close,
    Setup,
}

/// The Writing Help window, over the message being written, while it is open.
pub fn show(app: &mut MailApp, ctx: &egui::Context) {
    if app.ai.assist.is_none() {
        return;
    }
    // The draft went away (sent, discarded): there is nothing to help with.
    if app.session.compose.is_none() {
        app.ai.assist = None;
        return;
    }
    let t = Tokens::get(ctx);
    let ready = app.ui.ai.ready();
    let busy = app.ai.busy() == Some(Purpose::Assist);
    let goes = where_it_goes(&app.ui.ai);
    let model = app.ui.ai.model.clone();
    let provider = app.ui.ai.preset().map_or("", |p| p.name);
    let frame = egui::Frame::window(&ctx.global_style()).fill(t.surface).inner_margin(egui::Margin::same(16));
    let width = (ctx.content_rect().width() - 48.0).clamp(280.0, 720.0);
    let tall = (ctx.content_rect().height() - 360.0).clamp(80.0, 220.0);
    let mut press = None;
    let Some(assist) = app.ai.assist.as_mut() else { return };
    egui::Modal::new(layer().id).frame(frame).backdrop_color(t.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        ui.label(egui::RichText::new(tl!("Writing Help")).font(semibold(16.0)).color(t.text));
        if !ready {
            ui.label(
                egui::RichText::new(tl!("Writing help isn't set up yet. Choose where it runs: on this computer, or with a provider you pick."))
                    .color(t.text_dim),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, tl!("Set Up Writing Help"), true).clicked() {
                    press = Some(Press::Setup);
                }
                if plain_button(ui, tl!("Close")).clicked() {
                    press = Some(Press::Close);
                }
            });
            return;
        }
        ui.horizontal_wrapped(|ui| {
            for action in Action::ALL {
                if chip(ui, &t, tl!(action.label()), action == assist.action).clicked() && !busy {
                    assist.action = action;
                }
            }
        });
        if assist.action == Action::Custom {
            ui.add(
                egui::TextEdit::singleline(&mut assist.wish)
                    .hint_text(tl!("Say what you want changed, in your own words"))
                    .desired_width(f32::INFINITY),
            );
        }
        let text_box = |ui: &mut Ui, id: &str, heading: &str, text: &str, colour: egui::Color32| {
            ui.label(egui::RichText::new(heading).font(medium(12.0)).color(t.text_dim));
            egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt(id).max_height(tall).auto_shrink([false, true]).show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text).font(regular(13.0)).color(colour)).wrap().selectable(true));
                });
            });
        };
        text_box(ui, "ai_before", tl!("Your text"), &assist.original, t.text);
        match &assist.result {
            Some(Ok(text)) => text_box(ui, "ai_after", tl!("Suggestion, written by the model"), text, t.text),
            Some(Err(why)) => {
                ui.label(egui::RichText::new(why.as_str()).color(t.red));
            }
            None if busy => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(egui::RichText::new(tl!("Writing…")).color(t.text_dim));
                });
            }
            None => {}
        }
        ui.label(
            egui::RichText::new(crate::i18n::fmt(
                tl!("{where} Model: {model} ({provider})."),
                &[("where", &goes), ("model", &model), ("provider", provider)],
            ))
            .font(regular(11.5))
            .color(t.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &assist.result {
            Some(Ok(text)) => {
                if primary_button(ui, tl!("Use This"), !busy).clicked() {
                    press = Some(Press::Accept(text.clone()));
                }
                if plain_button(ui, tl!("Try Again")).clicked() && !busy {
                    press = Some(Press::Generate);
                }
                if plain_button(ui, tl!("Discard")).clicked() {
                    press = Some(Press::Close);
                }
            }
            _ => {
                if primary_button(ui, if busy { tl!("Writing…") } else { tl!("Generate") }, !busy).clicked() {
                    press = Some(Press::Generate);
                }
                if plain_button(ui, tl!("Cancel")).clicked() {
                    press = Some(Press::Close);
                }
            }
        });
    });
    match press {
        Some(Press::Generate) => {
            let Some(a) = app.ai.assist.as_mut() else { return };
            a.result = None;
            let (system, user) = ai::prompt(a.action, &app.ui.ai, &a.original, &a.wish);
            start(app, Purpose::Assist, Job::Chat { system, user });
        }
        Some(Press::Accept(text)) => {
            let Some(a) = app.ai.assist.take() else { return };
            accept(app, &a, &text);
        }
        Some(Press::Close) => {
            app.ai.assist = None;
            // An answer still on its way is for nobody.
            if busy {
                app.ai.pending = None;
            }
        }
        Some(Press::Setup) => {
            app.ai.assist = None;
            let _ = app.run("ui.onboarding", json!({"page": "ai"}));
        }
        None => {}
    }
}

/// Put `text` in the draft in place of what was worked on, when that part is still what it was:
/// a draft changed meanwhile is left alone, and the owner is told.
pub(crate) fn accept(app: &mut MailApp, a: &Assist, text: &str) {
    let still: Option<String> =
        app.session.compose.as_ref().map(|doc| doc.body.plain().chars().skip(a.from).take(a.to.saturating_sub(a.from)).collect());
    if still.as_deref() != Some(a.original.as_str()) {
        app.status(tl!("The draft changed while the suggestion was written, so it wasn't put in."));
        return;
    }
    if app.run("compose.select", json!({"from": a.from, "to": a.to})).is_ok() {
        let _ = app.run("compose.type", json!({"text": text}));
    }
}
