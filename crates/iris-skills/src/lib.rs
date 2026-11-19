//! The skills Iris ships with.
//!
//! Each skill is a Rust type for the logic and a directory `data/<skill>/` for everything
//! else: `skill.toml` (intents, capabilities) and one file per language (patterns, word lists,
//! replies, training templates). To teach a skill a language, add a file; to add a skill, add a
//! type, a directory and one line in [`all`].

mod alarm;
mod clock;
mod convert;
mod device;
mod help;
mod lists;
mod notes;
mod reminder;
mod stopwatch;
mod timer;

use iris_core::{Engine, Host, Skill, SkillError, Store, Utterance};
use std::sync::Arc;

pub use alarm::Alarm;
pub use clock::Clock;
pub use convert::{convert, Convert};
pub use device::Device;
pub use help::Help;
pub use lists::Lists;
pub use notes::Notes;
pub use reminder::Reminder;
pub use stopwatch::Stopwatch;
pub use timer::Timer;

/// Every built-in skill.
pub fn all() -> Vec<Box<dyn Skill>> {
    vec![Box::new(Reminder), Box::new(Lists), Box::new(Notes), Box::new(Timer), Box::new(Alarm), Box::new(Clock), Box::new(Device), Box::new(Stopwatch), Box::new(Convert), Box::new(Help)]
}

/// An engine with all built-in skills registered.
pub fn engine(host: Arc<dyn Host>, store: Arc<dyn Store>) -> Result<Engine, SkillError> {
    let mut engine = Engine::new(host, store);
    for skill in all() {
        engine.register(skill)?;
    }
    Ok(engine)
}

/// How many tokens the longest of `phrases` covers at position `at` of `tokens` (indexes into `utt`).
fn phrase_len(utt: &Utterance, tokens: &[usize], at: usize, phrases: &[String]) -> usize {
    phrases
        .iter()
        .filter_map(|p| {
            let words: Vec<&str> = p.split(' ').collect();
            let fits = tokens.len() >= at + words.len() && words.iter().enumerate().all(|(k, w)| utt.tokens[tokens[at + k]].norm == *w);
            fits.then_some(words.len())
        })
        .max()
        .unwrap_or(0)
}

/// Removes `drop` phrases anywhere, then `lead` phrases from the front and `trail` phrases from the end until none is left.
pub(crate) fn strip(utt: &Utterance, mut tokens: Vec<usize>, drop: &[String], lead: &[String], trail: &[String]) -> Vec<usize> {
    let mut i = 0;
    while i < tokens.len() {
        match phrase_len(utt, &tokens, i, drop) {
            0 => i += 1,
            n => {
                tokens.drain(i..i + n);
            }
        }
    }
    loop {
        let n = phrase_len(utt, &tokens, 0, lead);
        if n == 0 {
            break;
        }
        tokens.drain(..n);
    }
    'outer: loop {
        for phrase in trail {
            let words: Vec<&str> = phrase.split(' ').collect();
            if tokens.len() >= words.len() && words.iter().enumerate().all(|(k, w)| utt.tokens[tokens[tokens.len() - words.len() + k]].norm == *w) {
                tokens.truncate(tokens.len() - words.len());
                continue 'outer;
            }
        }
        break;
    }
    tokens
}

pub(crate) fn join(utt: &Utterance, tokens: &[usize]) -> String {
    iris_core::text::capitalise(&tokens.iter().map(|i| utt.tokens[*i].orig.as_str()).collect::<Vec<_>>().join(" "))
}

/// The entry a sentence names: the one whose whole title was said (the longest when several), or else the
/// only one that shares the most words with it ("Lösche die Erinnerung Müll" names "Müll rausbringen").
/// Only words of four letters or more, or with a digit, count. `None` when no entry or several fit equally.
pub(crate) fn named<'a>(items: &'a [iris_core::Entry], said: &str) -> Option<&'a iris_core::Entry> {
    if let Some(whole) = items.iter().filter(|e| !e.title.is_empty() && said.contains(&e.title.to_lowercase())).max_by_key(|e| e.title.len()) {
        return Some(whole);
    }
    let words: Vec<&str> = said.split(' ').filter(|w| w.chars().count() >= 4 || w.chars().any(|c| c.is_ascii_digit())).collect();
    let score = |e: &iris_core::Entry| {
        let title = e.title.to_lowercase();
        title.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty() && words.contains(t)).count()
            // a word said in part of a compound counts too: "Zahn" names "Zahnarzt"
            + title.split(|c: char| !c.is_alphanumeric()).filter(|t| !words.contains(t) && said.split(' ').any(|w| w.chars().count() >= 4 && t.starts_with(w))).count()
    };
    let best = items.iter().map(score).max().filter(|n| *n > 0)?;
    let mut top = items.iter().filter(|e| score(e) == best);
    let one = top.next()?;
    top.next().is_none().then_some(one)
}

/// Why a typed action did not find its entry.
pub(crate) enum Miss {
    /// No entry has that title.
    None,
    /// Several entries fit: nothing is changed.
    Several,
}

/// The entry an action names by title: the one whose title is exactly `title` (any case), otherwise the only one that
/// contains it or is contained in it. A title that fits several entries is `Several`: a typed action never guesses.
pub(crate) fn by_title<'a>(items: &'a [iris_core::Entry], title: &str) -> Result<&'a iris_core::Entry, Miss> {
    let want = title.trim().to_lowercase();
    let exact: Vec<&iris_core::Entry> = items.iter().filter(|e| e.title.trim().to_lowercase() == want).collect();
    let pool: Vec<&iris_core::Entry> = if exact.is_empty() { items.iter().filter(|e| !want.is_empty() && e.title.to_lowercase().contains(&want)).collect() } else { exact };
    match pool.as_slice() {
        [] => Err(Miss::None),
        [one] => Ok(one),
        _ => Err(Miss::Several),
    }
}
