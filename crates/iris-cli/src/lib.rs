//! A desktop stand-in for the phone, and the runner for conversation specs.
//!
//! A spec is a text file of conversations. Each line is one of:
//!
//! ```text
//! ## name of the case            starts a fresh engine
//! @now 2026-11-10T16:00 Europe/Berlin
//! @lang de
//! @setting reminder.destination tasks
//! @tasklist Arbeit               the task app has this list
//! @tasks down                    the task app cannot be reached
//! @advance 90s                   time passes (s, m, h)
//! > what the user says
//! < text the reply must contain  (case does not matter)
//! <! text the reply must not contain
//! = reminder "Title" 2026-11-11T08:00 repeat=daily
//! = timer "Title" 600s
//! = task "Title" 2026-11-11T08:00 list=Arbeit
//! = event "Title" 2026-11-11T08:00
//! = count reminder 1
//! = kind question                done, question, failed or unhandled
//! ```

use iris_core::{Capability, Engine, Entry, EntryKind, ExternalEvent, ExternalList, ExternalTask, Host, HostError, MemoryStore, Reply, ReplyKind};
use jiff::civil::DateTime;
use jiff::tz::TimeZone;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct FakeState {
    pub now_ms: i64,
    pub zone: String,
    pub settings: HashMap<String, String>,
    pub lists: Vec<ExternalList>,
    pub tasks_down: bool,
    pub tasks: Vec<(ExternalTask, String)>,
    pub events: Vec<ExternalEvent>,
    pub scheduled: Vec<String>,
}

/// A phone that exists only in memory.
pub struct FakeHost(pub Mutex<FakeState>);

impl FakeHost {
    pub fn new(now_ms: i64, zone: &str) -> Self {
        Self(Mutex::new(FakeState { now_ms, zone: zone.to_string(), ..Default::default() }))
    }
}

impl Host for FakeHost {
    fn now_ms(&self) -> i64 {
        self.0.lock().unwrap().now_ms
    }
    fn zone(&self) -> String {
        self.0.lock().unwrap().zone.clone()
    }
    fn has(&self, _capability: Capability) -> bool {
        true
    }
    fn setting(&self, key: &str) -> Option<String> {
        self.0.lock().unwrap().settings.get(key).cloned()
    }
    fn schedule(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.lock().unwrap().scheduled.push(entry.id.clone());
        Ok(())
    }
    fn unschedule(&self, id: &str) {
        self.0.lock().unwrap().scheduled.retain(|x| x != id);
    }
    fn task_lists(&self) -> Result<Vec<ExternalList>, HostError> {
        let s = self.0.lock().unwrap();
        if s.tasks_down {
            Err(HostError::Unavailable)
        } else {
            Ok(s.lists.clone())
        }
    }
    fn create_task(&self, task: &ExternalTask) -> Result<String, HostError> {
        let mut s = self.0.lock().unwrap();
        if s.tasks_down {
            return Err(HostError::Unavailable);
        }
        let list = task.list_id.as_ref().and_then(|id| s.lists.iter().find(|l| l.id == *id)).or(s.lists.first()).map(|l| l.name.clone()).unwrap_or_else(|| "Tasks".into());
        s.tasks.push((task.clone(), list.clone()));
        Ok(list)
    }
    fn create_event(&self, event: &ExternalEvent) -> Result<String, HostError> {
        self.0.lock().unwrap().events.push(event.clone());
        Ok("Calendar".into())
    }
}

pub struct Session {
    pub host: Arc<FakeHost>,
    pub store: Arc<MemoryStore>,
    pub engine: Engine,
    pub lang: String,
}

impl Session {
    pub fn new(now_ms: i64, zone: &str, lang: &str) -> Self {
        let host = Arc::new(FakeHost::new(now_ms, zone));
        let store = Arc::new(MemoryStore::new());
        let engine = iris_skills::engine(host.clone(), store.clone()).expect("built-in skills are valid");
        Self { host, store, engine, lang: lang.to_string() }
    }

    pub fn say(&mut self, text: &str) -> Reply {
        self.engine.process(text, &self.lang)
    }
}

#[derive(Debug)]
pub struct Failure {
    pub line: usize,
    pub case: String,
    pub message: String,
}

fn millis(local: &str, zone: &str) -> Result<i64, String> {
    let dt: DateTime = local.parse().map_err(|e| format!("bad date-time {local:?}: {e}"))?;
    let tz = TimeZone::get(zone).map_err(|e| format!("bad zone {zone:?}: {e}"))?;
    Ok(tz.to_ambiguous_zoned(dt).compatible().map_err(|e| e.to_string())?.timestamp().as_millisecond())
}

fn quoted(rest: &str) -> Option<(String, &str)> {
    let rest = rest.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    Some((rest[..end].to_string(), rest[end + 1..].trim()))
}

/// Runs every case of one spec. Returns the checks made and what failed.
pub fn run_spec(source: &str) -> (usize, Vec<Failure>) {
    let default_now = "2026-11-10T16:00";
    let mut session = Session::new(millis(default_now, "Europe/Berlin").unwrap(), "Europe/Berlin", "de");
    let (mut case, mut last, mut checks, mut failures) = (String::new(), None::<Reply>, 0, Vec::new());
    for (index, raw) in source.lines().enumerate() {
        let line = raw.trim();
        let mut fail = |message: String| failures.push(Failure { line: index + 1, case: case.clone(), message });
        if let Some(name) = line.strip_prefix("##") {
            case = name.trim().to_string();
            session = Session::new(millis(default_now, "Europe/Berlin").unwrap(), "Europe/Berlin", "de");
            last = None;
        } else if let Some(rest) = line.strip_prefix('@') {
            let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
            let mut host = session.host.0.lock().unwrap();
            match key {
                "now" => {
                    let (local, zone) = value.split_once(' ').unwrap_or((value, "Europe/Berlin"));
                    match millis(local, zone) {
                        Ok(ms) => {
                            host.now_ms = ms;
                            host.zone = zone.to_string();
                        }
                        Err(e) => fail(e),
                    }
                }
                "lang" => session.lang = value.to_string(),
                "setting" => {
                    let (k, v) = value.split_once(' ').unwrap_or((value, ""));
                    host.settings.insert(k.to_string(), v.to_string());
                }
                "tasklist" => {
                    let id = format!("list-{}", host.lists.len() + 1);
                    host.lists.push(ExternalList { id, name: value.to_string() });
                }
                "tasks" => host.tasks_down = value == "down",
                "advance" => {
                    let (number, unit) = value.split_at(value.len().saturating_sub(1));
                    let scale = match unit {
                        "s" => 1000,
                        "m" => 60_000,
                        "h" => 3_600_000,
                        _ => 0,
                    };
                    match number.parse::<i64>() {
                        Ok(n) if scale > 0 => host.now_ms += n * scale,
                        _ => fail(format!("bad @advance {value:?}")),
                    }
                }
                other => fail(format!("unknown directive @{other}")),
            }
        } else if let Some(said) = line.strip_prefix('>') {
            last = Some(session.say(said.trim()));
        } else if let Some(want) = line.strip_prefix("<!") {
            checks += 1;
            let text = last.as_ref().map(|r| r.text.clone()).unwrap_or_default();
            if text.to_lowercase().contains(&want.trim().to_lowercase()) {
                fail(format!("reply must not contain {:?}: {text:?}", want.trim()));
            }
        } else if let Some(want) = line.strip_prefix('<') {
            checks += 1;
            let text = last.as_ref().map(|r| r.text.clone()).unwrap_or_default();
            if !text.to_lowercase().contains(&want.trim().to_lowercase()) {
                fail(format!("reply should contain {:?}: {text:?}", want.trim()));
            }
        } else if let Some(check) = line.strip_prefix('=') {
            checks += 1;
            if let Err(message) = verify(check.trim(), &session, last.as_ref()) {
                fail(message);
            }
        }
    }
    (checks, failures)
}

fn kind_of(word: &str) -> Option<EntryKind> {
    match word {
        "reminder" => Some(EntryKind::Reminder),
        "timer" => Some(EntryKind::Timer),
        "note" => Some(EntryKind::Note),
        "item" => Some(EntryKind::ListItem),
        _ => None,
    }
}

fn verify(check: &str, session: &Session, last: Option<&Reply>) -> Result<(), String> {
    use iris_core::Store;
    let (word, rest) = check.split_once(' ').unwrap_or((check, ""));
    let host = session.host.0.lock().unwrap();
    let zone = host.zone.clone();
    let everything = || session.store.all(None).iter().map(|e| format!("{:?} {:?} due={:?} repeat={:?}", e.kind, e.title, e.due_at, e.repeat)).collect::<Vec<_>>().join("; ");
    match word {
        "kind" => {
            let want = match rest {
                "done" => ReplyKind::Done,
                "question" => ReplyKind::Question,
                "failed" => ReplyKind::Failed,
                "unhandled" => ReplyKind::Unhandled,
                other => return Err(format!("unknown kind {other:?}")),
            };
            match last {
                Some(r) if r.kind == want => Ok(()),
                other => Err(format!("expected a {rest} reply, got {:?}: {:?}", other.map(|r| r.kind), other.map(|r| r.text.clone()))),
            }
        }
        "count" => {
            let (kind, n) = rest.split_once(' ').ok_or("count needs a kind and a number")?;
            let kind = kind_of(kind).ok_or(format!("unknown kind {kind:?}"))?;
            let have = session.store.all(Some(kind)).iter().filter(|e| e.pending).count();
            (have.to_string() == n).then_some(()).ok_or(format!("expected {n} {kind:?}, found {have}: {}", everything()))
        }
        "reminder" | "timer" | "note" | "item" => {
            let kind = kind_of(word).unwrap();
            let (title, rest) = quoted(rest).ok_or("a title in quotes is needed")?;
            let found: Vec<Entry> = session.store.all(Some(kind)).into_iter().filter(|e| e.title == title).collect();
            if found.is_empty() {
                return Err(format!("no {word} titled {title:?}; stored: {}", everything()));
            }
            for part in rest.split_whitespace() {
                let ok = if let Some(code) = part.strip_prefix("repeat=") {
                    found.iter().any(|e| e.repeat == code)
                } else if let Some(secs) = part.strip_suffix('s').and_then(|s| s.parse::<i64>().ok()) {
                    found.iter().any(|e| e.duration_ms == secs * 1000)
                } else {
                    let want = millis(part, &zone)?;
                    found.iter().any(|e| e.due_at == Some(want))
                };
                if !ok {
                    return Err(format!("{word} {title:?} does not match {part}; stored: {}", everything()));
                }
            }
            Ok(())
        }
        "task" => {
            let (title, rest) = quoted(rest).ok_or("a title in quotes is needed")?;
            let found: Vec<&(ExternalTask, String)> = host.tasks.iter().filter(|(t, _)| t.title == title).collect();
            if found.is_empty() {
                return Err(format!("no task {title:?} in the task app; it has {:?}", host.tasks));
            }
            for part in rest.split_whitespace() {
                let ok = match part.strip_prefix("list=") {
                    Some(list) => found.iter().any(|(_, l)| l == list),
                    None => {
                        let want = millis(part, &zone)?;
                        found.iter().any(|(t, _)| t.due_at == Some(want))
                    }
                };
                if !ok {
                    return Err(format!("task {title:?} does not match {part}: {:?}", host.tasks));
                }
            }
            Ok(())
        }
        "event" => {
            let (title, rest) = quoted(rest).ok_or("a title in quotes is needed")?;
            let found: Vec<&ExternalEvent> = host.events.iter().filter(|e| e.title == title).collect();
            if found.is_empty() {
                return Err(format!("no event {title:?} in the calendar; it has {:?}", host.events));
            }
            for part in rest.split_whitespace() {
                let want = millis(part, &zone)?;
                if !found.iter().any(|e| e.start_at == want) {
                    return Err(format!("event {title:?} is not at {part}: {:?}", host.events));
                }
            }
            Ok(())
        }
        other => Err(format!("unknown check {other:?}")),
    }
}
