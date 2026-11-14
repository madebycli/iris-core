//! The skills Iris ships with.
//!
//! Each skill is a Rust type for the logic and a directory `data/<skill>/` for everything
//! else: `skill.toml` (intents, capabilities) and one file per language (patterns, word lists,
//! replies, training templates). To teach a skill a language, add a file; to add a skill, add a
//! type, a directory and one line in [`all`].

mod alarm;
mod clock;
mod lists;
mod notes;
mod reminder;
mod timer;

use iris_core::{Engine, Host, Skill, SkillError, Store, Utterance};
use std::sync::Arc;

pub use alarm::Alarm;
pub use clock::Clock;
pub use lists::Lists;
pub use notes::Notes;
pub use reminder::Reminder;
pub use timer::Timer;

/// Every built-in skill.
pub fn all() -> Vec<Box<dyn Skill>> {
    vec![Box::new(Reminder), Box::new(Lists), Box::new(Notes), Box::new(Timer), Box::new(Alarm), Box::new(Clock)]
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
