//! Alarms, through the device's clock app: set one, ask for the next, open the list.
//!
//! A bare hour is taken as said ("um fünf" is 5:00): the user who asks a clock for an alarm at
//! five means the next five on a 24-hour dial, and the reply names the time in full.

use iris_core::time::{describe, resolve, scan, HalfDay, Repeat, TimeError, TimeExpr};
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
            "dismiss" | "delete" => {
                // a time names the alarm ("Lösche den Wecker für 15 Uhr"); without one it is the next alarm
                let expr = scan(&cx.utt.norms(), cx.lang).expr;
                let at = expr.clock.map(|c| (c.hour, c.minute));
                cx.ask("dismiss.confirm", json!({ "hour": at.map(|a| a.0), "minute": at.map(|a| a.1) }));
                Some(Reply::question(match at {
                    Some((h, m)) => {
                        let time = cx.lang.format.clock.replace("{h}", &h.to_string()).replace("{mm}", &format!("{m:02}"));
                        cx.msg("dismiss_ask_at", &[("time", &time)])
                    }
                    None => cx.msg("dismiss_ask", &[]),
                }))
            }
            // neither yes nor no: a new request, the question is dropped
            "dismiss.confirm" if cx.lang.dialog.is_no(&cx.utt.norm) => Some(Reply::done(cx.lang.messages.get("cancelled", &[]))),
            "dismiss.confirm" if !cx.lang.dialog.is_yes(&cx.utt.norm) => None,
            "dismiss.confirm" => {
                let hour = cx.state["hour"].as_u64().map(|h| h as u8);
                let minute = cx.state["minute"].as_u64().map(|m| m as u8).or(hour.map(|_| 0));
                Some(match cx.host.device(&iris_core::DeviceAction::DismissAlarm { hour, minute }) {
                    Ok(_) if hour.is_some() => Reply::done(cx.msg("dismissed_at", &[])),
                    Ok(_) => Reply::done(cx.msg("dismissed", &[])),
                    Err(_) => Reply::failed(cx.msg("no_clock", &[])),
                })
            }
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
    // a repeating alarm is one the clock app repeats on these weekdays (Monday = 1)
    let days: Vec<u8> = match &resolved.repeat {
        Some(Repeat::Daily) => (1..=7).collect(),
        Some(Repeat::Weekdays) => (1..=5).collect(),
        Some(Repeat::Weekly) => vec![z.weekday().to_monday_one_offset() as u8],
        _ => Vec::new(),
    };
    let request = AlarmRequest { at_ms: resolved.at.as_millisecond(), hour: z.hour() as u8, minute: z.minute() as u8, days: days.clone() };
    let when = if days.is_empty() {
        describe(resolved.at, &cx.now, cx.lang, false)
    } else {
        let time = cx.lang.format.clock.replace("{h}", &z.hour().to_string()).replace("{mm}", &format!("{:02}", z.minute()));
        let repeat = match &resolved.repeat {
            Some(Repeat::Daily) => cx.msg("repeat.daily", &[]),
            Some(Repeat::Weekdays) => cx.msg("repeat.weekdays", &[]),
            _ => cx.msg("repeat.weekly", &[]),
        };
        cx.msg("repeating_when", &[("repeat", &repeat), ("time", &time)])
    };
    match cx.host.set_alarm(&request) {
        Ok(true) => Reply::done(cx.msg("set", &[("when", &when)])),
        Ok(false) => Reply::done(cx.msg("handed_over", &[("when", &when)])),
        Err(HostError::Unavailable) | Err(HostError::Denied) => Reply::failed(cx.msg("no_clock", &[])),
        Err(HostError::Failed(_)) => Reply::failed(cx.msg("no_clock", &[])),
    }
}
