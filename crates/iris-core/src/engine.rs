//! The pipeline: one utterance in, one reply out.
//!
//! 1. An open question goes back to the skill that asked it (a cancel word ends it, in any skill).
//! 2. Grammar: the skills' own patterns, exact and free of any model.
//! 3. Classifier: the trained network names an act, and the skill that declared the act runs.
//! 4. Otherwise the reply is [`ReplyKind::Unhandled`] and the app may ask its language model.
//!
//! A skill can decline at every step by returning `None`; the next candidate is then tried.

use crate::entry::EntryKind;
use crate::host::Host;
use crate::lang::{LangPack, Messages};
use crate::reply::{Reply, ReplyKind};
use crate::skill::{Cx, Manifest, Pattern, Skill, SkillError, SkillLanguage, Slots};
use crate::store::Store;
use crate::text::Utterance;
use crate::time::next_occurrence;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    pub label: String,
    pub p: f32,
}

/// The trained network, seen from the engine: text in, acts out, most likely first.
pub trait Classifier: Send + Sync {
    fn classify(&self, text: &str, lang: &str) -> Vec<Prediction>;
}

/// Grammar only. Used until a network is plugged in, and in tests of the grammar itself.
pub struct NoClassifier;

impl Classifier for NoClassifier {
    fn classify(&self, _text: &str, _lang: &str) -> Vec<Prediction> {
        Vec::new()
    }
}

struct Loaded {
    patterns: Vec<Pattern>,
    messages: Messages,
    words: HashMap<String, Vec<String>>,
    expressions: HashMap<String, Regex>,
    rewrites: Vec<(Regex, String)>,
    tables: HashMap<String, HashMap<String, String>>,
}

struct Registered {
    skill: Box<dyn Skill>,
    manifest: Manifest,
    langs: HashMap<String, Loaded>,
}

struct Pending {
    skill: usize,
    intent: String,
    state: serde_json::Value,
    at_ms: i64,
}

pub struct Engine {
    langs: HashMap<String, Arc<LangPack>>,
    skills: Vec<Registered>,
    host: Arc<dyn Host>,
    store: Arc<dyn Store>,
    classifier: Box<dyn Classifier>,
    pending: Option<Pending>,
    memory: HashMap<String, (String, i64)>,
    ids: u64,
    /// Below this the classifier's answer is not acted on.
    pub cutoff: f32,
    /// How long a question waits for its answer.
    pub question_ttl_ms: i64,
}

impl Engine {
    pub fn new(host: Arc<dyn Host>, store: Arc<dyn Store>) -> Self {
        let langs = LangPack::builtin().into_iter().map(|p| (p.code.clone(), Arc::new(p))).collect();
        Self { langs, skills: Vec::new(), host, store, classifier: Box::new(NoClassifier), pending: None, memory: HashMap::new(), ids: 0, cutoff: 0.60, question_ttl_ms: 120_000 }
    }

    pub fn add_language(&mut self, pack: LangPack) {
        self.langs.insert(pack.code.clone(), Arc::new(pack));
    }

    pub fn set_classifier(&mut self, classifier: Box<dyn Classifier>) {
        self.classifier = classifier;
    }

    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    pub fn has_question(&self) -> bool {
        self.pending.is_some()
    }

    /// Adds a skill. Its manifest and every language file are checked now, so a broken skill never reaches a user.
    pub fn register(&mut self, skill: Box<dyn Skill>) -> Result<(), SkillError> {
        let manifest: Manifest = toml::from_str(skill.manifest()).map_err(|e| SkillError::Manifest(e.to_string()))?;
        let mut langs = HashMap::new();
        for code in self.langs.keys() {
            let Some(source) = skill.language(code) else { continue };
            let parsed: SkillLanguage = toml::from_str(source).map_err(|e| SkillError::Language { skill: manifest.id.clone(), code: code.clone(), error: e.to_string() })?;
            let mut patterns = Vec::new();
            for p in parsed.patterns {
                if !manifest.intents.iter().any(|i| i.id == p.intent) {
                    return Err(SkillError::UnknownIntent { skill: manifest.id.clone(), intent: p.intent });
                }
                let re = Regex::new(&p.re).map_err(|e| SkillError::Pattern { skill: manifest.id.clone(), code: code.clone(), re: p.re.clone(), error: e.to_string() })?;
                patterns.push(Pattern { intent: p.intent, re });
            }
            let mut expressions = HashMap::new();
            for (name, source) in parsed.expressions {
                let re = Regex::new(&source).map_err(|e| SkillError::Pattern { skill: manifest.id.clone(), code: code.clone(), re: source.clone(), error: e.to_string() })?;
                expressions.insert(name, re);
            }
            let mut rewrites = Vec::new();
            for r in parsed.rewrites {
                let re = Regex::new(&r.re).map_err(|e| SkillError::Pattern { skill: manifest.id.clone(), code: code.clone(), re: r.re.clone(), error: e.to_string() })?;
                rewrites.push((re, r.to));
            }
            langs.insert(code.clone(), Loaded { patterns, messages: parsed.messages, words: parsed.words, expressions, rewrites, tables: parsed.tables });
        }
        self.skills.push(Registered { skill, manifest, langs });
        self.skills.sort_by_key(|r| -r.manifest.priority);
        Ok(())
    }

    /// Answers one utterance said or typed in the language `code`.
    pub fn process(&mut self, text: &str, code: &str) -> Reply {
        let Some(lang) = self.langs.get(code).or_else(|| self.langs.get("en")).cloned() else {
            return Reply::new(ReplyKind::Unhandled, "");
        };
        let utt = Utterance::new(text, &lang);
        let unhandled = || {
            let mut r = Reply::new(ReplyKind::Unhandled, lang.messages.get("not_understood", &[]));
            r.skill = "core".into();
            r
        };
        if utt.is_empty() {
            return unhandled();
        }
        let now_ms = self.host.now_ms();
        // 1. an open question
        if let Some(p) = self.pending.take() {
            if now_ms - p.at_ms <= self.question_ttl_ms {
                if lang.dialog.is_cancel(&utt.norm) {
                    let mut r = Reply::done(lang.messages.get("cancelled", &[]));
                    r.skill = "core".into();
                    r.intent = "cancel".into();
                    return r;
                }
                if let Some(r) = self.run(p.skill, &p.intent, &utt, Slots::default(), &lang, p.state) {
                    return r;
                }
            }
        }
        // 2. grammar
        let mut matches = Vec::new();
        for (index, reg) in self.skills.iter().enumerate() {
            let Some(loaded) = reg.langs.get(&lang.code) else { continue };
            for pattern in &loaded.patterns {
                if let Some(caps) = pattern.re.captures(&utt.norm) {
                    let slots = pattern.re.capture_names().flatten().filter_map(|name| caps.name(name).map(|m| (name.to_string(), utt.token_range(m.range())))).collect();
                    matches.push((index, pattern.intent.clone(), Slots(slots)));
                    break;
                }
            }
        }
        for (index, intent, slots) in matches {
            if let Some(r) = self.run(index, &intent, &utt, slots, &lang, serde_json::Value::Null) {
                return r;
            }
        }
        // 3. the classifier
        for prediction in self.classifier.classify(&utt.raw, &lang.code).into_iter().take(1) {
            if prediction.p < self.cutoff {
                break;
            }
            let target = self.skills.iter().enumerate().find_map(|(i, r)| r.manifest.intents.iter().find(|n| n.acts.contains(&prediction.label)).map(|n| (i, n.id.clone())));
            if let Some((index, intent)) = target {
                if let Some(r) = self.run(index, &intent, &utt, Slots::default(), &lang, serde_json::Value::Null) {
                    return r;
                }
            }
        }
        unhandled()
    }

    fn run(&mut self, index: usize, intent: &str, utt: &Utterance, slots: Slots, lang: &LangPack, state: serde_json::Value) -> Option<Reply> {
        let reg = &self.skills[index];
        let loaded = reg.langs.get(&lang.code)?;
        let now_ms = self.host.now_ms();
        let tz = TimeZone::get(&self.host.zone()).unwrap_or(TimeZone::UTC);
        let now = Timestamp::from_millisecond(now_ms).unwrap_or(Timestamp::UNIX_EPOCH).to_zoned(tz);
        let mut cx = Cx { utt, slots, lang, messages: &loaded.messages, words: &loaded.words, expressions: &loaded.expressions, rewrites: &loaded.rewrites, tables: &loaded.tables, memory: &mut self.memory, skill_id: &reg.manifest.id, host: self.host.as_ref(), store: self.store.as_ref(), now, state, ask: None, ids: &mut self.ids };
        let mut reply = reg.skill.handle(intent, &mut cx)?;
        reply.skill = reg.manifest.id.clone();
        reply.intent = intent.to_string();
        if let Some((next, state)) = cx.ask.take() {
            reply.kind = ReplyKind::Question;
            self.pending = Some(Pending { skill: index, intent: next, state, at_ms: now_ms });
        }
        Some(reply)
    }

    /// The app reports that a reminder or timer went off. A repeating entry is planned again; the others are finished.
    pub fn fired(&mut self, id: &str) {
        let Some(mut entry) = self.store.get(id) else { return };
        let now = Timestamp::from_millisecond(self.host.now_ms()).unwrap_or(Timestamp::UNIX_EPOCH);
        let next = entry.due_at.and_then(|d| Timestamp::from_millisecond(d).ok()).and_then(|due| next_occurrence(due, &entry.repeat, &entry.zone, now));
        match next {
            Some(at) => {
                entry.due_at = Some(at.as_millisecond());
                entry.pending = true;
                let _ = self.host.schedule(&entry);
            }
            None => entry.pending = false,
        }
        if entry.kind == EntryKind::Timer && !entry.pending {
            self.store.delete(id);
        } else {
            self.store.put(entry);
        }
    }

    /// Languages the engine can be spoken to in.
    pub fn languages(&self) -> Vec<String> {
        let mut codes: Vec<String> = self.langs.keys().cloned().collect();
        codes.sort();
        codes
    }
}
