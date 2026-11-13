//! Where entries live. The engine only knows this trait; the app supplies SQLite, tests use memory.

use crate::entry::{Entry, EntryKind};
use std::sync::Mutex;

pub trait Store: Send + Sync {
    /// All entries, optionally of one kind, oldest first.
    fn all(&self, kind: Option<EntryKind>) -> Vec<Entry>;
    fn get(&self, id: &str) -> Option<Entry>;
    /// Inserts, or replaces the entry with the same id.
    fn put(&self, entry: Entry);
    fn delete(&self, id: &str) -> bool;
}

#[derive(Default)]
pub struct MemoryStore(Mutex<Vec<Entry>>);

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn all(&self, kind: Option<EntryKind>) -> Vec<Entry> {
        self.0.lock().unwrap().iter().filter(|e| kind.is_none_or(|k| e.kind == k)).cloned().collect()
    }
    fn get(&self, id: &str) -> Option<Entry> {
        self.0.lock().unwrap().iter().find(|e| e.id == id).cloned()
    }
    fn put(&self, entry: Entry) {
        let mut all = self.0.lock().unwrap();
        match all.iter_mut().find(|e| e.id == entry.id) {
            Some(slot) => *slot = entry,
            None => all.push(entry),
        }
    }
    fn delete(&self, id: &str) -> bool {
        let mut all = self.0.lock().unwrap();
        let before = all.len();
        all.retain(|e| e.id != id);
        all.len() != before
    }
}
