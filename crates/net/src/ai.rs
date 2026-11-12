// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mailflow: asking a language model the owner chose for help with a text
//! (`mailcraft_model::ai`), once the owner has pressed the button for it: one request, and its
//! answer read.
//!
//! - **Only to the address the owner's settings name**, over HTTPS; plain `http` only to this
//!   machine, which is where a local model's server listens (the crate's HTTP client refuses it
//!   to anywhere else).
//! - **The key goes in a header**, never in the address, and is never logged; nor is anything of
//!   the text or of the answer. The log says which kind of job ran and how it ended.
//! - **The answer is hostile**: at most [`MAX_ANSWER`] bytes within the job's patience, no
//!   redirects, read by `mailcraft_model::ai::read`.
//! - **Asked once.** Nothing here tries again: the owner decides.

use std::time::Duration;

use mailcraft_model::ai::{Call, Failure, Job, Outcome, read, wire};

use crate::NetError;
use crate::http::{self, Request, Url};

/// Most bytes of an answer read: a long list of models, or a long text.
pub const MAX_ANSWER: usize = 2 * 1024 * 1024;

/// How long connecting may take, and so may the answer: a model on this computer can be slow to
/// load, and fetching one takes as long as the download.
fn patience(job: &Job) -> Duration {
    match job {
        Job::Models => Duration::from_secs(15),
        Job::Chat { .. } => Duration::from_secs(120),
        Job::Pull => Duration::from_secs(30 * 60),
    }
}

/// Run `call` with `key` (empty: none). Blocks for the job's patience twice over at most.
pub fn run(call: &Call, key: &str) -> Result<Outcome, Failure> {
    run_within(call, key, patience(&call.job))
}

pub(crate) fn run_within(call: &Call, key: &str, patience: Duration) -> Result<Outcome, Failure> {
    let wire = wire(call, key);
    let url = Url::parse(&wire.url).map_err(|_| Failure::Unreachable)?;
    let headers: Vec<(&str, &str)> = wire.headers.iter().map(|(name, value)| (*name, value.as_str())).collect();
    let request = Request { method: wire.method, headers: &headers, body: wire.body.as_deref().map(str::as_bytes), patience, max_body: MAX_ANSWER };
    let kind = match call.job {
        Job::Chat { .. } => "chat",
        Job::Models => "models",
        Job::Pull => "pull",
    };
    let got = match http::request(&url, &request) {
        Ok(reply) => read(call.kind, &call.job, reply.status, &reply.body),
        Err(e) if http::is_too_slow(&e) => Err(Failure::TooSlow),
        Err(NetError::Connect { .. } | NetError::Tls { .. } | NetError::Certificate(_) | NetError::Config(_)) => Err(Failure::Unreachable),
        Err(_) => Err(Failure::Garbled),
    };
    // How it ended, and nothing of what was asked or answered.
    match &got {
        Ok(_) => log::info!("writing help: {kind} done"),
        Err(Failure::Refused(_)) => log::info!("writing help: {kind} refused by the provider"),
        Err(why) => log::info!("writing help: {kind} failed: {why:?}"),
    }
    got
}
