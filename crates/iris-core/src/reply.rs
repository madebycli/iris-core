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
}

impl Reply {
    pub fn new(kind: ReplyKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into(), speech: None, card: None, skill: String::new(), intent: String::new() }
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
