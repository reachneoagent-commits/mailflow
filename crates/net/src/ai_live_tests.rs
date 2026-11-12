// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow: help with writing against a real model server on this machine. Ignored: run by hand
//! with `MAILFLOW_AI_MODEL` (and `MAILFLOW_AI_BASE`, default Ollama's) when one is running.

use mailcraft_model::ai::{Action, Call, Job, Kind, Outcome, Settings, prompt};

#[test]
#[ignore = "needs a model server running on this machine"]
fn a_model_on_this_machine_lists_its_models_and_tidies_a_sentence() {
    let base = std::env::var("MAILFLOW_AI_BASE").unwrap_or_else(|_| "http://127.0.0.1:11434/v1".into());
    let model = std::env::var("MAILFLOW_AI_MODEL").expect("MAILFLOW_AI_MODEL");
    let call =
        |job| Call { kind: Kind::OpenAi, base: base.clone(), model: model.clone(), key: None, key_name: String::new(), remember_key: false, job };
    let Ok(Outcome::Models(models)) = super::ai::run(&call(Job::Models), "") else { panic!("no list of models") };
    assert!(models.contains(&model), "{models:?}");
    let (system, user) = prompt(Action::Grammar, &Settings::default(), "Thanks for you're help yesterday, it went really good.", "");
    let got = super::ai::run(&call(Job::Chat { system, user }), "");
    let Ok(Outcome::Text(text)) = got else { panic!("{got:?}") };
    println!("the model wrote: {text}");
    assert!(!text.is_empty() && !text.contains("text_to_work_on"));
}

#[test]
#[ignore = "needs a model server running on this machine"]
fn a_server_on_this_machine_answers_with_its_list_and_a_missing_model_is_said() {
    let base = std::env::var("MAILFLOW_AI_BASE").unwrap_or_else(|_| "http://127.0.0.1:11434/v1".into());
    let call = |job| Call {
        kind: Kind::OpenAi,
        base: base.clone(),
        model: "no-such-model-here".into(),
        key: None,
        key_name: String::new(),
        remember_key: false,
        job,
    };
    let got = super::ai::run(&call(Job::Models), "");
    println!("models: {got:?}");
    assert!(matches!(got, Ok(Outcome::Models(_))));
    let (system, user) = prompt(Action::Grammar, &Settings::default(), "Helo.", "");
    let got = super::ai::run(&call(Job::Chat { system, user }), "");
    println!("chat with a model it doesn't have: {got:?}");
    assert!(got.is_err());
    // And a server that isn't there is unreachable, not a hang.
    let nowhere = Call { base: "http://127.0.0.1:9".into(), ..call(Job::Models) };
    assert_eq!(super::ai::run(&nowhere, ""), Err(mailcraft_model::ai::Failure::Unreachable));
}
