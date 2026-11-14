//! The contract every skill fulfils, and what the engine hands a skill to work with.

use crate::host::Host;
use crate::lang::{LangPack, Messages};
use crate::reply::Reply;
use crate::store::Store;
use crate::text::Utterance;
use jiff::Zoned;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::ops::Range;

/// A skill: one area the assistant is good at.
///
/// A skill owns its logic and nothing else. Its words — the patterns that recognise it, the
/// sentences it answers with, the examples the classifier is trained on — are data, one file
/// per language, returned by [`Skill::language`]. Adding a language to a skill is adding a file.
pub trait Skill: Send + Sync {
    /// TOML: id, capabilities, intents and the classifier labels that mean them. See [`Manifest`].
    fn manifest(&self) -> &'static str;
    /// TOML for one language, `None` when the skill does not speak it. See [`SkillLanguage`].
    fn language(&self, code: &str) -> Option<&'static str>;
    /// Carries out `intent`. `None` means "not mine after all": the engine tries the next candidate.
    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    /// Higher is asked first when patterns of several skills match.
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub capabilities: Vec<crate::host::Capability>,
    #[serde(default, rename = "intent")]
    pub intents: Vec<Intent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Intent {
    pub id: String,
    /// Labels of the trained classifier that mean this intent.
    #[serde(default)]
    pub acts: Vec<String>,
    /// A sentence that matches this intent's patterns is a new request even while a question is open,
    /// so it ends the question. Off for intents whose patterns also fit ordinary answers
    /// ("Einkaufsliste mitnehmen" as the title of a reminder is not a list request).
    #[serde(default = "yes")]
    pub interrupts: bool,
}

fn yes() -> bool {
    true
}

/// A skill's words in one language, as written in `data/<skill>/<code>.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillLanguage {
    /// Tried in order against the normalised utterance. Named groups become [`Slots`].
    #[serde(default, rename = "pattern")]
    pub patterns: Vec<PatternSource>,
    #[serde(default)]
    pub messages: Messages,
    /// Word lists the skill's code asks for by name.
    #[serde(default)]
    pub words: HashMap<String, Vec<String>>,
    /// Regular expressions the skill's code asks for by name. Checked when the skill is registered.
    #[serde(default)]
    pub expressions: HashMap<String, String>,
    /// Named word-to-word tables ("einkaufs" is the list "Einkauf").
    #[serde(default)]
    pub tables: HashMap<String, HashMap<String, String>>,
    /// Replacements applied to text the skill takes from the user (a title), in order.
    #[serde(default, rename = "rewrite")]
    pub rewrites: Vec<RewriteSource>,
    /// Sentence templates per intent for the classifier's training data. Not used at run time.
    #[serde(default)]
    pub training: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatternSource {
    pub intent: String,
    pub re: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RewriteSource {
    pub re: String,
    pub to: String,
}

pub struct Pattern {
    pub intent: String,
    pub re: Regex,
}

#[derive(Debug)]
pub enum SkillError {
    Manifest(String),
    Language { skill: String, code: String, error: String },
    Pattern { skill: String, code: String, re: String, error: String },
    UnknownIntent { skill: String, intent: String },
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillError::Manifest(e) => write!(f, "manifest: {e}"),
            SkillError::Language { skill, code, error } => write!(f, "{skill}/{code}: {error}"),
            SkillError::Pattern { skill, code, re, error } => write!(f, "{skill}/{code}: pattern {re:?}: {error}"),
            SkillError::UnknownIntent { skill, intent } => write!(f, "{skill}: a pattern names the intent {intent:?}, which the manifest does not declare"),
        }
    }
}

impl std::error::Error for SkillError {}

/// Parts of the utterance a pattern named, as token ranges.
#[derive(Debug, Clone, Default)]
pub struct Slots(pub HashMap<String, Range<usize>>);

impl Slots {
    pub fn get(&self, name: &str) -> Option<Range<usize>> {
        self.0.get(name).cloned()
    }
}

/// Everything a skill may touch while it handles one utterance.
pub struct Cx<'a> {
    pub utt: &'a Utterance,
    pub slots: Slots,
    pub lang: &'a LangPack,
    /// The skill's own texts in the user's language.
    pub messages: &'a Messages,
    pub words: &'a HashMap<String, Vec<String>>,
    pub expressions: &'a HashMap<String, Regex>,
    pub rewrites: &'a [(Regex, String)],
    pub tables: &'a HashMap<String, HashMap<String, String>>,
    pub(crate) memory: &'a mut HashMap<String, (String, i64)>,
    pub(crate) skill_id: &'a str,
    pub host: &'a dyn Host,
    pub store: &'a dyn Store,
    pub now: Zoned,
    /// What the skill left with its question, when this utterance is the answer. `Null` otherwise.
    pub state: serde_json::Value,
    pub(crate) ask: Option<(String, serde_json::Value)>,
    pub(crate) ids: &'a mut u64,
    pub(crate) restore: Vec<String>,
}

impl Cx<'_> {
    /// Park a question: the next utterance comes back to this skill as `intent`, with `state`.
    pub fn ask(&mut self, intent: &str, state: serde_json::Value) {
        self.ask = Some((intent.to_string(), state));
    }

    pub fn msg(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.messages.get(key, args)
    }

    /// "Rückgängig" should also hand `token` back to [`Host::restore`] (a change to the phone, not to the store).
    pub fn undoable(&mut self, token: String) {
        self.restore.push(token);
    }

    /// A word list from the skill's language file; empty when the file has none of that name.
    pub fn list(&self, name: &str) -> &[String] {
        self.words.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn expr(&self, name: &str) -> Option<&Regex> {
        self.expressions.get(name)
    }

    /// A value from one of the language file's tables.
    pub fn table(&self, table: &str, key: &str) -> Option<&str> {
        self.tables.get(table)?.get(key).map(String::as_str)
    }

    /// Keeps a small fact for later turns ("the list talked about last"). Each skill has its own.
    pub fn remember(&mut self, key: &str, value: &str) {
        let now = self.now_ms();
        self.memory.insert(format!("{}.{key}", self.skill_id), (value.to_string(), now));
    }

    /// A remembered fact, if it is no older than `max_age_ms`.
    pub fn recall(&self, key: &str, max_age_ms: i64) -> Option<String> {
        let (value, at) = self.memory.get(&format!("{}.{key}", self.skill_id))?;
        (self.now_ms() - at <= max_age_ms).then(|| value.clone())
    }

    /// `text` after the language's rewrites ("rauszubringen" becomes "rausbringen").
    pub fn rewrite(&self, text: &str) -> String {
        self.rewrites.iter().fold(text.to_string(), |t, (re, to)| re.replace_all(&t, to.as_str()).into_owned())
    }

    pub fn now_ms(&self) -> i64 {
        self.now.timestamp().as_millisecond()
    }

    pub fn new_id(&mut self) -> String {
        *self.ids += 1;
        format!("{:x}-{:x}", self.now_ms(), *self.ids)
    }

    /// The tokens of a slot, or of the whole utterance when the pattern named no such slot.
    pub fn span(&self, slot: &str) -> Range<usize> {
        self.slots.get(slot).unwrap_or(0..self.utt.len())
    }
}
