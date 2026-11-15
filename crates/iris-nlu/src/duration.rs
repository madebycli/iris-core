//! "zehn Minuten", "1 Stunde 30 Minuten": seconds, at most a week.

use crate::re::{group, nlu, Find};

pub fn extract(text: &str) -> Option<i64> {
    let matches = nlu(r"\b(\d{1,4})\s*(stunden?|hours?|minuten?|minutes?|sekunden?|seconds?)\b").found_all(text);
    if matches.is_empty() {
        return None;
    }
    let mut seconds: i64 = 0;
    for m in &matches {
        let n: i64 = group(m, 1).parse().unwrap_or(0);
        let unit = group(m, 2);
        let f = if unit.starts_with("stund") || unit.starts_with("hour") { 3600 } else if unit.starts_with("min") { 60 } else { 1 };
        seconds += n * f;
    }
    (1..=604800).contains(&seconds).then_some(seconds)
}
