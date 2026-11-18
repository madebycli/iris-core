//! What the engine hands back: words to show, words to speak, and something to draw.

use crate::entry::Entry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyKind {
    /// A skill did what was asked.
    Done,
    /// A skill needs an answer; the microphone should open again.
    Question,
    /// A skill understood but could not do it.
    Failed,
    /// No skill claimed the utterance: the app may hand it to its language model.
    Unhandled,
}

/// Something the app draws next to the text. The engine says what, the app decides how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Card {
    Entries { title: String, items: Vec<Entry> },
    Timer { entry: Entry },
}

/// A step of a plan only the app can carry out: its action id and the values the plan checked. The app runs it with
/// the executor it has and reports back (`Engine::plan_continue`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCall {
    pub action: String,
    pub args: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub kind: ReplyKind,
    pub text: String,
    /// Spoken instead of `text` when the two differ.
    pub speech: Option<String>,
    pub card: Option<Card>,
    /// Which skill answered, for logs and tests.
    pub skill: String,
    pub intent: String,
    /// Set when a running plan has reached a step only the app can carry out. The app runs it and calls
    /// `Engine::plan_continue` with the outcome; until then the rest of the plan waits in the core.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<HostCall>,
    /// How many validated steps of a plan are still waiting or were not carried out.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub left: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Reply {
    pub fn new(kind: ReplyKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into(), speech: None, card: None, skill: String::new(), intent: String::new(), host: None, left: 0 }
    }
    pub fn done(text: impl Into<String>) -> Self {
        Self::new(ReplyKind::Done, text)
    }
    pub fn question(text: impl Into<String>) -> Self {
        Self::new(ReplyKind::Question, text)
    }
    pub fn failed(text: impl Into<String>) -> Self {
        Self::new(ReplyKind::Failed, text)
    }
    pub fn with_card(mut self, card: Card) -> Self {
        self.card = Some(card);
        self
    }
}
