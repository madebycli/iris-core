//! A clock time in a sentence ("um halb acht", "at 6 30", "18:15", "7 pm"), what is left without it,
//! and whether the time was impossible or named twice differently.

use crate::re::{group, nlu, replace_range, Find};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Clock {
    pub hour: u8,
    pub minute: u8,
}

impl std::fmt::Display for Clock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeExtraction {
    pub time: Option<Clock>,
    pub rest: String,
    pub invalid: bool,
    pub ambiguous: bool,
}

impl TimeExtraction {
    fn none(text: &str) -> Self {
        Self { time: None, rest: text.to_string(), invalid: false, ambiguous: false }
    }
    fn invalid(text: &str) -> Self {
        Self { invalid: true, ..Self::none(text) }
    }
    fn ambiguous(text: &str) -> Self {
        Self { ambiguous: true, ..Self::none(text) }
    }
}

pub fn extract(text: &str) -> TimeExtraction {
    let first = single(text);
    let Some(t0) = first.time else { return first };
    let mut rest = first.rest.clone();
    let mut count = 0;
    loop {
        let next = single(&rest);
        if next.invalid {
            return TimeExtraction::invalid(text);
        }
        let Some(t) = next.time else { break };
        count += 1;
        if t != t0 || count >= 16 {
            return TimeExtraction::ambiguous(text);
        }
        rest = next.rest;
    }
    // every bare "or seventeen" alternative, also after repeated equal times
    for marker in nlu(r"\b(?:oder|or)\s+").found_all(&first.rest) {
        let end = marker.get(0).unwrap().end();
        let alternative = single(&format!("um {}", &first.rest[end..]));
        if alternative.invalid {
            return TimeExtraction::invalid(text);
        }
        if alternative.time.is_some() && alternative.time != Some(t0) {
            return TimeExtraction::ambiguous(text);
        }
    }
    first
}

fn result(text: &str, start: usize, end: usize, hour: i64, minute: i64) -> TimeExtraction {
    if (0..=23).contains(&hour) && (0..=59).contains(&minute) {
        TimeExtraction { time: Some(Clock { hour: hour as u8, minute: minute as u8 }), rest: replace_range(text, start, end, " "), invalid: false, ambiguous: false }
    } else {
        TimeExtraction::invalid(text)
    }
}

fn num(s: &str) -> i64 {
    s.parse().unwrap_or(-1)
}

fn single(text: &str) -> TimeExtraction {
    if let Some(m) = nlu(r"\b(?:halb (\d{1,2})|half past (\d{1,2})|(?:viertel nach|quarter past) (\d{1,2})|(?:viertel vor|quarter to) (\d{1,2}))\b").found(text) {
        let whole = m.get(0).unwrap();
        let n = num((1..=4).map(|i| group(&m, i)).find(|v| !v.is_empty()).unwrap());
        if !(1..=12).contains(&n) {
            return TimeExtraction::invalid(text);
        }
        let (h, min) = if !group(&m, 1).is_empty() {
            ((n + 11) % 12, 30)
        } else if !group(&m, 2).is_empty() {
            (n % 12, 30)
        } else if !group(&m, 3).is_empty() {
            (n % 12, 15)
        } else {
            ((n + 11) % 12, 45)
        };
        return result(text, whole.start(), whole.end(), h, min);
    }
    // spoken "at six thirty" arrives as "at 6 30" once number words became digits
    if let Some(m) = nlu(r"\b(?:um|at|for) (\d{1,2}) ([0-5]\d)\b(?!\s*(?:min|sek|stund|hours?|minutes?|seconds?|uhr|am|pm|[:.]))").found(text) {
        let whole = m.get(0).unwrap();
        return result(text, whole.start(), whole.end(), num(group(&m, 1)), num(group(&m, 2)));
    }
    if let Some(m) = nlu(r"\b(\d{1,2})(?::|\.)(\d{2})(?:\s*(am|pm|uhr))?\b|\b(\d{1,2})\s*(uhr|am|pm)(?:\s+(\d{1,2}))?\b|\b(?:um|at|auf|for) (\d{1,2})(?!\s*(?:min|sek|stund|hours?|minutes?|seconds?))(?![\d:.]|\s*(?:uhr|am|pm))\b").found(text) {
        let whole = m.get(0).unwrap();
        let h = [group(&m, 1), group(&m, 4), group(&m, 7)].into_iter().find(|v| !v.is_empty()).unwrap_or("");
        let mut h = num(h);
        let min_s = if group(&m, 2).is_empty() { group(&m, 6) } else { group(&m, 2) };
        let min = min_s.parse::<i64>().unwrap_or(0);
        let suffix = if group(&m, 3).is_empty() { group(&m, 5) } else { group(&m, 3) };
        if suffix == "am" || suffix == "pm" {
            if !(1..=12).contains(&h) {
                return TimeExtraction::invalid(text);
            }
            h = h % 12 + if suffix == "pm" { 12 } else { 0 };
        }
        return result(text, whole.start(), whole.end(), h, min);
    }
    TimeExtraction::none(text)
}
