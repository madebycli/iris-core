//! The details of a request once the network has named its function: the day, the time, the place, the
//! person to call, the app to open, the fact to remember. No keyword decides the function here.

use crate::date::{self, DateSlot, Period};
use crate::duration;
use crate::intent::{Clarification, IntentKind, RoutedIntent, WeatherAdvice};
use crate::normalize::{len_utf16, normalize, trim};
use crate::place;
use crate::re::{group, nlu, re, Find};
use crate::time;
use jiff::civil::Date;

/// The functions that run on typed requests.
pub const TYPED_LABELS: [&str; 11] = ["weather", "time", "date", "alarm_set", "alarm_next", "alarm_list", "timer_set", "timer_query", "timer_cancel", "calendar_query", "calendar_create"];

/// Speech-to-text noise off the sentence: fillers ("uh"), polite openers ("can you"), end punctuation.
pub fn clean(text: &str) -> String {
    let t = re(r"(?iu)\b(?:uh|uhm|äh|ähm|hmm)\b,?\s*").sub(trim(text), "");
    let t = re(r"(?iu)^(?:(?:hey|hi|hallo)\s+iris,?\s+)?(?:(?:can|could|would|will) you(?: please)?\s+|please\s+|bitte\s+|kannst du(?: mir)?(?: bitte)?\s+|könntest du(?: mir)?(?: bitte)?\s+)+").sub(trim(&t), "");
    trim(trim(&t).trim_end_matches(['?', '.', '!'])).to_string()
}

const NAME_MARKER: &str = r"(?i)\b(?:mit(?: dem)? (?:namen|titel)|namens|called|named|titled|with(?: the)? title)\s+(.+)$";

fn name_of(marker: &Option<crate::re::Caps>) -> Option<String> {
    marker.as_ref().map(|m| trim(group(m, 1)).to_string()).filter(|n| (1..=80).contains(&len_utf16(n)))
}

pub fn routed(label: &str, input: &str, today: Date) -> Option<RoutedIntent> {
    if !TYPED_LABELS.contains(&label) {
        return None;
    }
    let cleaned = clean(input);
    let cleaned = if trim(&cleaned).is_empty() { input.to_string() } else { cleaned };
    let marker = re(NAME_MARKER).found(&cleaned);
    let source = match &marker {
        Some(m) if label.starts_with("timer") || label.starts_with("calendar") => &cleaned[..m.get(0).unwrap().start()],
        _ => cleaned.as_str(),
    };
    let t = normalize(source);
    let date = date::extract(&t, today);
    let time = time::extract(&date.rest);
    let duration = duration::extract(&date.rest);
    let has = |p: &str| nlu(p).contains(&t);
    // the past only makes sense for reading; an alarm, a forecast or a new appointment cannot lie in it
    let past_not_allowed = ["weather", "alarm_set", "timer_set", "calendar_create"].contains(&label) && date.slot.as_ref().is_some_and(|s| s.date < today);
    let time_problem = if date.invalid || past_not_allowed {
        Some(Clarification::InvalidDate)
    } else if time.ambiguous {
        Some(Clarification::Time)
    } else if time.invalid {
        Some(Clarification::InvalidTime)
    } else {
        None
    };
    Some(match label {
        "weather" => {
            let location = place::resolve(&time.rest);
            let mut advice = Vec::new();
            if has(r"\b(jacke|jacket|mantel|coat)\b") {
                advice.push(WeatherAdvice::Jacket);
            }
            if has(r"\b(schirm|umbrella)\b") {
                advice.push(WeatherAdvice::Umbrella);
            }
            let clarification = time_problem.or(if location.needs_clarification {
                Some(Clarification::PlaceUnclear)
            } else if location.place.is_none() {
                Some(Clarification::Place)
            } else {
                None
            });
            RoutedIntent {
                date: Some(date.slot.clone().unwrap_or_else(|| DateSlot::with(today, if time.time.is_some() { Period::Day } else { Period::Now }))),
                place: location.place,
                time: time.time,
                clarification,
                advice,
                ..RoutedIntent::new(IntentKind::Weather)
            }
        }
        "time" => RoutedIntent::new(IntentKind::Time),
        "date" => RoutedIntent::on(IntentKind::Date, Some(DateSlot::day(today))),
        "alarm_set" => RoutedIntent {
            time: time.time,
            clarification: time_problem.or(if time.time.is_none() { Some(Clarification::Time) } else { None }),
            ..RoutedIntent::on(IntentKind::AlarmSet, date.slot.clone())
        },
        "alarm_next" => RoutedIntent::on(IntentKind::AlarmNext, date.slot.clone()),
        "alarm_list" => RoutedIntent::on(IntentKind::AlarmList, date.slot.clone()),
        "timer_set" => RoutedIntent {
            duration,
            clarification: if duration.is_none() { Some(Clarification::Duration) } else { None },
            timer_name: name_of(&marker),
            ..RoutedIntent::new(IntentKind::TimerSet)
        },
        "timer_query" => RoutedIntent::new(IntentKind::TimerQuery),
        "timer_cancel" => RoutedIntent::new(IntentKind::TimerCancel),
        "calendar_query" | "calendar_create" => RoutedIntent {
            time: time.time,
            clarification: if date.invalid { Some(Clarification::InvalidDate) } else { None },
            ..RoutedIntent::on(if label == "calendar_query" { IntentKind::CalendarQuery } else { IntentKind::CalendarCreate }, Some(date.slot.clone().unwrap_or_else(|| DateSlot::day(today))))
        },
        _ => return None,
    })
}

fn at_least_two(s: &str) -> Option<String> {
    let s = trim(s);
    (len_utf16(s) >= 2).then(|| s.to_string())
}

/// The person or number to call, or none when only a verb was said.
pub fn call_target(input: &str) -> Option<String> {
    let clean = clean(input);
    if re(r"(?iu)^(?:ruf|rufe|anruf|call|phone|ring|dial|telefon)(?:\s+(?:an|mal|bitte|please|now|jetzt))*$").whole(&clean) {
        return None;
    }
    let mut t = re(r"(?iu)^(?:ruf|rufe|wähl|wähle|telefoniere mit|stell mich zu|verbinde mich mit|hol|call|phone|ring up|ring|dial|get|connect me (?:with|to)|patch me through to|put a call through to|give)\s+(?:(?:mir|me|bitte|please)\s+)*(?:(?:the|my|meine[nrm]?|meinen|mein)\s+)?").sub(&clean, "");
    for _ in 0..3 {
        t = re(r"(?iu)\s+(?:an|ans telefon|durch|on the phone|a ring|a call|now|jetzt|bitte|please)$").sub(&t, "");
    }
    at_least_two(&t)
}

pub fn destination(input: &str) -> Option<String> {
    let t = clean(input);
    if let Some(m) = re(r"(?iu)\b(?:to|zu|zum|zur|nach|towards)\s+(.{2,100})$").found(&t) {
        return Some(trim(group(&m, 1)).to_string());
    }
    at_least_two(&re(r"(?iu)^(?:navigier(?:e)?(?: mich)?|navigate(?: me)?|take me|bring mich|führe mich|fahr mich|route|directions?|how do i get|wie komme ich|show me the way)\s*").sub(&t, ""))
}

pub fn app_name(input: &str) -> String {
    let t = clean(input);
    re(r"(?iu)(?:open|launch|start|search up|look up|go to|bring up|bring me to|get me to|öffne|oeffne|starte|mach|geh zu|ich (?:will|möchte)|i want(?: to (?:use|watch|open))?|i'd like|give me)\s+(?:the\s+|die\s+|den\s+|das\s+|my\s+|meine\s+)?(?:app\s+)?(.+)$")
        .found(&t)
        .map(|m| trim(group(&m, 1)).to_string())
        .unwrap_or(t)
}

pub fn search_query(input: &str) -> String {
    let t = clean(input);
    re(r"(?iu)(?:search(?: up| for)?|google|look up|find|such(?:e)?(?: nach)?|schau(?: nach)?)\s+(?:for\s+|nach\s+)?(.+)$").found(&t).map(|m| trim(group(&m, 1)).to_string()).unwrap_or(t)
}

pub fn fact(input: &str) -> Option<String> {
    at_least_two(&re(r"(?iu)^(?:merk dir|merke dir|merk|remember|notier(?:e)?|schreib auf|keep in mind|denk daran|denk dran|note)\s*(?:dass|that|bitte|please)?\s*").sub(&clean(input), ""))
}

pub fn task_title(input: &str) -> Option<String> {
    let t = re(r"(?iu)^(?:bitte\s+|please\s+)?(?:erstelle|erstell|füge|lege|leg|trag|schreib|notiere|speichere?|add|create|new|neue|make|put)\s+(?:mir\s+|me\s+)?(?:(?:eine|1|a|an)\s+)?(?:neue\s+|new\s+)?(?:aufgabe|task|to[ -]?do)?\s*(?:(?:für mich|for me|for myself)\s+)?(?:namens\s+|called\s+|named\s+|mit dem titel\s+|für\s+|for\s+|about\s+|saying\s+|that says\s+|to\s+|:)?\s*(?:(?:für mich|for me)\s+)?").sub(&clean(input), "");
    let t = trim(&t).trim_start_matches(['.', ':', ',', ' ']);
    let lower = t.to_lowercase();
    (len_utf16(t) >= 2 && lower != "task" && lower != "aufgabe" && !re(r"(?i)to[ -]?do").whole(t)).then(|| t.to_string())
}

/// "play some jazz" gives "some jazz"; none for "continue the music".
pub fn music_query(input: &str) -> Option<String> {
    let m = re(r"(?iu)^(?:play|spiel(?:e)?|put on|start|mach|setz)\s+(.+)$").found_owned(&clean(input))?;
    let q = trim(&m).to_string();
    let generic = re(r"(?iu)^(?:music|musik|something|etwas|was|again|weiter|on|it|das|the music|die musik|the song|das lied|where i stopped|where it stopped|from before)\b.*$|.*\b(?:where (?:i|it) (?:stopped|paused|left off)|wo ich aufgehört|aufgehört habe|von vorhin)\b.*");
    (len_utf16(&q) >= 2 && !generic.whole(&q)).then_some(q)
}

pub fn wants_tomorrow(input: &str) -> bool {
    nlu(r"\b(?:morgen|tomorrow|tomorrow'?s|morgige|morgigen)\b").contains(&normalize(input))
}

/// "flashlight_on" / "flashlight_off" need no details.
pub fn flashlight(label: &str) -> Option<bool> {
    match label {
        "flashlight_on" => Some(true),
        "flashlight_off" => Some(false),
        _ => None,
    }
}

trait FoundOwned {
    fn found_owned(&self, text: &str) -> Option<String>;
}

impl FoundOwned for fancy_regex::Regex {
    /// The first group of the first match.
    fn found_owned(&self, text: &str) -> Option<String> {
        self.found(text).map(|m| group(&m, 1).to_string())
    }
}
