//! What Iris does when the network is not sure enough to run a function: it says why it cannot and what
//! would help. A near miss becomes a question ("Meinst du Wecker oder Timer?"), a knowledge question says it
//! needs the online AI, a fragment asks what it refers to, noise gets an example. Pure and deterministic.

use crate::re::{re, Find};

/// One scored guess of the network.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction {
    pub label: String,
    pub p: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    Run(String),
    Clarify(String, String),
    Suggest(Vec<String>),
    Knowledge,
    Fragment,
    Unknown,
}

pub const CUTOFF: f32 = 0.60;

pub fn is_question(text: &str) -> bool {
    let t = text.trim();
    t.ends_with('?') || re(r"(?iu)^(?:wer|was|wann|wo|wie|warum|wieso|weshalb|welche[rsnm]?|wieviel\w*|who|what|when|where|why|how|which|whose|is|are|does|do|did|can|could)\b").contains(t)
}

/// Kotlin's `trim().split(Regex("\\s+")).count { it.isNotEmpty() }`.
pub fn word_count(text: &str) -> usize {
    re(r"\s+").split_all(text.trim()).into_iter().filter(|w| !w.is_empty()).count()
}

/// `last` is the function of the previous turn. When the network is unsure and its runner-up is the previous
/// function, the sentence most likely continues that conversation, so the two swap places (a context prior;
/// it never overrides a confident guess).
pub fn plan(top0: &[Prediction], text: &str, last: Option<&str>) -> Plan {
    let top: Vec<Prediction> = match last {
        Some(l) if top0.len() >= 2 && top0[0].p < CUTOFF && top0[1].label == l && top0[0].label != l && top0[1].p >= 0.15 => {
            let mut v = vec![top0[1].clone()];
            v.extend(top0.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, p)| p.clone()));
            v
        }
        _ => top0.to_vec(),
    };
    let Some(first) = top.first() else {
        return if word_count(text) <= 2 { Plan::Fragment } else { Plan::Unknown };
    };
    let second0 = top.get(1);
    let both_functional = second0.is_some_and(|s| s.label != "none" && !first.label.starts_with("smalltalk") && !s.label.starts_with("smalltalk"));
    // sure enough and clearly ahead of the runner-up; a narrow win between two functions is asked back instead of guessed
    if first.label != "none" && first.p >= CUTOFF && !(both_functional && second0.unwrap().p >= 0.20 && first.p - second0.unwrap().p < 0.25) {
        return Plan::Run(first.label.clone());
    }
    let functional: Vec<&Prediction> = top.iter().filter(|p| p.label != "none" && !p.label.starts_with("smalltalk") && p.p >= 0.12).collect();
    if let Some(second) = top.get(1) {
        if first.label != "none" && second.label != "none" && first.p >= 0.30 && second.p >= 0.20 && first.p - second.p < 0.25 {
            return Plan::Clarify(first.label.clone(), second.label.clone());
        }
    }
    if first.label == "none" || first.p < 0.20 {
        if is_question(text) && word_count(text) >= 3 {
            return Plan::Knowledge;
        }
        if word_count(text) <= 3 {
            return Plan::Fragment;
        }
    }
    if !functional.is_empty() {
        return Plan::Suggest(functional.iter().take(2).map(|p| p.label.clone()).collect());
    }
    if is_question(text) {
        Plan::Knowledge
    } else {
        Plan::Unknown
    }
}

/// Plain names of what a label does, for questions back to the user.
pub fn name(label: &str, de: bool) -> String {
    let table: &[(&str, &str)] = if de { DE } else { EN };
    table.iter().find(|(l, _)| *l == label).map(|(_, n)| n.to_string()).unwrap_or_else(|| label.replace('_', " "))
}

pub fn message(plan: &Plan, de: bool, online_configured: bool, last_topic: Option<&str>) -> String {
    match plan {
        Plan::Clarify(a, b) => {
            if de {
                format!("Meinst du {} oder {}?", name(a, true), name(b, true))
            } else {
                format!("Do you mean {} or {}?", name(a, false), name(b, false))
            }
        }
        Plan::Suggest(labels) => {
            let names: Vec<String> = labels.iter().map(|l| name(l, de)).collect();
            if de {
                format!("Ich bin nicht sicher, was du genau willst. Meinst du {}? Sag es mir gern noch einmal etwas genauer.", names.join(" oder "))
            } else {
                format!("I am not sure what exactly you want. Do you mean {}? Say it once more, a bit more precisely.", names.join(" or "))
            }
        }
        Plan::Knowledge => {
            if online_configured {
                if de { "Das ist eine Wissensfrage, ich frage dafür die Online-KI." } else { "That is a knowledge question, so I ask the online AI." }.to_string()
            } else if de {
                "Das ist eine Wissensfrage. Dafür brauche ich die Online-KI, die ist aber nicht eingerichtet (Einstellungen, KI-Assistent). Offline kann ich Erinnerungen, Listen, Notizen, Timer, Wecker, Wetter, Kalender, Stundenplan, Anime, Musik, Anrufe, Navigation und mehr. Sag „Befehle“ für Beispiele.".to_string()
            } else {
                "That is a knowledge question. I need the online AI for those and it is not set up (Settings, AI assistant). Offline I can do reminders, lists, notes, timers, alarms, weather, calendar, timetable, anime, music, calls, navigation and more. Say “commands” for examples.".to_string()
            }
        }
        Plan::Fragment => match last_topic {
            Some(t) => {
                if de {
                    format!("Das habe ich nur halb verstanden. Meinst du das zu „{}“? Sag zum Beispiel „und morgen?“ oder die ganze Frage.", name(t, true))
                } else {
                    format!("I only half got that. Do you mean it about “{}”? Say for example “and tomorrow?” or the whole question.", name(t, false))
                }
            }
            None => if de {
                "Das war mir zu kurz. Worum geht es? Zum Beispiel: „Stell einen Timer auf zehn Minuten“ oder „Wie wird das Wetter morgen?“."
            } else {
                "That was too short for me. What is it about? For example: “Set a timer for ten minutes” or “What is the weather tomorrow?”."
            }
            .to_string(),
        },
        Plan::Unknown => if de {
            "Das habe ich nicht verstanden. Sag es bitte anders, zum Beispiel: „Wann ist mein nächster Termin?“ oder „Erkennst du dieses Lied?“."
        } else {
            "I did not understand that. Please say it differently, for example: “When is my next appointment?” or “Do you recognise this song?”."
        }
        .to_string(),
        Plan::Run(_) => String::new(),
    }
}

const DE: &[(&str, &str)] = &[
    ("weather", "das Wetter"), ("time", "die Uhrzeit"), ("date", "das Datum"), ("alarm_set", "einen Wecker stellen"), ("alarm_next", "deinen nächsten Wecker"),
    ("alarm_list", "deine Wecker"), ("timer_set", "einen Timer stellen"), ("timer_query", "die Restzeit des Timers"), ("timer_cancel", "den Timer abbrechen"),
    ("calendar_query", "deine Termine"), ("calendar_create", "einen Termin anlegen"), ("day_overview", "deine Tagesübersicht"), ("tasks_create", "eine Aufgabe anlegen"),
    ("tasks_list", "deine Aufgaben"), ("school_timetable", "deinen Stundenplan"), ("school_next_lesson", "deine nächste Stunde"), ("school_room", "deinen Raum"),
    ("school_exams", "deine Klausuren"), ("school_homework", "deine Hausaufgaben"), ("school_cancelled", "Ausfälle in der Schule"), ("school_messages", "Schulnachrichten"),
    ("anime_airing", "Anime heute"), ("anime_next", "deine nächste Anime-Folge"), ("anime_behind", "Anime, bei denen du hinterher bist"), ("anime_title", "einen Anime"), ("anime_progress", "deinen Anime-Fortschritt eintragen"), ("anime_status", "einen Anime-Status ändern"), ("anime_refresh", "den Anime-Tracker aktualisieren"), ("anime_changes", "verschobene Anime-Folgen"),
    ("music_play", "Musik abspielen"), ("music_pause", "die Musik pausieren"), ("music_next", "das nächste Lied"), ("music_previous", "das vorherige Lied"), ("song_recognize", "ein Lied erkennen"),
    ("lyrics", "einen Liedtext"), ("call", "jemanden anrufen"), ("navigation", "eine Navigation"), ("open_app", "eine App öffnen"), ("search_web", "eine Websuche"), ("translate", "eine Übersetzung"),
    ("calculator", "eine Rechnung"), ("battery", "den Akkustand"), ("flashlight_on", "die Taschenlampe einschalten"), ("flashlight_off", "die Taschenlampe ausschalten"),
    ("system_settings", "eine Einstellung ändern"), ("memory_remember", "dir etwas merken"), ("memory_recall", "dein Gedächtnis abfragen"), ("joke", "einen Witz"), ("smalltalk", "ein bisschen plaudern"),
    ("smalltalk_greeting", "eine Begrüßung"), ("smalltalk_howareyou", "wie es mir geht"), ("smalltalk_thanks", "ein Danke"), ("smalltalk_farewell", "eine Verabschiedung"), ("smalltalk_identity", "wer ich bin"),
    ("smalltalk_help", "was ich kann"), ("smalltalk_compliment", "ein Kompliment"), ("smalltalk_love", "eine Liebeserklärung"), ("smalltalk_insult", "eine Beschimpfung"), ("smalltalk_feeling_bad", "dass es dir schlecht geht"), ("smalltalk_feeling_good", "dass es dir gut geht"),
];

const EN: &[(&str, &str)] = &[
    ("weather", "the weather"), ("time", "the time"), ("date", "the date"), ("alarm_set", "setting an alarm"), ("alarm_next", "your next alarm"),
    ("alarm_list", "your alarms"), ("timer_set", "setting a timer"), ("timer_query", "the time left on the timer"), ("timer_cancel", "cancelling the timer"),
    ("calendar_query", "your appointments"), ("calendar_create", "creating an appointment"), ("day_overview", "your day overview"), ("tasks_create", "creating a task"),
    ("tasks_list", "your tasks"), ("school_timetable", "your timetable"), ("school_next_lesson", "your next lesson"), ("school_room", "your room"),
    ("school_exams", "your exams"), ("school_homework", "your homework"), ("school_cancelled", "cancelled lessons"), ("school_messages", "school messages"),
    ("anime_airing", "anime today"), ("anime_next", "your next anime episode"), ("anime_behind", "anime you are behind on"), ("anime_title", "an anime"), ("anime_progress", "recording anime progress"), ("anime_status", "changing an anime status"), ("anime_refresh", "refreshing the anime tracker"), ("anime_changes", "moved anime episodes"),
    ("music_play", "playing music"), ("music_pause", "pausing the music"), ("music_next", "the next song"), ("music_previous", "the previous song"), ("song_recognize", "recognising a song"),
    ("lyrics", "song lyrics"), ("call", "calling someone"), ("navigation", "navigation"), ("open_app", "opening an app"), ("search_web", "a web search"), ("translate", "a translation"),
    ("calculator", "a calculation"), ("battery", "the battery level"), ("flashlight_on", "turning the flashlight on"), ("flashlight_off", "turning the flashlight off"),
    ("system_settings", "changing a setting"), ("memory_remember", "remembering something"), ("memory_recall", "asking your memory"), ("joke", "a joke"), ("smalltalk", "a little chat"),
    ("smalltalk_greeting", "a greeting"), ("smalltalk_howareyou", "how I am"), ("smalltalk_thanks", "a thank you"), ("smalltalk_farewell", "a goodbye"), ("smalltalk_identity", "who I am"),
    ("smalltalk_help", "what I can do"), ("smalltalk_compliment", "a compliment"), ("smalltalk_love", "a declaration of love"), ("smalltalk_insult", "an insult"), ("smalltalk_feeling_bad", "that you feel bad"), ("smalltalk_feeling_good", "that you feel good"),
];

/// Whether an unsure sentence is worth a paid request to the online AI. A knowledge question is; a
/// sentence the network could not place and that is no real question is not (most often the speech
/// recogniser heard noise or nonsense): Iris asks back locally instead, which costs nothing.
pub fn worth_cloud(plan: &Plan, text: &str) -> bool {
    match plan {
        Plan::Knowledge => true,
        Plan::Unknown | Plan::Suggest(_) => (is_question(text) || asks_to_explain(text)) && word_count(text) >= 3,
        Plan::Run(_) | Plan::Clarify(_, _) | Plan::Fragment => false,
    }
}

/// "Erzähl mir etwas über ...", "Nenn drei Flüsse", "Explain ...": a knowledge request without a question mark.
fn asks_to_explain(text: &str) -> bool {
    re(r"(?iu)^(?:bitte\s+)?(?:erzähl\w*|nenn\w*|erklär\w*|beschreib\w*|sag mir|übersetz\w*|fass\w* .* zusammen|tell me|explain|describe|name|list|give me|summari[sz]e|translate)\b").contains(text.trim())
}

/// A real conversation ("ich fühle mich heute mies, weil ...") goes to the online AI; a set phrase
/// ("hi", "danke", "wie geht's") keeps Iris's own answer.
pub fn conversation(label: &str, text: &str) -> bool {
    matches!(label, "smalltalk" | "smalltalk_feeling_bad" | "smalltalk_feeling_good" | "smalltalk_love" | "smalltalk_insult") && word_count(text) >= 4
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    #[test]
    fn noise_and_fragments_stay_local_questions_go_out() {
        assert!(worth_cloud(&Plan::Knowledge, "wer hat den Eiffelturm gebaut"));
        assert!(!worth_cloud(&Plan::Unknown, "blau Fenster laufen Nudel"));
        assert!(!worth_cloud(&Plan::Unknown, "ähm also"));
        assert!(worth_cloud(&Plan::Unknown, "warum ist der Himmel blau"));
        assert!(worth_cloud(&Plan::Unknown, "erzähl mir etwas über den Eiffelturm"));
        assert!(worth_cloud(&Plan::Unknown, "Nenn drei Flüsse in Europa"));
        assert!(!worth_cloud(&Plan::Suggest(vec!["timer_set".into()]), "Timer Nudel Wasser"));
        assert!(!worth_cloud(&Plan::Fragment, "und dann?"));
        assert!(!worth_cloud(&Plan::Clarify("a".into(), "b".into()), "was meinst du"));
    }

    #[test]
    fn set_phrases_stay_local_conversation_goes_out() {
        assert!(!conversation("smalltalk_greeting", "hi wie geht es dir heute"));
        assert!(!conversation("smalltalk_feeling_bad", "mir gehts mies"));
        assert!(conversation("smalltalk_feeling_bad", "ich fühle mich heute richtig mies"));
        assert!(conversation("smalltalk", "ich hab heute meine Prüfung bestanden"));
    }
}
