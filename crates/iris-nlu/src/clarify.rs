//! Short readings of the network's ranking to tap when it is unsure: never generic presets, only functions
//! it actually considered, with the values the user really said.

use crate::fallback::Prediction;
use jiff::civil::Date;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub display: String,
    pub label: String,
    /// Only values the user actually supplied.
    pub text: String,
    pub heads: BTreeMap<String, String>,
    pub day: Date,
}

pub const MAX: usize = 3;
pub const MIN_SHARE: f32 = 0.12;

fn ranked(top: &[Prediction]) -> Vec<&Prediction> {
    let mut v: Vec<&Prediction> = top.iter().filter(|p| p.p.is_finite() && (0.0..=1.0).contains(&p.p)).collect();
    v.sort_by(|a, b| b.p.partial_cmp(&a.p).unwrap());
    let mut seen: Vec<&str> = Vec::new();
    v.retain(|p| {
        if seen.contains(&p.label.as_str()) {
            false
        } else {
            seen.push(&p.label);
            true
        }
    });
    v
}

/// A tail of unrelated low scores must not become a random recommendation.
pub fn offers(top: &[Prediction], text: &str, de: bool, heads: &BTreeMap<String, String>, today: Date) -> Vec<Offer> {
    let ranked = ranked(top);
    let eligible: Vec<&Prediction> = ranked.iter().copied().filter(|p| title(&p.label, de).is_some()).collect();
    let Some(best) = eligible.first().map(|p| p.p) else { return Vec::new() };
    if best < MIN_SHARE {
        return Vec::new();
    }
    if ranked.first().is_some_and(|p| p.label == "none" && p.p >= 0.75) {
        return Vec::new();
    }
    let strongest = family(&eligible[0].label);
    eligible
        .iter()
        .filter(|p| p.p >= MIN_SHARE.max(best * 0.45) && best - p.p <= 0.25 && (family(&p.label) == strongest || (p.p >= best * 0.7 && best - p.p <= 0.14)))
        .take(MAX)
        .map(|p| Offer { display: title(&p.label, de).unwrap().to_string(), label: p.label.clone(), text: text.to_string(), heads: heads.clone(), day: today })
        .collect()
}

/// A choice before running, when the two strongest functions are genuinely close.
pub fn ambiguous(top: &[Prediction]) -> bool {
    let mut p: Vec<&Prediction> = top.iter().filter(|p| title(&p.label, true).is_some() && p.p.is_finite()).collect();
    p.sort_by(|a, b| b.p.partial_cmp(&a.p).unwrap());
    p.len() > 1 && p[0].p < 0.85 && p[1].p >= 0.22 && p[0].p - p[1].p <= 0.18
}

fn family(label: &str) -> &str {
    if label.starts_with("school_") {
        "school"
    } else if label.starts_with("alarm_") || label.starts_with("timer_") || label == "time" || label == "date" {
        "clock"
    } else if label.starts_with("music_") || label == "lyrics" || label == "song_recognize" {
        "music"
    } else if label.starts_with("anime_") {
        "anime"
    } else if label.starts_with("calendar_") {
        "calendar"
    } else {
        label
    }
}

/// A short name of a function as a heading: "Raum", "Timer stellen". None: nothing to offer for it.
pub fn title(label: &str, de: bool) -> Option<&'static str> {
    let table: &[(&str, &str)] = if de { DE } else { EN };
    table.iter().find(|(l, _)| *l == label).map(|(_, t)| *t)
}

const DE: &[(&str, &str)] = &[
    ("weather", "Wetter"), ("time", "Uhrzeit"), ("date", "Datum"), ("alarm_set", "Wecker stellen …"), ("alarm_next", "Nächster Wecker"), ("alarm_list", "Meine Wecker"),
    ("timer_set", "Timer stellen …"), ("timer_query", "Restzeit des Timers"), ("timer_cancel", "Timer beenden"),
    ("calendar_query", "Termine"), ("calendar_create", "Termin anlegen …"), ("day_overview", "Tagesübersicht"), ("tasks_create", "Aufgabe anlegen"), ("tasks_list", "Aufgaben"),
    ("school_timetable", "Stundenplan"), ("school_next_lesson", "Nächste Stunde"), ("school_room", "Raum"), ("school_exams", "Klausuren"), ("school_homework", "Hausaufgaben"),
    ("school_cancelled", "Ausfälle"), ("school_messages", "Schulnachrichten"),
    ("anime_airing", "Anime-Folgen"), ("anime_next", "Nächste Anime-Folge"), ("anime_behind", "Anime im Rückstand"), ("anime_title", "Anime suchen"), ("anime_progress", "Anime-Fortschritt eintragen"),
    ("anime_status", "Anime-Status ändern"), ("anime_refresh", "Anime-Tracker aktualisieren"), ("anime_changes", "Verschobene Anime-Folgen"),
    ("music_play", "Musik abspielen"), ("music_pause", "Musik pausieren"), ("music_next", "Nächstes Lied"), ("music_previous", "Vorheriges Lied"), ("song_recognize", "Lied erkennen"),
    ("lyrics", "Liedtext"), ("call", "Anrufen …"), ("navigation", "Navigation starten"), ("open_app", "App öffnen …"), ("search_web", "Im Web suchen"), ("translate", "Übersetzen"),
    ("calculator", "Ausrechnen"), ("battery", "Akkustand"), ("flashlight_on", "Taschenlampe an"), ("flashlight_off", "Taschenlampe aus"),
    ("system_settings", "Einstellung ändern"), ("memory_remember", "Merken"), ("memory_recall", "Gemerktes abrufen"), ("joke", "Witz erzählen"),
];

const EN: &[(&str, &str)] = &[
    ("weather", "Weather"), ("time", "Time"), ("date", "Date"), ("alarm_set", "Set an alarm …"), ("alarm_next", "Next alarm"), ("alarm_list", "My alarms"),
    ("timer_set", "Set a timer …"), ("timer_query", "Time left on the timer"), ("timer_cancel", "Stop the timer"),
    ("calendar_query", "Appointments"), ("calendar_create", "Create an appointment …"), ("day_overview", "Day overview"), ("tasks_create", "Create a task"), ("tasks_list", "Tasks"),
    ("school_timetable", "Timetable"), ("school_next_lesson", "Next lesson"), ("school_room", "Room"), ("school_exams", "Exams"), ("school_homework", "Homework"),
    ("school_cancelled", "Cancelled lessons"), ("school_messages", "School messages"),
    ("anime_airing", "Anime episodes"), ("anime_next", "Next anime episode"), ("anime_behind", "Anime I am behind on"), ("anime_title", "Look up an anime"), ("anime_progress", "Record anime progress"),
    ("anime_status", "Change an anime status"), ("anime_refresh", "Refresh the anime tracker"), ("anime_changes", "Moved anime episodes"),
    ("music_play", "Play music"), ("music_pause", "Pause the music"), ("music_next", "Next song"), ("music_previous", "Previous song"), ("song_recognize", "Recognise a song"),
    ("lyrics", "Song lyrics"), ("call", "Call"), ("navigation", "Start navigation"), ("open_app", "Open an app"), ("search_web", "Search the web"), ("translate", "Translate"),
    ("calculator", "Calculate"), ("battery", "Battery level"), ("flashlight_on", "Flashlight on"), ("flashlight_off", "Flashlight off"),
    ("system_settings", "Change a setting"), ("memory_remember", "Remember this"), ("memory_recall", "Recall a memory"), ("joke", "Tell a joke"),
];
