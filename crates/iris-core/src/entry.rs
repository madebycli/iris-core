//! What Iris keeps for the user: reminders, timers, notes and list items.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntryKind {
    Reminder,
    Timer,
    Note,
    ListItem,
}

/// One stored thing. The fields and their meaning are those of the Android app's `entries` table, so existing data carries over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub kind: EntryKind,
    pub title: String,
    /// The list an item is on. Empty for everything else.
    #[serde(default)]
    pub collection: String,
    /// When it is due, in milliseconds since the epoch.
    #[serde(default)]
    pub due_at: Option<i64>,
    /// IANA zone the time was said in, so a repeating entry keeps its wall-clock time.
    #[serde(default)]
    pub zone: String,
    /// "", "daily", "weekdays", "weekly" or "days:1,3,5".
    #[serde(default)]
    pub repeat: String,
    #[serde(default = "yes")]
    pub pending: bool,
    #[serde(default)]
    pub created_at: i64,
    /// Timers: more than 0 means paused with this much left.
    #[serde(default)]
    pub remaining_ms: i64,
    /// Timers: the whole length.
    #[serde(default)]
    pub duration_ms: i64,
}

fn yes() -> bool {
    true
}

impl Entry {
    pub fn new(id: String, kind: EntryKind, title: impl Into<String>, now_ms: i64, zone: &str) -> Self {
        Self { id, kind, title: title.into(), collection: String::new(), due_at: None, zone: zone.to_string(), repeat: String::new(), pending: true, created_at: now_ms, remaining_ms: 0, duration_ms: 0 }
    }

    /// Time left on a timer at `now_ms`.
    pub fn left_ms(&self, now_ms: i64) -> i64 {
        if self.remaining_ms > 0 {
            self.remaining_ms
        } else {
            (self.due_at.unwrap_or(now_ms) - now_ms).max(0)
        }
    }
}
