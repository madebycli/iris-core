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

/// Wraps a store and remembers what each entry looked like before this wrapper first changed it,
/// so a whole turn can be taken back.
pub struct RecordingStore<'a> {
    inner: &'a dyn Store,
    before: Mutex<Vec<(String, Option<Entry>)>>,
}

impl<'a> RecordingStore<'a> {
    pub fn new(inner: &'a dyn Store) -> Self {
        Self { inner, before: Mutex::new(Vec::new()) }
    }

    fn note(&self, id: &str) {
        let mut before = self.before.lock().unwrap();
        if !before.iter().any(|(i, _)| i == id) {
            before.push((id.to_string(), self.inner.get(id)));
        }
    }

    /// The entries as they were before the turn, oldest change first. Empty when nothing changed.
    pub fn into_journal(self) -> Vec<(String, Option<Entry>)> {
        self.before.into_inner().unwrap()
    }
}

impl Store for RecordingStore<'_> {
    fn all(&self, kind: Option<EntryKind>) -> Vec<Entry> {
        self.inner.all(kind)
    }
    fn get(&self, id: &str) -> Option<Entry> {
        self.inner.get(id)
    }
    fn put(&self, entry: Entry) {
        self.note(&entry.id);
        self.inner.put(entry);
    }
    fn delete(&self, id: &str) -> bool {
        self.note(id);
        self.inner.delete(id)
    }
}
