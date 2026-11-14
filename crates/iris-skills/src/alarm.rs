//! Alarms, through the device's clock app: set one, ask for the next, open the list.
//!
//! A bare hour is taken as said ("um fünf" is 5:00): the user who asks a clock for an alarm at
//! five means the next five on a 24-hour dial, and the reply names the time in full.

use iris_core::time::{describe, resolve, scan, HalfDay, TimeError, TimeExpr};
use iris_core::{AlarmRequest, Cx, HostError, Reply, Skill};
use serde_json::json;

pub struct Alarm;

impl Skill for Alarm {
    fn manifest(&self) -> &'static str {
        include_str!("../data/alarm/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/alarm/de.toml")),
            "en" => Some(include_str!("../data/alarm/en.toml")),
            _ => None,
        }
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        match intent {
            "set" | "set.time" => Some(set(cx)),
            "next" => Some(match cx.host.next_alarm().and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok()) {
                Some(at) => Reply::done(cx.msg("next", &[("when", &describe(at, &cx.now, cx.lang, false))])),
                None => Reply::done(cx.msg("none", &[])),
            }),
            "list" => Some(match cx.host.show_alarms() {
                Ok(()) => Reply::done(cx.msg("list_opened", &[])),
                Err(_) => Reply::failed(cx.msg("no_clock", &[])),
            }),
            _ => None,
        }
    }
}

fn set(cx: &mut Cx) -> Reply {
    let norms = cx.utt.norms();
    let expr: TimeExpr = scan(&norms, cx.lang).expr;
    let resolved = match resolve(&expr, &cx.now, HalfDay::AsSaid) {
        Ok(r) => r,
        Err(error) => {
            let key = if matches!(error, TimeError::Missing) { "ask_time" } else { "time_unclear" };
            cx.ask("set.time", json!({}));
            return Reply::question(cx.msg(key, &[]));
        }
    };
    let z = resolved.at.to_zoned(cx.now.time_zone().clone());
    let request = AlarmRequest { at_ms: resolved.at.as_millisecond(), hour: z.hour() as u8, minute: z.minute() as u8 };
    let when = describe(resolved.at, &cx.now, cx.lang, false);
    match cx.host.set_alarm(&request) {
        Ok(true) => Reply::done(cx.msg("set", &[("when", &when)])),
        Ok(false) => Reply::done(cx.msg("handed_over", &[("when", &when)])),
        Err(HostError::Unavailable) | Err(HostError::Denied) => Reply::failed(cx.msg("no_clock", &[])),
        Err(HostError::Failed(_)) => Reply::failed(cx.msg("no_clock", &[])),
    }
}
