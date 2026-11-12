// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow: help with writing in the window: the settings, what is sent and when, and what
//! becomes of the draft.

use std::sync::{Arc, Mutex};

use mailcraft_engine::Session;
use mailcraft_model::ai::{Call, Failure, Job, Outcome};
use serde_json::json;

use crate::ai_ui::Purpose;
use crate::onboarding::Page;
use crate::{MailApp, Services, UiState};

const NOW: i64 = 1_791_993_600;

type Calls = Arc<Mutex<Vec<Call>>>;

/// A window whose host answers every chat with `answer` and every list with two models, and
/// keeps what it was asked.
fn app_with(answer: Result<&'static str, Failure>) -> (MailApp, Calls) {
    let calls: Calls = Arc::default();
    let seen = calls.clone();
    let services = Services {
        ai: Some(Box::new(move |call: Call| {
            let got = match (&call.job, &answer) {
                (Job::Models, _) => Ok(Outcome::Models(vec!["big".into(), "qwen2.5:0.5b".into()])),
                (Job::Pull, _) => Ok(Outcome::Pulled),
                (Job::Chat { .. }, Ok(text)) => Ok(Outcome::Text((*text).to_string())),
                (Job::Chat { .. }, Err(why)) => Err(why.clone()),
            };
            seen.lock().unwrap().push(call);
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(got).unwrap();
            rx
        })),
        ..Services::default()
    };
    let mut a = MailApp::new(Session::at(mailcraft_engine::sample::mailbox(NOW, 0), NOW, 0), services);
    a.autosave = false;
    (a, calls)
}

fn ready(a: &mut MailApp) {
    a.execute("ui.ai", json!({"provider": "ollama"})).unwrap();
    a.execute("ui.ai", json!({"enabled": true})).unwrap();
    assert!(a.ui.ai.ready());
}

fn draft(a: &mut MailApp, text: &str) {
    a.execute("mail.new", json!({})).unwrap();
    let len = a.session.compose.as_ref().unwrap().body.plain().chars().count();
    a.execute("compose.select", json!({"from": 0, "to": len})).unwrap();
    a.execute("compose.type", json!({"text": text})).unwrap();
    let end = a.session.compose.as_ref().unwrap().body.plain().chars().count();
    a.execute("compose.select", json!({"from": end})).unwrap();
}

#[test]
fn help_is_off_in_a_new_window_and_the_settings_command_sends_nothing() {
    let (mut a, calls) = app_with(Ok("x"));
    assert!(!UiState::default().ai.enabled && !a.ui.ai.ready());
    assert_eq!(a.execute("ui.ai", json!({"provider": "ollama"})).unwrap()["model"], "qwen2.5:0.5b");
    assert!(!a.ui.ai.enabled);
    a.execute("ui.ai", json!({"tone": "direct", "length": "concise", "language": "Deutsch", "instructions": "Kurz."})).unwrap();
    assert_eq!((a.ui.ai.tone.code(), a.ui.ai.length.code(), a.ui.ai.language.as_str()), ("direct", "concise", "Deutsch"));
    for junk in [
        json!(null),
        json!({}),
        json!([]),
        json!({"provider": "skynet"}),
        json!({"provider": 3}),
        json!({"tone": "angry"}),
        json!({"length": 5}),
        json!({"enabled": "yes"}),
        json!({"key": "sk-secret"}),
        json!({"model": null}),
    ] {
        assert!(a.execute("ui.ai", junk).is_err());
    }
    // Help can't be turned on with no provider chosen.
    let (mut b, _) = app_with(Ok("x"));
    assert!(b.execute("ui.ai", json!({"enabled": true})).is_err());
    // Another provider starts from its own model, with help off until it is tested.
    a.execute("ui.ai", json!({"enabled": true})).unwrap();
    a.execute("ui.ai", json!({"provider": "anthropic"})).unwrap();
    assert_eq!((a.ui.ai.enabled, a.ui.ai.model.as_str(), a.ui.ai.key_saved), (false, "claude-opus-5-5", false));
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn opening_the_window_sends_nothing_and_takes_the_owners_text_alone() {
    let (mut a, calls) = app_with(Ok("Better."));
    ready(&mut a);
    assert!(a.execute("ui.aiAssist", json!({})).is_err(), "nothing is being written");
    draft(&mut a, "Hi Sam, thanks for you're help.");
    let said = a.execute("ui.aiAssist", json!({"action": "grammar"})).unwrap();
    assert_eq!((said["open"].clone(), said["action"].clone()), (json!(true), json!("grammar")));
    let assist = a.ai.assist.clone().unwrap();
    assert!(assist.original.starts_with("Hi Sam, thanks for you're help."));
    assert!(assist.result.is_none());
    for junk in [json!({"action": "explode"}), json!({"action": 3})] {
        assert!(a.execute("ui.aiAssist", junk).is_err());
    }
    // Opening it, by the owner or by an agent, asked no provider anything.
    let ctx = egui::Context::default();
    a.logic(&ctx);
    assert!(calls.lock().unwrap().is_empty());
    assert!(a.ai.busy().is_none());
}

#[test]
fn a_selection_is_what_is_worked_on_and_accepting_replaces_it_alone() {
    let (mut a, _) = app_with(Ok("x"));
    ready(&mut a);
    draft(&mut a, "Keep this. Change that. Keep this too.");
    a.execute("compose.select", json!({"from": 11, "to": 23})).unwrap();
    a.execute("ui.aiAssist", json!({"action": "rewrite"})).unwrap();
    let assist = a.ai.assist.take().unwrap();
    assert_eq!((assist.from, assist.to, assist.original.as_str()), (11, 23, "Change that."));
    crate::ai_ui::accept(&mut a, &assist, "Changed!");
    let body = a.session.compose.as_ref().unwrap().body.plain();
    assert!(body.starts_with("Keep this. Changed! Keep this too."), "{body:?}");
    // Undo gives the owner's own words back.
    a.execute("compose.undo", json!({})).unwrap();
    assert!(a.session.compose.as_ref().unwrap().body.plain().starts_with("Keep this. Change that. Keep this too."));
}

#[test]
fn a_draft_changed_meanwhile_is_left_as_the_owner_has_it() {
    let (mut a, _) = app_with(Ok("x"));
    ready(&mut a);
    draft(&mut a, "First version.");
    a.execute("ui.aiAssist", json!({})).unwrap();
    let assist = a.ai.assist.take().unwrap();
    a.execute("compose.select", json!({"from": 0, "to": 5})).unwrap();
    a.execute("compose.type", json!({"text": "Second"})).unwrap();
    crate::ai_ui::accept(&mut a, &assist, "Suggestion");
    let body = a.session.compose.as_ref().unwrap().body.plain();
    assert!(body.starts_with("Second version.") && !body.contains("Suggestion"), "{body:?}");
}

#[test]
fn an_answer_and_a_failure_both_reach_the_window_and_neither_touches_the_draft() {
    for (answer, good) in [(Ok("A better line."), true), (Err(Failure::RateLimited), false)] {
        let (mut a, calls) = app_with(answer);
        ready(&mut a);
        draft(&mut a, "A line.");
        a.execute("ui.aiAssist", json!({"action": "rewrite"})).unwrap();
        // The owner's press of Generate, as the window does it.
        let assist = a.ai.assist.clone().unwrap();
        let (system, user) = mailcraft_model::ai::prompt(assist.action, &a.ui.ai, &assist.original, "");
        let call = Call {
            kind: mailcraft_model::ai::Kind::OpenAi,
            base: a.ui.ai.base(),
            model: a.ui.ai.model.clone(),
            key: None,
            key_name: a.ui.ai.key_name(),
            remember_key: false,
            job: Job::Chat { system, user },
        };
        a.ai.pending = Some((Purpose::Assist, (a.services.ai.as_ref().unwrap())(call)));
        crate::ai_ui::poll(&mut a);
        let result = a.ai.assist.as_ref().unwrap().result.clone().unwrap();
        assert_eq!(result.is_ok(), good);
        assert!(a.session.compose.as_ref().unwrap().body.plain().starts_with("A line."));
        let sent = calls.lock().unwrap();
        let Job::Chat { user, .. } = &sent[0].job else { panic!() };
        // Only the text asked about went: no address, no subject.
        assert!(user.contains("A line.") && !user.contains('@'));
    }
}

#[test]
fn a_test_that_passes_turns_help_on_and_keeps_the_typed_key_out_of_the_window() {
    let (mut a, _) = app_with(Ok("Thanks for your help yesterday."));
    a.execute("ui.ai", json!({"provider": "groq", "model": "m"})).unwrap();
    a.ai.key_field = "sk-secret".into();
    let (system, user) = mailcraft_model::ai::prompt(mailcraft_model::ai::Action::Grammar, &Default::default(), "x", "");
    let call = Call {
        kind: mailcraft_model::ai::Kind::OpenAi,
        base: a.ui.ai.base(),
        model: "m".into(),
        key: Some("sk-secret".into()),
        key_name: a.ui.ai.key_name(),
        remember_key: true,
        job: Job::Chat { system, user },
    };
    a.ai.pending = Some((Purpose::Test, (a.services.ai.as_ref().unwrap())(call)));
    crate::ai_ui::poll(&mut a);
    assert!(a.ui.ai.enabled && a.ui.ai.key_saved && a.ai.key_field.is_empty());
    assert_eq!(a.ai.note.as_ref().map(|n| n.0), Some(true));
    // The key is in neither the settings nor anything saved of the window.
    let saved = serde_json::to_string(&a.ui).unwrap();
    assert!(!saved.contains("sk-secret"));
}

#[test]
fn the_setup_shows_the_writing_page_only_once_help_is_on_and_offers_what_was_skipped() {
    let (mut a, _) = app_with(Ok("x"));
    a.begin_onboarding();
    a.execute("ui.onboarding", json!({"page": "ai"})).unwrap();
    // Skipping help goes past its preferences, and it waits in the card.
    assert_eq!(a.execute("ui.onboarding", json!({"skip": "ai"})).unwrap()["page"], "appearance");
    assert!(crate::onboarding::remaining(&a).contains(&Page::Ai));
    assert_eq!(Page::Appearance.back(&a), Some(Page::Ai));
    // With help on, its preferences come next, and nothing of it is left to do.
    ready(&mut a);
    assert_eq!(a.execute("ui.onboarding", json!({"next": "ai"})).unwrap()["page"], "writing");
    assert_eq!(a.execute("ui.onboarding", json!({"skip": "writing"})).unwrap()["page"], "appearance");
    assert!(!crate::onboarding::remaining(&a).contains(&Page::Ai) && !crate::onboarding::remaining(&a).contains(&Page::Writing));
}

#[test]
fn every_page_of_help_lays_out_light_and_dark_and_narrow() {
    for value in ["light", "dark"] {
        for width in [380.0, 1200.0] {
            let (mut a, _) = app_with(Ok("A better line."));
            a.execute("ui.theme", json!({"value": value})).unwrap();
            let mut harness = egui_kittest::Harness::builder()
                .with_accessibility_check(false)
                .allow_missing_glyphs()
                .with_size(egui::vec2(width, 800.0))
                .build_ui_state(
                    |ui, app: &mut MailApp| {
                        let ctx = ui.ctx().clone();
                        app.logic(&ctx);
                        app.ui(ui);
                    },
                    a,
                );
            for provider in ["ollama", "groq", "anthropic", "custom"] {
                harness.state_mut().execute("ui.ai", json!({"provider": provider})).unwrap();
                harness.state_mut().execute("ui.onboarding", json!({"page": "ai"})).unwrap();
                harness.run_steps(3);
            }
            ready(harness.state_mut());
            harness.state_mut().execute("ui.onboarding", json!({"page": "writing"})).unwrap();
            harness.run_steps(3);
            harness.state_mut().execute("ui.onboarding", json!({"page": "finish"})).unwrap();
            harness.run_steps(3);
            harness.state_mut().execute("ui.onboarding", json!({"done": true})).unwrap();
            harness.state_mut().execute("ui.backstage", json!({"page": "options"})).unwrap();
            harness.run_steps(3);
            harness.state_mut().execute("ui.backstage", json!({"value": false})).unwrap();
            draft(harness.state_mut(), "A line.");
            harness.state_mut().execute("ui.aiAssist", json!({"action": "custom"})).unwrap();
            harness.run_steps(3);
            harness.state_mut().ai.assist.as_mut().unwrap().result = Some(Ok("A better line.".into()));
            harness.run_steps(3);
            harness.state_mut().ai.assist.as_mut().unwrap().result = Some(Err("The server couldn't be reached.".into()));
            harness.run_steps(3);
        }
    }
}
