//! A rule reading of a sentence, kept only for the guard of the online AI (does a question need a phone
//! function?). The assistant itself is decided by the network.

use crate::date::{self, DateSlot, Period};
use crate::duration;
use crate::intent::{Clarification, IntentKind, RoutedIntent, WeatherAdvice};
use crate::normalize::{len_utf16, normalize, trim};
use crate::place;
use crate::re::{group, nlu, re, Find};
use crate::time;
use jiff::civil::Date;

pub fn route(input: &str, language: &str, today: Date) -> Option<RoutedIntent> {
    if language != "de" && language != "en" {
        return None;
    }
    let marker = re(r"(?i)\b(?:mit(?: dem)? (?:namen|titel)|namens|called|named|titled|with(?: the)? title)\s+(.+)$").found(input);
    let prefix = marker.as_ref().map(|m| &input[..m.get(0).unwrap().start()]);
    let named_payload = prefix.filter(|p| re(r"(?i)\b(?:timer|timers|termin|termine|kalender|calendar|appointment|appointments)\b").contains(p));
    let t = normalize(named_payload.unwrap_or(input));
    if nlu(r"^(?:öffne|starte|open|launch|suche|search)\b").contains(&t) {
        return None;
    }
    let date = date::extract(&t, today);
    let time = time::extract(&date.rest);
    let duration = duration::extract(&date.rest);
    let has = |p: &str| nlu(p).contains(&t);
    let weather_feel = nlu(r"^(?:wird es|ist es|wird|will it|is it going to|is it|will it be)\b.*\b(?:kalt|warm|heiß|heiss|windig|sonnig|neblig|schnee|schneit|gewitter|stürmisch|cold|warm|hot|windy|sunny|snow|snowing|foggy|storm|stormy)\b").contains(&t);
    if weather_feel || has(r"\b(wetter|weather|regnet|regnen|rain|raining|schirm|umbrella|jacke|jacket)\b") {
        let location = place::resolve(&time.rest);
        let clarification = if date.invalid {
            Some(Clarification::InvalidDate)
        } else if time.ambiguous {
            Some(Clarification::Time)
        } else if time.invalid {
            Some(Clarification::InvalidTime)
        } else if location.needs_clarification {
            Some(Clarification::PlaceUnclear)
        } else if location.place.is_none() {
            Some(Clarification::Place)
        } else {
            None
        };
        let mut advice = Vec::new();
        if has(r"\b(jacke|jacket)\b") {
            advice.push(WeatherAdvice::Jacket);
        }
        if has(r"\b(schirm|umbrella)\b") {
            advice.push(WeatherAdvice::Umbrella);
        }
        return Some(RoutedIntent {
            date: Some(date.slot.clone().unwrap_or_else(|| DateSlot::with(today, if time.time.is_some() { Period::Day } else { Period::Now }))),
            place: location.place,
            time: time.time,
            clarification,
            advice,
            ..RoutedIntent::new(IntentKind::Weather)
        });
    }
    if has(r"\b(wecker|weck mich|alarm|alarms|wake me|aufstehen|get up|geweckt|woken up)\b") {
        if has(r"\b(welche|welchen|alle|welcher|which|all|show|what alarms)\b") && !has(r"\b(wann|when|next|nächste|nächster)\b") {
            return Some(RoutedIntent::on(IntentKind::AlarmList, date.slot));
        }
        if has(r"\b(wann|when|nächster|nächsten|next|what time|wie spät)\b") {
            return Some(RoutedIntent::on(IntentKind::AlarmNext, date.slot));
        }
        // "Was sind meine aktiven Wecker?" is a question, not a request to set one; Android only exposes the next alarm
        if time.time.is_none() && has(r"\b(aktiv|aktive|aktiven|aktuell|aktuelle|gestellt|gestellten|active|set)\b") && !has(r"\b(stell|stelle|setze|set an|erstell|erstelle|create)\b") {
            return Some(RoutedIntent::on(IntentKind::AlarmNext, date.slot));
        }
        if time.time.is_none() && has(r"^(?:was|what)\b.*\b(?:meine|my)\b") {
            return Some(RoutedIntent::on(IntentKind::AlarmNext, date.slot));
        }
        let clarification = if date.invalid {
            Some(Clarification::InvalidDate)
        } else if time.ambiguous {
            Some(Clarification::Time)
        } else if time.invalid {
            Some(Clarification::InvalidTime)
        } else if time.time.is_none() {
            Some(Clarification::Time)
        } else {
            None
        };
        return Some(RoutedIntent { time: time.time, clarification, ..RoutedIntent::on(IntentKind::AlarmSet, date.slot) });
    }
    if has(r"\b(?:timer|timers)\b") {
        let timer_name = marker.as_ref().map(|m| trim(group(m, 1)).to_string()).filter(|n| (1..=80).contains(&len_utf16(n)));
        if has(r"\b(stoppe|stop|cancel|lösche|beende)\b") {
            return Some(RoutedIntent { timer_name, ..RoutedIntent::new(IntentKind::TimerCancel) });
        }
        if has(r"\b(wie lange|how long|remaining|noch|welche|welcher|welchen|which|what|all|laufen|active|running)\b") {
            return Some(RoutedIntent { timer_name, ..RoutedIntent::new(IntentKind::TimerQuery) });
        }
        if time.time.is_some() {
            return Some(RoutedIntent { time: time.time, ..RoutedIntent::on(IntentKind::AlarmSet, date.slot) });
        }
        return Some(RoutedIntent { duration, clarification: if duration.is_none() { Some(Clarification::Duration) } else { None }, timer_name, ..RoutedIntent::new(IntentKind::TimerSet) });
    }
    if has(r"\b(termin|termine|kalender|calendar|appointment|appointments|schedule|habe ich.*zeit|do i have.*(?:anything|time))\b")
        || has(r"\b(stundenplan|unterricht|schultag|schulstunde|schulstunden|timetable|school schedule|lessons)\b")
        || has(r"\b(schule|school)\b") && has(r"^(?:was|welche|wann|wie|habe ich|what|when|which|how|do i)\b")
        || date.slot.is_some() && has(r"^(?:wie sieht es|wie schaut es|wie sieht.*aus|what does.*look like|how does.*look)\b")
        || date.slot.is_some() && has(r"\b(was steht.*an|steht.*an|etwas vor|was habe ich.*vor|habe ich.*vor|what is on|what s on|anything planned|any plans)\b")
    {
        let create = has(r"\b(trag|trage|erstell|erstelle|add|create|schedule an|book)\b");
        return Some(RoutedIntent {
            time: time.time,
            clarification: if date.invalid { Some(Clarification::InvalidDate) } else { None },
            ..RoutedIntent::on(if create { IntentKind::CalendarCreate } else { IntentKind::CalendarQuery }, Some(date.slot.unwrap_or_else(|| DateSlot::day(today))))
        });
    }
    if has(r"\b(wie spät|wie viel uhr|what time|the time|current time|uhrzeit|tell me the time|time is it|time now)\b") {
        return Some(RoutedIntent::new(IntentKind::Time));
    }
    if has(r"\b(welcher tag|welches datum|what day|what's the date|what is the date)\b") {
        return Some(RoutedIntent::on(IntentKind::Date, Some(DateSlot::day(today))));
    }
    None
}
