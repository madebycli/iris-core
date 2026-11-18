//! The boundary a frontend links against.
//!
//! It is deliberately narrow: text goes in, a [`Reply`](iris_core::Reply) comes out as JSON, and
//! the frontend offers the device through one callback interface. Structured values cross as
//! JSON strings in the shapes `iris-core` serialises, so the generated bindings stay the same
//! when a field is added.

use iris_core::{Capability, Classifier, DeviceAction, DeviceResult, Engine, Entry, EntryKind, ExternalEvent, ExternalList, ExternalTask, Host, HostError, Notice, NowPlaying, Prediction, Store};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

uniffi::setup_scaffolding!();

mod nlu;
mod cloud;

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

/// The trained network, owned by the core. Loaded on the first sentence that needs it and released
/// when the app says the assistant has been idle, so it costs no memory while Iris is not used.
#[derive(Default)]
struct BrainSlot {
    place: Mutex<Option<(PathBuf, PathBuf)>>,
    brain: Mutex<Option<Arc<iris_brain::Brain>>>,
    error: Mutex<Option<String>>,
}

impl BrainSlot {
    fn get(&self) -> Option<Arc<iris_brain::Brain>> {
        let mut brain = self.brain.lock().unwrap();
        if let Some(b) = brain.as_ref() {
            return Some(b.clone());
        }
        let (dir, library) = self.place.lock().unwrap().clone()?;
        match iris_brain::Brain::load_with(&dir, &library) {
            Ok(b) => {
                let b = Arc::new(b);
                *brain = Some(b.clone());
                *self.error.lock().unwrap() = None;
                Some(b)
            }
            Err(e) => {
                *self.error.lock().unwrap() = Some(e.0);
                None
            }
        }
    }
}

struct BrainClassifier(Arc<BrainSlot>);

impl Classifier for BrainClassifier {
    fn classify(&self, text: &str, lang: &str) -> Vec<Prediction> {
        self.classify_with(text, lang, &iris_core::ConversationContext::default())
    }

    fn classify_with(&self, text: &str, lang: &str, context: &iris_core::ConversationContext) -> Vec<Prediction> {
        self.0.get().map(|b| b.classify_with(text, lang, context)).unwrap_or_default()
    }
}

/// The conversation as the app tells the network about it (the fields of `iris_brain::Context`).
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct ContextJson {
    last: String,
    last2: String,
    shown: String,
    titles: Vec<String>,
    focus: Option<i32>,
    has: Vec<String>,
    age: i32,
    expect: String,
    slot: String,
}

#[derive(serde::Serialize)]
struct ScoredJson<'a> {
    label: &'a str,
    p: f32,
}

#[derive(serde::Serialize)]
struct UnderstandingJson<'a> {
    acts: Vec<ScoredJson<'a>>,
    heads: std::collections::BTreeMap<&'a str, ScoredJson<'a>>,
}

/// The assistant. One per app.
#[derive(uniffi::Object)]
pub struct IrisCore {
    engine: Mutex<Engine>,
    brain: Arc<BrainSlot>,
    /// The clock of the planning turn in progress: taken when the planner prompt is made, used to check its answer.
    turn: Mutex<Option<(i64, String)>>,
}

#[uniffi::export]
impl IrisCore {
    #[uniffi::constructor]
    pub fn new(phone: Box<dyn Phone>) -> Arc<Self> {
        let phone: Arc<dyn Phone> = Arc::from(phone);
        let mut engine = iris_skills::engine(Arc::new(Bridge(phone.clone())), Arc::new(Bridge(phone))).expect("built-in skills are valid");
        let brain = Arc::new(BrainSlot::default());
        engine.set_classifier(Box::new(BrainClassifier(brain.clone())));
        Arc::new(Self { engine: Mutex::new(engine), brain, turn: Mutex::new(None) })
    }

    /// Where the network is: a directory with `brain.onnx`, `meta.txt` and `pieces.tsv`, and the ONNX
    /// Runtime library (on Android its file name, `libonnxruntime.so`, is enough). Nothing is loaded yet.
    pub fn set_brain(&self, dir: String, library: String) {
        *self.brain.place.lock().unwrap() = Some((PathBuf::from(dir), PathBuf::from(library)));
        *self.brain.brain.lock().unwrap() = None;
    }

    /// Loads the network now (when the assistant opens), so the first sentence does not wait. False when it cannot be loaded.
    pub fn warm_up(&self) -> bool {
        self.brain.get().is_some()
    }

    /// Frees the network's memory; the next sentence loads it again.
    pub fn release_brain(&self) {
        *self.brain.brain.lock().unwrap() = None;
    }

    pub fn brain_loaded(&self) -> bool {
        self.brain.brain.lock().unwrap().is_some()
    }

    /// Why the network could not be loaded the last time, if it could not.
    pub fn brain_error(&self) -> Option<String> {
        self.brain.error.lock().unwrap().clone()
    }

    /// What the network makes of a sentence in its conversation (JSON of `iris_brain::Context`):
    /// JSON `{"acts": [{"label","p"}] (three, likeliest first), "heads": {"name": {"label","p"}}}`.
    /// `None` when the network cannot be loaded or run.
    pub fn understand(&self, text: String, context: String) -> Option<String> {
        let c: ContextJson = serde_json::from_str(&context).unwrap_or_default();
        let ctx = iris_brain::Context { last: c.last, last2: c.last2, shown: c.shown, titles: c.titles, focus: c.focus.unwrap_or(-1), has: c.has, age: c.age, expect: c.expect, slot: c.slot };
        let brain = self.brain.get()?;
        match brain.understand(&ctx, &text) {
            Ok(u) => {
                let json = UnderstandingJson {
                    acts: u.acts.iter().map(|s| ScoredJson { label: &s.label, p: s.p }).collect(),
                    heads: u.heads.iter().map(|(n, s)| (n.as_str(), ScoredJson { label: &s.label, p: s.p })).collect(),
                };
                serde_json::to_string(&json).ok()
            }
            Err(e) => {
                *self.brain.error.lock().unwrap() = Some(e.0);
                None
            }
        }
    }

    /// Answers one utterance. Returns a reply as JSON: `kind` ("done", "question", "failed",
    /// "unhandled"), `text`, `speech`, `card`, `skill`, `intent`.
    pub fn process(&self, text: String, lang: String) -> String {
        let reply = self.engine.lock().unwrap().process(&text, &lang);
        serde_json::to_string(&reply).unwrap_or_default()
    }

    /// Where a sentence goes at AI-use `level` (1 sparing .. 4 AI first): "local", "talk" (the assistant answers in
    /// text) or "plan" (the assistant proposes actions, see `plan_prompt` and `plan_validate`). An answer to an
    /// open question of the core is always "local".
    pub fn route(&self, level: u8, text: String, lang: String) -> String {
        let open = self.engine.lock().unwrap().has_question();
        match iris_core::plan::route(level, &text, &lang, open) {
            iris_core::plan::Route::Local => "local",
            iris_core::plan::Route::Talk => "talk",
            iris_core::plan::Route::Plan => "plan",
        }
        .to_string()
    }

    /// What the assistant is told to plan actions: the contract for the action families this phone can carry out, and
    /// this turn's clock. The clock is kept, so `plan_validate` checks the answer against the same instant. `None`
    /// for a language the planner has no words for, or when the phone can carry out none of the actions.
    pub fn plan_prompt(&self, lang: String) -> Option<String> {
        let (clock, families) = {
            let engine = self.engine.lock().unwrap();
            (engine.turn_clock(), engine.plan_families())
        };
        let prompt = iris_core::plan::prompt(&lang, clock.0, &clock.1, &families);
        *self.turn.lock().unwrap() = prompt.is_some().then_some(clock);
        prompt
    }

    /// Checks the assistant's whole reply against the contract, the turn's clock and what this phone can carry out.
    /// JSON: `{"kind":"actions","actions":[{"kind","sentence"}]}`, `{"kind":"ask","slot","question"}`,
    /// `{"kind":"not_action"}` or `{"kind":"rejected","reason"}`. Nothing is carried out here: the host says each
    /// sentence to `process`.
    pub fn plan_validate(&self, raw: String, lang: String) -> String {
        let (host_clock, families) = {
            let engine = self.engine.lock().unwrap();
            (engine.turn_clock(), engine.plan_families())
        };
        let (now_ms, zone) = turn_for_validation(self.turn.lock().unwrap().take(), host_clock);
        serde_json::to_string(&iris_core::plan::validate(&raw, &lang, now_ms, &zone, &families)).unwrap_or_default()
    }

    /// The user dismissed the question the core waits on: it is dropped, and only it. Entries that exist stay.
    /// Returns whether a question was open.
    pub fn cancel_question(&self) -> bool {
        self.engine.lock().unwrap().cancel_question()
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

/// How long the clock of a planning turn is kept: an answer that takes longer than this is checked against the
/// current time instead.
const TURN_CLOCK_TTL_MS: i64 = 10 * 60_000;

/// The clock to check an answer against: the one the prompt was made with (one clock per turn), unless there was
/// none, it is older than [`TURN_CLOCK_TTL_MS`], or the phone's clock went backwards.
fn turn_for_validation(kept: Option<(i64, String)>, host: (i64, String)) -> (i64, String) {
    match kept {
        Some((at, zone)) if host.0 >= at && host.0 - at <= TURN_CLOCK_TTL_MS => (at, zone),
        _ => host,
    }
}

#[cfg(test)]
mod turn_clock_tests {
    use super::turn_for_validation;

    #[test]
    fn the_prompts_clock_is_the_one_the_answer_is_checked_with() {
        let kept = Some((1_000_000, "Europe/Berlin".to_string()));
        assert_eq!(turn_for_validation(kept, (1_020_000, "Europe/Berlin".into())), (1_000_000, "Europe/Berlin".into()));
    }

    #[test]
    fn without_a_kept_clock_or_when_it_is_stale_the_current_time_is_used() {
        let host = (50_000_000, "Europe/Berlin".to_string());
        assert_eq!(turn_for_validation(None, host.clone()), host);
        assert_eq!(turn_for_validation(Some((1_000, "UTC".into())), host.clone()), host);
        // a clock that went backwards
        assert_eq!(turn_for_validation(Some((60_000_000, "UTC".into())), host.clone()), host);
    }
}
