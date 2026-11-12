// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Interface text. Every string the window shows goes through [`t`] (the `tl!` macro) or, for a
//! count, [`tn`] (`tln!`), keyed by its English text and looked up in the catalogue of the
//! window's language (`mailcraft-i18n`). The window's language is set once a frame from the
//! owner's choice ([`set_current`]); the engine is told it through `Session::interface_language`.

use std::sync::atomic::{AtomicU8, Ordering};

pub use mailcraft_i18n::{AUTO, Lang, fmt};

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The region the window's dates are written for (`GB`), with the language ([`dated`], #978).
static REGION: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The region the window's dates are written for from now on (`mailcraft_i18n::date::region_of`
/// the system's regional format).
pub fn set_region(region: Option<&str>) {
    let _held = hold_language();
    store_region(region);
}

/// What the window says dates in: its language, the way of its region.
pub fn dated() -> mailcraft_i18n::date::Dated {
    let _held = hold_language();
    let region = REGION.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    mailcraft_i18n::date::Dated::new(load_current(), region.as_deref())
}

fn store_region(region: Option<&str>) {
    let mut held = REGION.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.as_deref() != region {
        *held = region.map(str::to_string);
    }
}

#[cfg(test)]
fn load_region() -> Option<String> {
    REGION.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// The window's language from now on.
pub fn set_current(lang: Lang) {
    let _held = hold_language();
    store_current(lang);
}

/// The window's language.
pub fn current() -> Lang {
    let _held = hold_language();
    load_current()
}

fn store_current(lang: Lang) {
    let i = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    CURRENT.store(u8::try_from(i).unwrap_or(0), Ordering::Relaxed);
}

fn load_current() -> Lang {
    Lang::ALL.get(usize::from(CURRENT.load(Ordering::Relaxed))).copied().unwrap_or_default()
}

/// In tests the language is one value for the whole process. A test that changes it holds this
/// lock for the whole change, so a test running alongside waits instead of reading the other
/// language, and the previous language is put back when the guard goes, including when an
/// assertion fails.
#[cfg(test)]
pub(crate) struct RestoreLanguage {
    previous: Lang,
    region: Option<String>,
    _hold: LanguageHold,
}

#[cfg(test)]
impl RestoreLanguage {
    pub(crate) fn hold() -> Self {
        let hold = LanguageHold::acquire();
        Self { previous: load_current(), region: load_region(), _hold: hold }
    }
}

#[cfg(test)]
impl Drop for RestoreLanguage {
    fn drop(&mut self) {
        store_current(self.previous);
        store_region(self.region.as_deref());
    }
}

#[cfg(test)]
std::thread_local! {
    static LANGUAGE_HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct LanguageHold {
    /// Held for the whole change. Read nowhere: the lock is the point.
    _guard: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl LanguageHold {
    fn acquire() -> Self {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let guard = LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        LANGUAGE_HELD.with(|held| held.set(true));
        Self { _guard: guard }
    }
}

#[cfg(test)]
impl Drop for LanguageHold {
    fn drop(&mut self) {
        LANGUAGE_HELD.with(|held| held.set(false));
    }
}

#[cfg(test)]
fn hold_language() -> Option<LanguageHold> {
    if LANGUAGE_HELD.with(|held| held.get()) { None } else { Some(LanguageHold::acquire()) }
}

#[cfg(not(test))]
struct LanguageHold;

#[cfg(not(test))]
fn hold_language() -> LanguageHold {
    LanguageHold
}

/// `s` in the interface language.
pub fn t(s: &str) -> &str {
    branded(current().t(s))
}

/// Mailflow: the name the program was written under, as the interface's texts have it.
const UPSTREAM: &str = "MailCraft";

/// Mailflow: the name the window says instead.
pub const BRAND: &str = "Mailflow";

/// Mailflow: whether the window says [`BRAND`]. Off until a host turns it on ([`use_brand`]), so
/// that upstream's tests, which read upstream's texts, run on them unchanged.
static BRANDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Mailflow: have the window say the program's name as [`BRAND`] from now on. The hosts (the
/// desktop program, the web one, the screenshot example) call it once at their start.
pub fn use_brand(on: bool) {
    BRANDED.store(on, Ordering::Relaxed);
}

/// Mailflow: the program's name as the window says it.
pub fn name() -> &'static str {
    if BRANDED.load(Ordering::Relaxed) { BRAND } else { UPSTREAM }
}

/// Mailflow: what follows the upstream name where it means the upstream project and its people
/// (who a problem report goes to, who wrote the program), not this program: there it stays.
const UPSTREAM_ITSELF: [&str; 4] = [" team", "'s makers", " contributors", " project"];

/// Mailflow: `s` with the program's name as this build has it. Every text the window shows comes
/// through here ([`t`], [`tc`], [`tn`]), so the texts themselves stay upstream's, in every
/// language, and merge with upstream's changes.
pub fn brand_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find(UPSTREAM) {
        let (before, from) = rest.split_at(at);
        let after = from.get(UPSTREAM.len()..).unwrap_or("");
        out.push_str(before);
        out.push_str(if UPSTREAM_ITSELF.iter().any(|kept| after.starts_with(kept)) { UPSTREAM } else { BRAND });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Mailflow: [`brand_text`] for a text that lives as long as the program (a catalogue's entry or
/// a literal): each is rewritten once and kept. The texts are the interface's own, so there are
/// only so many; past [`BRANDED_MAX`] of them a text is shown as it is rather than kept.
fn branded(s: &str) -> &str {
    const BRANDED_MAX: usize = 4096;
    static KEPT: std::sync::Mutex<Option<std::collections::HashMap<String, &'static str>>> = std::sync::Mutex::new(None);
    if !BRANDED.load(Ordering::Relaxed) || !s.contains(UPSTREAM) {
        return s;
    }
    let mut held = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let kept = held.get_or_insert_with(std::collections::HashMap::new);
    if let Some(done) = kept.get(s) {
        return done;
    }
    if kept.len() >= BRANDED_MAX {
        return s;
    }
    let done: &'static str = Box::leak(brand_text(s).into_boxed_str());
    kept.insert(s.to_string(), done);
    done
}

/// `s` in `context` (`msgctxt`), in the interface language.
pub fn tc<'a>(context: &str, s: &'a str) -> &'a str {
    branded(current().tc(context, s))
}

/// The form for a count of `n` of `one` / `other` (English singular and plural), in the interface
/// language, with `{n}` filled in (other placeholders are left to [`fmt`]).
pub fn tn(n: u64, one: &str, other: &str) -> String {
    let text = fmt(current().tn(n, one, other), &[("n", &n.to_string())]);
    if BRANDED.load(Ordering::Relaxed) && text.contains(UPSTREAM) { brand_text(&text) } else { text }
}

/// A number of things, for [`tn`]: any integer (a negative one counts as none).
pub trait Count {
    fn count(self) -> u64;
}

macro_rules! count {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn count(self) -> u64 {
                u64::try_from(self).unwrap_or(0)
            }
        }
    )*};
}
count!(u8, u16, u32, u64, usize, i32, i64, isize);

/// What didn't happen and then why, as one sentence: `heading` ("Your automatic replies weren't
/// changed") and `reason` ("you are working offline.") are each translated by themselves, so a
/// heading is written once for all its reasons (#912), and the pattern that joins them is
/// translated too, so a language puts its own spacing and punctuation between them (French
/// `{heading} : {reason}`). Its context is what a translator sees beside it.
pub fn with_reason(heading: &str, reason: &str) -> String {
    fmt(crate::tlc!("what didn't happen, then why", "{heading}: {reason}"), &[("heading", heading), ("reason", reason)])
}

/// A command's location (`Home › Respond`) in the interface language, part by part.
pub fn location(loc: &str) -> String {
    mailcraft_i18n::location(current(), loc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_by_default() {
        // (The window's language is shared by the tests running alongside: only English here.)
        assert_eq!(t("Reply"), "Reply");
        assert_eq!(tn(1, "{n} message", "{n} messages"), "1 message");
        assert_eq!(tn(3, "{n} message", "{n} messages"), "3 messages");
        assert_eq!(location("Home › Respond"), "Home › Respond");
    }
}
