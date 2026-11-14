//! The boundary a frontend links against.
//!
//! It is deliberately narrow: text goes in, a [`Reply`](iris_core::Reply) comes out as JSON, and
//! the frontend offers the device through one callback interface. Structured values cross as
//! JSON strings in the shapes `iris-core` serialises, so the generated bindings stay the same
//! when a field is added.

use iris_core::{Capability, Classifier, DeviceAction, DeviceResult, Engine, Entry, EntryKind, ExternalEvent, ExternalList, ExternalTask, Host, HostError, Notice, NowPlaying, Prediction, Store};
use std::sync::{Arc, Mutex};

uniffi::setup_scaffolding!();

/// What the device offers the core. Implemented by the app.
#[uniffi::export(callback_interface)]
pub trait Phone: Send + Sync {
    fn now_ms(&self) -> i64;
    /// IANA name of the time zone.
    fn zone(&self) -> String;
    fn setting(&self, key: String) -> Option<String>;

    /// Plan an alert for the entry (JSON of an entry). False when it could not be planned.
    fn schedule(&self, entry: String) -> bool;
    fn unschedule(&self, id: String);

    /// The lists of the user's task app as JSON `[{"id","name"}]`; `None` when there is no such app.
    fn task_lists(&self) -> Option<String>;
    /// Creates a task (JSON of an external task). Returns the name of the list it went to.
    fn create_task(&self, task: String) -> Option<String>;
    /// Creates a calendar event (JSON of an external event). Returns the calendar's name.
    fn create_event(&self, event: String) -> Option<String>;

    /// Asks the clock app for an alarm (JSON `{"at_ms","hour","minute"}`). `None`: no clock app took it;
    /// `Some(true)`: the device confirmed it; `Some(false)`: handed over, no receipt.
    fn set_alarm(&self, alarm: String) -> Option<bool>;
    /// Time of the next alarm that will ring, in milliseconds.
    fn next_alarm(&self) -> Option<i64>;
    /// Opens the clock app's alarm list. False when there is none.
    fn show_alarms(&self) -> bool;

    /// Keeps a small value across restarts, read back through `setting`. `None` removes it.
    fn keep(&self, key: String, value: Option<String>);
    /// Changes the phone (JSON of a device action, `{"action": "volume", ...}`). Returns JSON
    /// `{"ok": true, "percent": 40, "restore": "token"}` or `{"ok": false, "error": "denied" | "missing" | "failed"}`.
    fn device(&self, action: String) -> String;
    /// Undoes a change with the token its result carried. False when that is no longer possible.
    fn restore(&self, token: String) -> bool;
    /// What the media player plays, as JSON `{"title","artist"}`; `"{}"` when it names no title; `None` when no player is reachable.
    fn now_playing(&self) -> Option<String>;
    /// Notifications of other apps as JSON `[{"key","app","title","text","can_reply"}]`, newest first; `None` without access.
    fn notifications(&self, from: String) -> Option<String>;
    /// Answers a notification. False when it cannot be answered any more.
    fn reply_to(&self, key: String, message: String) -> bool;

    /// Stored entries as a JSON array, of one kind ("REMINDER", "TIMER", "NOTE", "LIST_ITEM") or all.
    fn entries(&self, kind: Option<String>) -> String;
    fn entry(&self, id: String) -> Option<String>;
    fn put_entry(&self, entry: String);
    fn delete_entry(&self, id: String) -> bool;

    /// The trained network: JSON `[{"label","p"}]`, most likely first; `[]` when it is not loaded.
    fn classify(&self, text: String, lang: String, context: String) -> String;
}

struct Bridge(Arc<dyn Phone>);

impl Host for Bridge {
    fn now_ms(&self) -> i64 {
        self.0.now_ms()
    }
    fn zone(&self) -> String {
        self.0.zone()
    }
    fn has(&self, _capability: Capability) -> bool {
        true
    }
    fn setting(&self, key: &str) -> Option<String> {
        self.0.setting(key.to_string())
    }
    fn schedule(&self, entry: &Entry) -> Result<(), HostError> {
        let json = serde_json::to_string(entry).map_err(|e| HostError::Failed(e.to_string()))?;
        self.0.schedule(json).then_some(()).ok_or(HostError::Denied)
    }
    fn unschedule(&self, id: &str) {
        self.0.unschedule(id.to_string());
    }
    fn set_alarm(&self, alarm: &iris_core::AlarmRequest) -> Result<bool, HostError> {
        let json = serde_json::to_string(alarm).map_err(|e| HostError::Failed(e.to_string()))?;
        self.0.set_alarm(json).ok_or(HostError::Unavailable)
    }
    fn next_alarm(&self) -> Option<i64> {
        self.0.next_alarm()
    }
    fn show_alarms(&self) -> Result<(), HostError> {
        self.0.show_alarms().then_some(()).ok_or(HostError::Unavailable)
    }
    fn keep(&self, key: &str, value: Option<&str>) {
        self.0.keep(key.to_string(), value.map(str::to_string));
    }
    fn device(&self, action: &DeviceAction) -> Result<DeviceResult, HostError> {
        #[derive(serde::Deserialize)]
        struct Answer {
            ok: bool,
            #[serde(default)]
            error: String,
            #[serde(default)]
            percent: Option<u8>,
            #[serde(default)]
            restore: Option<String>,
        }
        let json = serde_json::to_string(action).map_err(|e| HostError::Failed(e.to_string()))?;
        let answer: Answer = serde_json::from_str(&self.0.device(json)).map_err(|e| HostError::Failed(e.to_string()))?;
        if answer.ok {
            return Ok(DeviceResult { percent: answer.percent, restore: answer.restore });
        }
        Err(match answer.error.as_str() {
            "denied" => HostError::Denied,
            "missing" => HostError::Unavailable,
            other => HostError::Failed(other.to_string()),
        })
    }
    fn restore(&self, token: &str) -> Result<(), HostError> {
        self.0.restore(token.to_string()).then_some(()).ok_or(HostError::Unavailable)
    }
    fn now_playing(&self) -> Result<Option<NowPlaying>, HostError> {
        let json = self.0.now_playing().ok_or(HostError::Unavailable)?;
        Ok(serde_json::from_str::<NowPlaying>(&json).ok().filter(|p| !p.title.is_empty()))
    }
    fn notifications(&self, from: &str) -> Result<Vec<Notice>, HostError> {
        let json = self.0.notifications(from.to_string()).ok_or(HostError::Denied)?;
        serde_json::from_str(&json).map_err(|e| HostError::Failed(e.to_string()))
    }
    fn reply_to(&self, key: &str, message: &str) -> Result<(), HostError> {
        self.0.reply_to(key.to_string(), message.to_string()).then_some(()).ok_or(HostError::Unavailable)
    }
    fn task_lists(&self) -> Result<Vec<ExternalList>, HostError> {
        let json = self.0.task_lists().ok_or(HostError::Unavailable)?;
        serde_json::from_str(&json).map_err(|e| HostError::Failed(e.to_string()))
    }
    fn create_task(&self, task: &ExternalTask) -> Result<String, HostError> {
        let json = serde_json::to_string(task).map_err(|e| HostError::Failed(e.to_string()))?;
        self.0.create_task(json).ok_or(HostError::Unavailable)
    }
    fn create_event(&self, event: &ExternalEvent) -> Result<String, HostError> {
        let json = serde_json::to_string(event).map_err(|e| HostError::Failed(e.to_string()))?;
        self.0.create_event(json).ok_or(HostError::Unavailable)
    }
}

impl Store for Bridge {
    fn all(&self, kind: Option<EntryKind>) -> Vec<Entry> {
        let kind = kind.and_then(|k| serde_json::to_value(k).ok()).and_then(|v| v.as_str().map(str::to_string));
        serde_json::from_str(&self.0.entries(kind)).unwrap_or_default()
    }
    fn get(&self, id: &str) -> Option<Entry> {
        serde_json::from_str(&self.0.entry(id.to_string())?).ok()
    }
    fn put(&self, entry: Entry) {
        if let Ok(json) = serde_json::to_string(&entry) {
            self.0.put_entry(json);
        }
    }
    fn delete(&self, id: &str) -> bool {
        self.0.delete_entry(id.to_string())
    }
}

impl Classifier for Bridge {
    fn classify(&self, text: &str, lang: &str) -> Vec<Prediction> {
        self.classify_with(text, lang, &iris_core::ConversationContext::default())
    }

    fn classify_with(&self, text: &str, lang: &str, context: &iris_core::ConversationContext) -> Vec<Prediction> {
        #[derive(serde::Deserialize)]
        struct Row {
            label: String,
            p: f32,
        }
        let rows: Vec<Row> = serde_json::from_str(&self.0.classify(text.to_string(), lang.to_string(), serde_json::to_string(context).unwrap_or_default())).unwrap_or_default();
        rows.into_iter().map(|r| Prediction { label: r.label, p: r.p }).collect()
    }
}

/// The assistant. One per app.
#[derive(uniffi::Object)]
pub struct IrisCore {
    engine: Mutex<Engine>,
}

#[uniffi::export]
impl IrisCore {
    #[uniffi::constructor]
    pub fn new(phone: Box<dyn Phone>) -> Arc<Self> {
        let phone: Arc<dyn Phone> = Arc::from(phone);
        let mut engine = iris_skills::engine(Arc::new(Bridge(phone.clone())), Arc::new(Bridge(phone.clone()))).expect("built-in skills are valid");
        engine.set_classifier(Box::new(Bridge(phone)));
        Arc::new(Self { engine: Mutex::new(engine) })
    }

    /// Answers one utterance. Returns a reply as JSON: `kind` ("done", "question", "failed",
    /// "unhandled"), `text`, `speech`, `card`, `skill`, `intent`.
    pub fn process(&self, text: String, lang: String) -> String {
        let reply = self.engine.lock().unwrap().process(&text, &lang);
        serde_json::to_string(&reply).unwrap_or_default()
    }

    /// Whether a skill is waiting for an answer: the microphone should open again.
    pub fn awaits_answer(&self) -> bool {
        self.engine.lock().unwrap().has_question()
    }

    /// The app rang a reminder with its own scheduler (repeats and "done" are the app's business):
    /// the core remembers it, so "erinnere mich in 10 Minuten nochmal" means this one.
    pub fn rang(&self, id: String) {
        self.engine.lock().unwrap().rang(&id);
    }

    /// A reminder or timer went off and the core should plan what comes next.
    pub fn fired(&self, id: String) {
        self.engine.lock().unwrap().fired(&id);
    }

    /// The app answered the last sentence itself: "say that again" and "do that again" are no longer the core's.
    pub fn answered_elsewhere(&self) {
        self.engine.lock().unwrap().answered_elsewhere();
    }

    pub fn languages(&self) -> Vec<String> {
        self.engine.lock().unwrap().languages()
    }
}
