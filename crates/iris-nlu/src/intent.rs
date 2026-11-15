//! A request with its details read out: what the functions of the app run on directly.

use crate::date::DateSlot;
use crate::time::Clock;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntentKind {
    Weather,
    AlarmSet,
    AlarmNext,
    AlarmList,
    TimerSet,
    TimerQuery,
    TimerCancel,
    CalendarQuery,
    CalendarCreate,
    Time,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeatherAdvice {
    Jacket,
    Umbrella,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Clarification {
    Place,
    PlaceUnclear,
    Time,
    Duration,
    InvalidTime,
    InvalidDate,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoutedIntent {
    pub kind: IntentKind,
    pub date: Option<DateSlot>,
    pub place: Option<String>,
    pub time: Option<Clock>,
    /// Seconds.
    pub duration: Option<i64>,
    pub clarification: Option<Clarification>,
    pub advice: Vec<WeatherAdvice>,
    pub timer_name: Option<String>,
}

impl RoutedIntent {
    pub fn new(kind: IntentKind) -> Self {
        Self { kind, date: None, place: None, time: None, duration: None, clarification: None, advice: Vec::new(), timer_name: None }
    }
    pub fn on(kind: IntentKind, date: Option<DateSlot>) -> Self {
        Self { date, ..Self::new(kind) }
    }
}
