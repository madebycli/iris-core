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
    /// The phone's own settings (volume, brightness, sound mode), what is playing, and the web browser.
    Device,
    /// Reading notifications of other apps and answering them.
    Notifications,
    /// The camera's torch.
    Flashlight,
    /// The contacts, to call someone.
    Contacts,
    /// A maps app that takes a destination.
    Maps,
    /// Controlling the media player and asking what plays.
    Media,
    /// Launching other apps.
    Apps,
    /// Weather data the user has set up.
    Weather,
    /// Reading the device calendar.
    CalendarRead,
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
    /// ISO weekdays (Monday = 1) the alarm repeats on; empty for a single alarm.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
}

/// How a level (volume, brightness) should change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelChange {
    Up,
    Down,
    /// To `percent`.
    Set,
    /// Brightness only: let the phone decide.
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundMode {
    Ring,
    Vibrate,
    Silent,
}

/// A change to the phone itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum DeviceAction {
    /// `ring`: the ringer, otherwise media.
    Volume { ring: bool, change: LevelChange, percent: Option<u8> },
    Brightness { change: LevelChange, percent: Option<u8> },
    SoundMode { mode: SoundMode },
    DoNotDisturb { on: bool },
    /// Open a web search for `query` in the browser.
    WebSearch { query: String },
    /// Ask the clock app to dismiss an alarm: the one at `hour:minute`, or the next one when no time is given.
    DismissAlarm {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hour: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        minute: Option<u8>,
    },
}

/// What a [`DeviceAction`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceResult {
    /// The level the phone reports afterwards, for volume and brightness.
    #[serde(default)]
    pub percent: Option<u8>,
    /// Hand this back to [`Host::restore`] to undo the change. `None`: it cannot be undone.
    #[serde(default)]
    pub restore: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NowPlaying {
    pub title: String,
    #[serde(default)]
    pub artist: String,
}

/// A notification of another app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// Names this notification for [`Host::reply_to`].
    pub key: String,
    /// The app's name.
    pub app: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: String,
    /// It offers a reply field.
    #[serde(default)]
    pub can_reply: bool,
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
    /// Keeps a small value for later, also across restarts (read back with [`Host::setting`]). `None` removes it.
    fn keep(&self, _key: &str, _value: Option<&str>) {}
    /// Changes the phone. `Denied`: the permission is missing and the host has opened the screen to grant it;
    /// `Failed`: Android did not apply it.
    fn device(&self, _action: &DeviceAction) -> Result<DeviceResult, HostError> {
        Err(HostError::Unavailable)
    }
    /// Undoes a [`DeviceAction`] with the token its result carried.
    fn restore(&self, _token: &str) -> Result<(), HostError> {
        Err(HostError::Unavailable)
    }
    /// What the active media player plays. `Ok(None)`: a player is there but names no title.
    fn now_playing(&self) -> Result<Option<NowPlaying>, HostError> {
        Err(HostError::Unavailable)
    }
    /// Notifications of other apps, newest first; `from` (may be empty) matches the app's name or the title.
    fn notifications(&self, _from: &str) -> Result<Vec<Notice>, HostError> {
        Err(HostError::Unavailable)
    }
    /// Answers the notification `key` with `message` through its reply field.
    fn reply_to(&self, _key: &str, _message: &str) -> Result<(), HostError> {
        Err(HostError::Unavailable)
    }
}
