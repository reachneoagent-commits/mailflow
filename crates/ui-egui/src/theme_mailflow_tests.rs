// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow's colour themes: the command, `ui.json`, and what the window shows.

use mailcraft_engine::Session;
use serde_json::{Value, json};

use crate::theme::{Accent, ColorTheme, Tokens};
use crate::{MailApp, Services, UiState};

const NOW: i64 = 1_791_993_600;

fn app() -> MailApp {
    let mut a = MailApp::new(Session::at(mailcraft_engine::sample::mailbox(NOW, 0), NOW, 0), Services::default());
    a.autosave = false;
    a
}

#[test]
fn the_brand_is_the_theme_of_a_new_window() {
    assert_eq!(UiState::default().color_theme, ColorTheme::Mailflow);
    assert_eq!(ColorTheme::Mailflow.tokens(false).accent, egui::Color32::from_rgb(0x47, 0x37, 0xFF));
    assert_eq!(ColorTheme::Mailflow.tokens(false).surface, egui::Color32::WHITE);
    assert_eq!(ColorTheme::Mailflow.tokens(true).ribbon, egui::Color32::from_rgb(0x20, 0x20, 0x30));
}

#[test]
fn every_theme_has_a_code_that_reads_back_and_two_palettes() {
    for theme in ColorTheme::ALL {
        assert_eq!(ColorTheme::from_code(theme.code()), Some(theme));
        assert!(!theme.tokens(false).dark);
        assert!(theme.tokens(true).dark);
        assert_ne!(theme.tokens(false).surface, theme.tokens(true).surface);
    }
}

#[test]
fn the_command_sets_the_theme_and_refuses_anything_else() {
    let mut a = app();
    assert_eq!(a.execute("ui.colorTheme", json!({"theme": "ember"})).unwrap()["theme"], "ember");
    assert_eq!(a.ui.color_theme, ColorTheme::Ember);
    // The accent follows, for high contrast.
    assert_eq!(a.ui.accent, Accent::Graphite);
    for junk in [
        json!({}),
        json!(null),
        json!([]),
        json!("x"),
        json!({"theme": 7}),
        json!({"theme": "pink"}),
        json!({"theme": ["ember"]}),
        json!({"theme": null}),
    ] {
        assert!(a.execute("ui.colorTheme", junk).is_err());
    }
    assert_eq!(a.ui.color_theme, ColorTheme::Ember);
}

#[test]
fn the_theme_is_saved_and_a_missing_or_unreadable_one_is_the_brand() {
    let ui = UiState { color_theme: ColorTheme::RosePine, ..UiState::default() };
    let saved: Value = serde_json::to_value(&ui).unwrap();
    assert_eq!(saved["colorTheme"], "rose-pine");
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(back.color_theme, ColorTheme::RosePine);
    let old: UiState = serde_json::from_value(json!({"theme": "dark", "accent": "teal"})).unwrap();
    assert_eq!((old.color_theme, old.accent), (ColorTheme::Mailflow, Accent::Teal));
    for junk in [json!("mauve"), json!(5), json!(null), json!([1]), json!({"a": 1})] {
        let bad: UiState = serde_json::from_value(json!({"theme": "dark", "colorTheme": junk})).unwrap();
        assert_eq!((bad.color_theme, bad.theme), (ColorTheme::Mailflow, crate::theme::Appearance::Dark));
    }
}

#[test]
fn the_window_shows_the_theme_in_light_and_in_dark() {
    let mut a = app();
    let ctx = egui::Context::default();
    for theme in ColorTheme::ALL {
        a.execute("ui.colorTheme", json!({"theme": theme.code()})).unwrap();
        for (value, dark) in [("light", false), ("dark", true)] {
            a.execute("ui.theme", json!({"value": value})).unwrap();
            a.logic(&ctx);
            let t = Tokens::get(&ctx);
            assert_eq!(
                (t.accent, t.surface, t.text),
                (theme.tokens(dark).accent, theme.tokens(dark).surface, theme.tokens(dark).text),
                "{}",
                theme.code()
            );
        }
    }
}
