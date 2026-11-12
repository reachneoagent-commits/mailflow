// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow: the program's name in the interface's texts.

use crate::i18n::brand_text;

#[test]
fn the_programs_name_is_mailflow_wherever_a_text_names_the_program() {
    assert_eq!(brand_text("About MailCraft"), "About Mailflow");
    assert_eq!(brand_text("MailCraft can't reach the server. Quit MailCraft?"), "Mailflow can't reach the server. Quit Mailflow?");
    assert_eq!(brand_text("MailCraft's own record"), "Mailflow's own record");
    assert_eq!(brand_text("Nothing to change"), "Nothing to change");
    assert_eq!(brand_text(""), "");
    assert_eq!(brand_text("MailCraft"), "Mailflow");
    assert_eq!(brand_text("é MailCraft ü"), "é Mailflow ü");
}

#[test]
fn the_upstream_project_and_its_people_keep_their_name() {
    assert_eq!(
        brand_text("Sends the report to the MailCraft team, as this window shows it."),
        "Sends the report to the MailCraft team, as this window shows it."
    );
    assert_eq!(brand_text("Tell MailCraft's makers what went wrong."), "Tell MailCraft's makers what went wrong.");
    assert_eq!(brand_text("MailCraft sent nothing to the MailCraft team."), "Mailflow sent nothing to the MailCraft team.");
    assert_eq!(brand_text("Copyright the MailCraft contributors"), "Copyright the MailCraft contributors");
}

#[test]
fn upstreams_texts_are_what_the_tests_read() {
    // No host has turned the brand on here: the window's texts are upstream's.
    assert_eq!(crate::i18n::name(), "MailCraft");
    assert_eq!(crate::i18n::t("About MailCraft"), "About MailCraft");
}
