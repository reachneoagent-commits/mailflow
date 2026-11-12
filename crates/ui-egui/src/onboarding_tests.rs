// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow's setup: when it shows, the command, `ui.json`, and that it lays out.

use mailcraft_engine::Session;
use serde_json::{Value, json};

use crate::onboarding::Page;
use crate::theme::ColorTheme;
use crate::{MailApp, Services, UiState};

const NOW: i64 = 1_791_993_600;

fn app() -> MailApp {
    let mut a = MailApp::new(Session::at(mailcraft_engine::sample::mailbox(NOW, 0), NOW, 0), Services::default());
    a.autosave = false;
    a
}

#[test]
fn only_a_first_start_shows_the_setup() {
    let mut a = app();
    assert!(a.ui.onboarded);
    a.begin_onboarding();
    assert!(!a.ui.onboarded);
    assert_eq!(a.onboarding_page, Page::Welcome);
    // Settings from before the setup existed, or with something else there, don't bring it up.
    let old: UiState = serde_json::from_value(json!({"theme": "dark"})).unwrap();
    assert!(old.onboarded);
    for junk in [json!("no"), json!(0), json!(null), json!([false]), json!({"a": 1})] {
        let bad: UiState = serde_json::from_value(json!({"onboarded": junk})).unwrap();
        assert!(bad.onboarded);
    }
    // One left half way is taken up again at the next start.
    let saved: Value = serde_json::to_value(&a.ui).unwrap();
    assert_eq!(saved["onboarded"], false);
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert!(!back.onboarded);
}

#[test]
fn the_command_opens_a_page_closes_the_setup_and_refuses_anything_else() {
    let mut a = app();
    let opened = a.execute("ui.onboarding", json!({})).unwrap();
    assert_eq!((opened["open"].clone(), opened["page"].clone()), (json!(true), json!("welcome")));
    assert!(!a.ui.onboarded);
    for page in Page::ALL {
        assert_eq!(a.execute("ui.onboarding", json!({"page": page.code()})).unwrap()["page"], page.code());
        assert_eq!(a.onboarding_page, page);
    }
    for junk in [
        json!(null),
        json!([]),
        json!("welcome"),
        json!({"page": 3}),
        json!({"page": "aix"}),
        json!({"page": null}),
        json!({"done": false}),
        json!({"done": "yes"}),
        json!({"done": true, "page": "email"}),
        json!({"skip": "welcome"}),
        json!({"skip": "finish"}),
        json!({"skip": 1}),
        json!({"next": "finish"}),
        json!({"next": null}),
        json!({"reminder": "no"}),
        json!({"skip": "email", "next": "email"}),
    ] {
        assert!(a.execute("ui.onboarding", junk).is_err());
    }
    assert_eq!((a.ui.onboarded, a.onboarding_page), (false, Page::Finish));
    assert!(a.ui.setup_skipped.is_empty());
    a.offer_setup = true;
    assert_eq!(a.execute("ui.onboarding", json!({"done": true})).unwrap()["open"], false);
    assert!(a.ui.onboarded && !a.offer_setup);
    assert_eq!(a.onboarding_page, Page::Welcome);
}

fn empty() -> MailApp {
    let mut a = MailApp::new(Session::at(mailcraft_model::Mailbox::default(), NOW, 0), Services::default());
    a.autosave = false;
    a
}

#[test]
fn skip_skips_its_own_section_and_goes_on_to_the_next() {
    let mut a = empty();
    a.begin_onboarding();
    a.execute("ui.onboarding", json!({"page": "email"})).unwrap();
    // Skipping email goes on to the next section: the setup stays up.
    let said = a.execute("ui.onboarding", json!({"skip": "email"})).unwrap();
    assert_eq!((said["open"].clone(), said["page"].clone()), (json!(true), json!("ai")));
    assert_eq!(a.ui.setup_skipped, ["email"]);
    // Skipping twice is remembered once.
    a.execute("ui.onboarding", json!({"page": "email"})).unwrap();
    a.execute("ui.onboarding", json!({"skip": "email"})).unwrap();
    assert_eq!(a.ui.setup_skipped, ["email"]);
    // Skipping help with writing goes past its preferences to the look.
    assert_eq!(a.execute("ui.onboarding", json!({"skip": "ai"})).unwrap()["page"], "appearance");
    // Appearance done: it is not among what is left; the two that were skipped are.
    let said = a.execute("ui.onboarding", json!({"next": "appearance"})).unwrap();
    assert_eq!((said["page"].clone(), said["remaining"].clone()), (json!("finish"), json!(["email", "ai"])));
    assert_eq!(crate::onboarding::remaining(&a), [Page::Email, Page::Ai]);
}

#[test]
fn what_was_skipped_is_offered_again_until_it_is_done_or_the_card_is_closed() {
    let mut a = app();
    a.begin_onboarding();
    a.execute("ui.onboarding", json!({"skip": "appearance"})).unwrap();
    a.execute("ui.onboarding", json!({"done": true})).unwrap();
    // The sample mailbox has an account: only the look is left.
    assert_eq!(crate::onboarding::remaining(&a), [Page::Appearance]);
    assert!(!a.ui.setup_reminder_hidden);
    // Closing the card hides it and forgets nothing.
    assert_eq!(a.execute("ui.onboarding", json!({"reminder": false})).unwrap()["reminder"], false);
    assert!(a.ui.setup_reminder_hidden);
    assert_eq!(crate::onboarding::remaining(&a), [Page::Appearance]);
    // Skipping again brings the card back; doing the section empties it.
    a.execute("ui.onboarding", json!({"skip": "appearance"})).unwrap();
    assert!(!a.ui.setup_reminder_hidden);
    a.execute("ui.onboarding", json!({"page": "appearance"})).unwrap();
    a.execute("ui.onboarding", json!({"next": "appearance"})).unwrap();
    assert!(crate::onboarding::remaining(&a).is_empty());
    // A mailbox nobody set up always has its account left to connect, skipped or not.
    assert_eq!(crate::onboarding::remaining(&empty()), [Page::Email]);
}

#[test]
fn what_was_skipped_is_saved_and_anything_else_there_is_passed_over() {
    let ui = UiState { setup_skipped: vec!["appearance".into()], setup_reminder_hidden: true, ..UiState::default() };
    let saved: Value = serde_json::to_value(&ui).unwrap();
    assert_eq!((saved["setupSkipped"].clone(), saved["setupReminderHidden"].clone()), (json!(["appearance"]), json!(true)));
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert_eq!((back.setup_skipped, back.setup_reminder_hidden), (vec!["appearance".to_string()], true));
    let odd: UiState = serde_json::from_value(
        json!({"setupSkipped": ["email", 3, "welcome", "email", null, "nonsense", "appearance"], "setupReminderHidden": "yes"}),
    )
    .unwrap();
    assert_eq!((odd.setup_skipped, odd.setup_reminder_hidden), (vec!["email".to_string(), "appearance".to_string()], false));
    for junk in [json!("email"), json!(5), json!(null), json!({"a": 1})] {
        let bad: UiState = serde_json::from_value(json!({"theme": "dark", "setupSkipped": junk})).unwrap();
        assert!(bad.setup_skipped.is_empty());
        assert_eq!(bad.theme, crate::theme::Appearance::Dark);
    }
}

#[test]
fn add_account_waits_for_the_setup() {
    let mut a = empty();
    a.begin_onboarding();
    a.offer_setup = true;
    let ctx = egui::Context::default();
    a.logic(&ctx);
    assert!(a.dialog.is_none() && a.offer_setup);
}

#[test]
fn every_page_lays_out_in_every_theme_light_and_dark_and_narrow() {
    for theme in ColorTheme::ALL {
        for value in ["light", "dark"] {
            for width in [360.0, 1200.0] {
                let mut harness = egui_kittest::Harness::builder()
                    .with_accessibility_check(false)
                    .allow_missing_glyphs()
                    .with_size(egui::vec2(width, 760.0))
                    .build_ui_state(
                        |ui, app: &mut MailApp| {
                            let ctx = ui.ctx().clone();
                            app.logic(&ctx);
                            app.ui(ui);
                        },
                        app(),
                    );
                harness.state_mut().execute("ui.colorTheme", json!({"theme": theme.code()})).unwrap();
                harness.state_mut().execute("ui.theme", json!({"value": value})).unwrap();
                for page in Page::ALL {
                    harness.state_mut().execute("ui.onboarding", json!({"page": page.code()})).unwrap();
                    harness.run_steps(3);
                }
                // And the card of what is left, in the folder pane.
                harness.state_mut().execute("ui.onboarding", json!({"skip": "appearance"})).unwrap();
                harness.state_mut().execute("ui.onboarding", json!({"done": true})).unwrap();
                harness.run_steps(3);
            }
        }
    }
}
