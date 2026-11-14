//! What the engine asks of the device it runs on. Android implements this; tests use a fake.
//!
//! A skill may only use what its manifest declares, and the engine checks that against
//! [`Host::has`] when the skill is registered.

use crate::entry::Entry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Wake the app at an entry's due time (alarms, notifications).
    Schedule,
    /// The user's task app (Tasks.org, jtx Board, OpenTasks, CalDAV).
    Tasks,
    /// The device calendar.
    Calendar,
    Http,
    Location,
    /// Setting and asking for alarms through the device's clock app.
    Alarm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// The device has no such thing, or the user has not connected it.
    Unavailable,
    /// The user has not granted the permission.
    Denied,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalList {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalTask {
    pub title: String,
    pub due_at: Option<i64>,
    /// No time of day: the task is due on that day.
    pub date_only: bool,
    pub list_id: Option<String>,
    pub repeat: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEvent {
    pub title: String,
    pub start_at: i64,
    pub minutes: u32,
    pub repeat: String,
}

/// An alarm to set: the next time the clock shows `hour:minute`, at `at_ms`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmRequest {
    pub at_ms: i64,
    pub hour: u8,
    pub minute: u8,
}

pub trait Host: Send + Sync {
    fn now_ms(&self) -> i64;
    /// IANA name of the device's time zone.
    fn zone(&self) -> String;
    fn has(&self, _capability: Capability) -> bool {
        false
    }
    /// A user setting, by key ("reminder.destination").
    fn setting(&self, _key: &str) -> Option<String> {
        None
    }
    /// Arrange for the user to be alerted when `entry` is due. Replaces an earlier plan for the same id.
    fn schedule(&self, _entry: &Entry) -> Result<(), HostError> {
        Err(HostError::Unavailable)
    }
    fn unschedule(&self, _id: &str) {}
    fn task_lists(&self) -> Result<Vec<ExternalList>, HostError> {
        Err(HostError::Unavailable)
    }
    /// Returns the name of the list the task went to.
    fn create_task(&self, _task: &ExternalTask) -> Result<String, HostError> {
        Err(HostError::Unavailable)
    }
    /// Returns the name of the calendar the event went to.
    fn create_event(&self, _event: &ExternalEvent) -> Result<String, HostError> {
        Err(HostError::Unavailable)
    }
    /// Asks the clock app to set an alarm. `Ok(true)`: the device confirmed it; `Ok(false)`: it was handed over and nothing more is known.
    fn set_alarm(&self, _alarm: &AlarmRequest) -> Result<bool, HostError> {
        Err(HostError::Unavailable)
    }
    /// The time of the next alarm that will ring, in milliseconds.
    fn next_alarm(&self) -> Option<i64> {
        None
    }
    /// Opens the clock app's list of alarms.
    fn show_alarms(&self) -> Result<(), HostError> {
        Err(HostError::Unavailable)
    }
}
