//! The registry of actions the assistant AI may propose, and the checking of one action's arguments.
//!
//! Language-neutral data (`core/actions/actions.toml`): every action has an id, an executor (a core skill's
//! [`crate::Skill::act`] or the host's `act`), the capabilities it needs and typed arguments. [`Registry::check`]
//! validates one proposed action against that schema and the turn's clock and gives back the checked values; the
//! executor receives those values as they are, never a sentence to read again.
//!
//! Which words describe an action to the assistant, and which request needs which tools, are language data in
//! `core/lang/plan/<code>.toml` (see `plan`).

use crate::host::{Capability, Host};
use jiff::civil::{Date, Time};
use jiff::{ToSpan, Zoned};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

const SOURCE: &str = include_str!("../../../actions/actions.toml");
const MAX_DAYS_AHEAD: i64 = 366;
/// A moment must lie further ahead than this: the plan is checked against the turn's clock and carried out a few
/// seconds later, and a moment that has passed by then would be understood as the next day's.
pub const MIN_LEAD_MS: i64 = 60_000;

/// Why an action was rejected (the `reason` code of a rejected proposal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    Malformed,
    UnknownAction,
    Text,
    Date,
    Time,
    Past,
    Duration,
    Number,
    Choice,
    Unsupported,
}

impl Reject {
    pub fn code(self) -> &'static str {
        match self {
            Reject::Malformed => "malformed",
            Reject::UnknownAction => "unknown_action",
            Reject::Text => "text",
            Reject::Date => "date",
            Reject::Time => "time",
            Reject::Past => "past",
            Reject::Duration => "duration",
            Reject::Number => "number",
            Reject::Choice => "choice",
            Reject::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Seconds,
    Int,
    Date,
    Time,
    Enum,
    Bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    pub kind: Kind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max: Option<i64>,
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Action {
    pub id: String,
    /// `skill:<id>` or `host`.
    pub exec: String,
    #[serde(default)]
    pub need: Vec<Capability>,
    #[serde(default)]
    pub confirm: bool,
    /// `[date argument, time argument, date optional]`.
    #[serde(default)]
    pub moment: Option<(String, String, bool)>,
    #[serde(default)]
    pub args: Vec<ArgSpec>,
}

impl Action {
    /// The family: the part of the id before the dot. Requests select tools by family.
    pub fn family(&self) -> &str {
        self.id.split('.').next().unwrap_or_default()
    }

    /// The skill that carries this action out, `None` for the host.
    pub fn skill(&self) -> Option<&str> {
        self.exec.strip_prefix("skill:")
    }
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    direct: Vec<String>,
    #[serde(rename = "action")]
    actions: Vec<Action>,
}

/// One action whose arguments were checked: the id, the values (only the declared arguments, normalised), and the
/// moment as a point in time when the action has one.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub id: String,
    pub args: Map<String, Value>,
}

/// The values of a checked action, as an executor reads them.
pub struct Args<'a>(pub &'a Map<String, Value>);

impl Args<'_> {
    pub fn text(&self, name: &str) -> Option<&str> {
        self.0.get(name)?.as_str()
    }
    pub fn int(&self, name: &str) -> Option<i64> {
        self.0.get(name)?.as_i64()
    }
    pub fn flag(&self, name: &str) -> Option<bool> {
        self.0.get(name)?.as_bool()
    }
    /// The moment in milliseconds (`at_ms`), set by the check for actions with a date and a time.
    pub fn at_ms(&self) -> Option<i64> {
        self.int("at_ms")
    }
}

pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let file: File = toml::from_str(SOURCE).expect("actions.toml is valid");
        Registry { actions: file.actions, direct: file.direct }
    })
}

pub struct Registry {
    actions: Vec<Action>,
    direct: Vec<String>,
}

impl Registry {
    pub fn all(&self) -> &[Action] {
        &self.actions
    }

    /// Whether the local network's label means one plain command that stays local at every level of AI use.
    pub fn is_direct(&self, label: &str) -> bool {
        self.direct.iter().any(|d| d == label)
    }

    pub fn get(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// The actions the host can carry out now.
    pub fn available(&self, host: &dyn Host) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.need.iter().all(|c| host.has(*c))).collect()
    }

    /// Checks the proposed action `value` (`{"type": id, ...arguments}`) against the schema. Unknown fields, missing
    /// required arguments, a wrong type or a value out of its range reject it. `allowed` are the ids on offer for this
    /// request; another action is `Unsupported`.
    pub fn check(&self, value: &Value, allowed: &[String], now: &Zoned) -> Result<Checked, Reject> {
        let object = value.as_object().ok_or(Reject::Malformed)?;
        let id = object.get("type").and_then(Value::as_str).ok_or(Reject::Malformed)?;
        let action = self.get(id).ok_or(Reject::UnknownAction)?;
        if !allowed.iter().any(|a| a == id) {
            return Err(Reject::Unsupported);
        }
        if object.keys().any(|k| k != "type" && !action.args.iter().any(|a| a.name == *k)) {
            return Err(Reject::Malformed);
        }
        let mut args = Map::new();
        for spec in &action.args {
            match object.get(&spec.name) {
                None | Some(Value::Null) if spec.required => return Err(Reject::Malformed),
                None | Some(Value::Null) => {}
                Some(raw) => {
                    args.insert(spec.name.clone(), check_value(spec, raw, now)?);
                }
            }
        }
        if let Some((date_name, time_name, date_optional)) = &action.moment {
            let time = args.get(time_name).and_then(Value::as_str).and_then(|t| t.parse::<Time>().ok()).ok_or(Reject::Time)?;
            let date = args.get(date_name).and_then(Value::as_str).and_then(|d| d.parse::<Date>().ok());
            match (date, date_optional) {
                (Some(date), _) => {
                    let at = date.at(time.hour(), time.minute(), 0, 0).to_zoned(now.time_zone().clone()).map_err(|_| Reject::Time)?;
                    let ms = at.timestamp().as_millisecond();
                    if ms <= now.timestamp().as_millisecond() + MIN_LEAD_MS {
                        return Err(Reject::Past);
                    }
                    args.insert("at_ms".into(), json!(ms));
                }
                (None, true) => {}
                (None, false) => return Err(Reject::Date),
            }
        }
        Ok(Checked { id: id.to_string(), args })
    }
}

/// Text from the assistant that reaches a skill. It must be a plain phrase: no control characters and no sentence
/// punctuation or brackets, so that a title can never carry a second command.
pub fn clean(text: &str, max: usize) -> Option<String> {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = joined.trim_matches(|c| matches!(c, '"' | '“' | '”' | '„' | '\'' | ' ')).trim_end_matches(['.', '!', '?']).trim();
    if t.is_empty() || t.chars().count() > max {
        return None;
    }
    if t.chars().any(|c| c.is_control() || matches!(c, '.' | ';' | ':' | '!' | '?' | '<' | '>' | '{' | '}' | '[' | ']')) {
        return None;
    }
    Some(t.to_string())
}

fn check_value(spec: &ArgSpec, raw: &Value, now: &Zoned) -> Result<Value, Reject> {
    match spec.kind {
        Kind::Text => {
            let text = raw.as_str().ok_or(Reject::Malformed)?;
            Ok(json!(clean(text, spec.max.unwrap_or(80) as usize).ok_or(Reject::Text)?))
        }
        Kind::Seconds => {
            let n = raw.as_i64().ok_or(Reject::Malformed)?;
            if n < spec.min.unwrap_or(1) || n > spec.max.unwrap_or(86_400) {
                return Err(Reject::Duration);
            }
            Ok(json!(n))
        }
        Kind::Int => {
            let n = raw.as_i64().ok_or(Reject::Malformed)?;
            if n < spec.min.unwrap_or(i64::MIN) || n > spec.max.unwrap_or(i64::MAX) {
                return Err(Reject::Number);
            }
            Ok(json!(n))
        }
        Kind::Bool => Ok(json!(raw.as_bool().ok_or(Reject::Malformed)?)),
        Kind::Enum => {
            let s = raw.as_str().ok_or(Reject::Malformed)?;
            if spec.values.iter().any(|v| v == s) {
                Ok(json!(s))
            } else {
                Err(Reject::Choice)
            }
        }
        Kind::Time => {
            let s = raw.as_str().ok_or(Reject::Malformed)?;
            let shaped = s.len() == 5 && s.as_bytes()[2] == b':' && s.bytes().enumerate().all(|(i, b)| i == 2 || b.is_ascii_digit());
            if !shaped || s.parse::<Time>().is_err() {
                return Err(Reject::Time);
            }
            Ok(json!(s))
        }
        Kind::Date => {
            let s = raw.as_str().ok_or(Reject::Malformed)?;
            let shaped = s.len() == 10 && s.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
            if !shaped {
                return Err(Reject::Date);
            }
            let date = s.parse::<Date>().map_err(|_| Reject::Date)?;
            let today = now.date();
            if date < today {
                return Err(Reject::Past);
            }
            if date > today.checked_add(MAX_DAYS_AHEAD.days()).map_err(|_| Reject::Date)? {
                return Err(Reject::Date);
            }
            Ok(json!(s))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> Zoned {
        "2026-11-18T14:00[Europe/Berlin]".parse().unwrap()
    }

    fn all() -> Vec<String> {
        registry().all().iter().map(|a| a.id.clone()).collect()
    }

    fn check(v: Value) -> Result<Checked, Reject> {
        registry().check(&v, &all(), &now())
    }

    #[test]
    fn the_registry_is_valid_and_every_action_names_a_known_executor() {
        for a in registry().all() {
            assert!(a.exec == "host" || a.skill().is_some(), "{}", a.id);
            if let Some((date, time, _)) = &a.moment {
                assert!(a.args.iter().any(|x| &x.name == time), "{} has no {time}", a.id);
                assert!(a.args.iter().any(|x| &x.name == date) || a.moment.as_ref().is_some_and(|m| m.2), "{} has no {date}", a.id);
            }
        }
        let mut ids: Vec<&str> = registry().all().iter().map(|a| a.id.as_str()).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len(), "duplicate ids");
    }

    #[test]
    fn a_moment_becomes_one_point_in_time_on_the_turns_clock() {
        let c = check(json!({"type":"reminder.create","title":"Oma","date":"2026-11-19","time":"16:00"})).unwrap();
        let at: Zoned = "2026-11-19T16:00[Europe/Berlin]".parse().unwrap();
        assert_eq!(c.args["at_ms"], json!(at.timestamp().as_millisecond()));
        assert_eq!(c.args["title"], json!("Oma"));
    }

    #[test]
    fn what_does_not_fit_the_schema_is_rejected() {
        let past = json!({"type":"reminder.create","title":"x","date":"2026-11-18","time":"14:00"});
        assert_eq!(check(past), Err(Reject::Past));
        assert_eq!(check(json!({"type":"timer.set","seconds":0})), Err(Reject::Duration));
        assert_eq!(check(json!({"type":"timer.set","seconds":600,"extra":1})), Err(Reject::Malformed));
        assert_eq!(check(json!({"type":"timer.set"})), Err(Reject::Malformed));
        assert_eq!(check(json!({"type":"teleport"})), Err(Reject::UnknownAction));
        assert_eq!(check(json!({"type":"device.volume","change":"loud"})), Err(Reject::Choice));
        assert_eq!(check(json!({"type":"device.volume","change":"set","percent":101})), Err(Reject::Number));
        assert_eq!(check(json!({"type":"note.add","text":"a; b"})), Err(Reject::Text));
        assert_eq!(check(json!({"type":"alarm.set","time":"7:00"})), Err(Reject::Time));
        assert_eq!(check(json!({"type":"flashlight.set","on":"yes"})), Err(Reject::Malformed));
    }

    #[test]
    fn an_action_that_is_not_on_offer_is_unsupported() {
        let only_notes = vec!["note.add".to_string()];
        assert_eq!(registry().check(&json!({"type":"timer.set","seconds":60}), &only_notes, &now()), Err(Reject::Unsupported));
        assert!(registry().check(&json!({"type":"note.add","text":"Milch"}), &only_notes, &now()).is_ok());
    }

    #[test]
    fn an_alarm_may_come_without_a_date() {
        let c = check(json!({"type":"alarm.set","time":"07:00"})).unwrap();
        assert!(c.args.get("at_ms").is_none());
    }
}
