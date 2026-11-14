//! Entries in a SQLite file. The table is the one of the Android app (`entries`, version 3), so a
//! database written by either side can be read by the other.

use iris_core::{Entry, EntryKind, Store};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

pub struct SqliteStore(Mutex<Connection>);

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS entries(id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL, title TEXT NOT NULL, collection_name TEXT NOT NULL, due_at INTEGER, zone TEXT NOT NULL, repeat_rule TEXT NOT NULL, pending INTEGER NOT NULL, anchor_time TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL DEFAULT 0, remaining_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS entries_due ON entries(pending, due_at);";

const KINDS: [(EntryKind, &str); 4] = [(EntryKind::Reminder, "REMINDER"), (EntryKind::Timer, "TIMER"), (EntryKind::Note, "NOTE"), (EntryKind::ListItem, "LIST_ITEM")];

fn kind_name(kind: EntryKind) -> &'static str {
    KINDS.iter().find(|(k, _)| *k == kind).map(|(_, n)| *n).unwrap()
}

fn row(r: &Row) -> rusqlite::Result<Option<Entry>> {
    let kind: String = r.get("kind")?;
    let Some((kind, _)) = KINDS.iter().find(|(_, n)| *n == kind) else { return Ok(None) };
    Ok(Some(Entry {
        id: r.get("id")?,
        kind: *kind,
        title: r.get("title")?,
        collection: r.get("collection_name")?,
        due_at: r.get("due_at")?,
        zone: r.get("zone")?,
        repeat: r.get("repeat_rule")?,
        pending: r.get::<_, i64>("pending")? != 0,
        created_at: r.get("created_at")?,
        remaining_ms: r.get("remaining_ms")?,
        duration_ms: r.get("duration_ms")?,
    }))
}

impl SqliteStore {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Self::with(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::with(Connection::open_in_memory()?)
    }

    fn with(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self(Mutex::new(conn)))
    }
}

impl Store for SqliteStore {
    fn all(&self, kind: Option<EntryKind>) -> Vec<Entry> {
        let conn = self.0.lock().unwrap();
        let sql = format!("SELECT * FROM entries {} ORDER BY CASE WHEN due_at IS NULL THEN 1 ELSE 0 END, due_at, rowid", if kind.is_some() { "WHERE kind = ?1" } else { "" });
        let mut stmt = conn.prepare(&sql).expect("valid statement");
        let rows = match kind {
            Some(k) => stmt.query_map(params![kind_name(k)], row),
            None => stmt.query_map([], row),
        };
        rows.map(|it| it.filter_map(|r| r.ok().flatten()).collect()).unwrap_or_default()
    }

    fn get(&self, id: &str) -> Option<Entry> {
        let conn = self.0.lock().unwrap();
        conn.query_row("SELECT * FROM entries WHERE id = ?1", params![id], row).optional().ok().flatten().flatten()
    }

    fn put(&self, e: Entry) {
        let conn = self.0.lock().unwrap();
        let anchor = String::new();
        let _ = conn.execute(
            "INSERT INTO entries(id, kind, title, collection_name, due_at, zone, repeat_rule, pending, anchor_time, created_at, remaining_ms, duration_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET kind=?2, title=?3, collection_name=?4, due_at=?5, zone=?6, repeat_rule=?7, pending=?8, created_at=?10, remaining_ms=?11, duration_ms=?12",
            params![e.id, kind_name(e.kind), e.title, e.collection, e.due_at, e.zone, e.repeat, e.pending as i64, anchor, e.created_at, e.remaining_ms, e.duration_ms],
        );
    }

    fn delete(&self, id: &str) -> bool {
        self.0.lock().unwrap().execute("DELETE FROM entries WHERE id = ?1", params![id]).map(|n| n > 0).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, kind: EntryKind, title: &str, due: Option<i64>) -> Entry {
        let mut e = Entry::new(id.into(), kind, title, 1_000, "Europe/Berlin");
        e.due_at = due;
        e
    }

    #[test]
    fn stores_updates_and_deletes() {
        let s = SqliteStore::in_memory().unwrap();
        s.put(entry("a", EntryKind::Reminder, "Oma", Some(5_000)));
        s.put(entry("b", EntryKind::ListItem, "Milch", None));
        assert_eq!(s.all(None).len(), 2);
        assert_eq!(s.all(Some(EntryKind::ListItem))[0].title, "Milch");
        let mut a = s.get("a").unwrap();
        a.title = "Opa".into();
        a.pending = false;
        s.put(a);
        assert_eq!(s.all(None).len(), 2);
        assert_eq!(s.get("a").unwrap().title, "Opa");
        assert!(!s.get("a").unwrap().pending);
        assert!(s.delete("a") && !s.delete("a"));
        assert!(s.get("a").is_none());
    }

    #[test]
    fn orders_by_due_time_and_puts_undated_last() {
        let s = SqliteStore::in_memory().unwrap();
        s.put(entry("late", EntryKind::Reminder, "late", Some(9_000)));
        s.put(entry("none", EntryKind::Reminder, "none", None));
        s.put(entry("soon", EntryKind::Reminder, "soon", Some(2_000)));
        let titles: Vec<String> = s.all(Some(EntryKind::Reminder)).into_iter().map(|e| e.title).collect();
        assert_eq!(titles, ["soon", "late", "none"]);
    }

    #[test]
    fn keeps_everything_a_timer_needs_and_survives_reopening() {
        let path = std::env::temp_dir().join(format!("iris-store-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let s = SqliteStore::open(&path).unwrap();
            let mut t = entry("t", EntryKind::Timer, "Nudeln", Some(8_000));
            t.duration_ms = 600_000;
            t.remaining_ms = 120_000;
            t.repeat = "days:1,3".into();
            s.put(t);
        }
        let s = SqliteStore::open(&path).unwrap();
        let t = s.get("t").unwrap();
        assert_eq!((t.duration_ms, t.remaining_ms, t.repeat.as_str(), t.zone.as_str()), (600_000, 120_000, "days:1,3", "Europe/Berlin"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reads_what_the_android_app_wrote() {
        // a row as the app's own SQLiteOpenHelper (version 3) leaves it
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE entries(id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL, title TEXT NOT NULL, collection_name TEXT NOT NULL, due_at INTEGER, zone TEXT NOT NULL, repeat_rule TEXT NOT NULL, pending INTEGER NOT NULL, anchor_time TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT 0, remaining_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER NOT NULL DEFAULT 0);
            INSERT INTO entries VALUES('x','LIST_ITEM','Brot','Einkauf',NULL,'Europe/Berlin','',1,'',42,0,0);").unwrap();
        let s = SqliteStore::with(conn).unwrap();
        let e = s.get("x").unwrap();
        assert_eq!((e.title.as_str(), e.collection.as_str(), e.created_at, e.pending), ("Brot", "Einkauf", 42, true));
    }
}
