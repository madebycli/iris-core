//! The boundary a frontend links against.
//!
//! It is deliberately narrow: text goes in, a [`Reply`](iris_core::Reply) comes out as JSON, and
//! the frontend offers the device through one callback interface. Structured values cross as
//! JSON strings in the shapes `iris-core` serialises, so the generated bindings stay the same
//! when a field is added.

use iris_core::{Capability, Classifier, Engine, Entry, EntryKind, ExternalEvent, ExternalList, ExternalTask, Host, HostError, Prediction, Store};
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

    /// Stored entries as a JSON array, of one kind ("REMINDER", "TIMER", "NOTE", "LIST_ITEM") or all.
    fn entries(&self, kind: Option<String>) -> String;
    fn entry(&self, id: String) -> Option<String>;
    fn put_entry(&self, entry: String);
    fn delete_entry(&self, id: String) -> bool;

    /// The trained network: JSON `[{"label","p"}]`, most likely first; `[]` when it is not loaded.
    fn classify(&self, text: String, lang: String) -> String;
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
        #[derive(serde::Deserialize)]
        struct Row {
            label: String,
            p: f32,
        }
        let rows: Vec<Row> = serde_json::from_str(&self.0.classify(text.to_string(), lang.to_string())).unwrap_or_default();
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

    /// A reminder or timer went off.
    pub fn fired(&self, id: String) {
        self.engine.lock().unwrap().fired(&id);
    }

    pub fn languages(&self) -> Vec<String> {
        self.engine.lock().unwrap().languages()
    }
}
