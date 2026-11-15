//! Which language a sentence is in, by its words; and the politeness and stumbling taken out of it.

use crate::re::{group, re, Find};

const GERMAN: &[&str] = &[
    "wie", "viel", "kosten", "ich", "mir", "mich", "mein", "meine", "meinen", "der", "die", "das", "nicht", "ist", "was", "wo", "wann", "bitte",
    "öffne", "suche", "zeig", "zeige", "spiel", "spiele", "ruf", "an", "und", "ein", "eine", "kein", "welche", "welcher", "heute", "morgen",
    "wetter", "wecker", "kalender", "liedtext", "übersetze", "nach", "für", "von", "auf", "mit", "wird", "gibt", "habe", "hast", "kannst", "erzähl",
    "erkenne", "lied", "starte", "aktuell", "wieviel", "warum", "wer", "wieso", "noch", "mal",
    // the own features: lists, reminders, notes, timers; said on a German phone they must not read as English
    "einen", "einem", "erstelle", "erstell", "stelle", "stell", "setze", "setz", "füge", "füg", "lösche", "hake", "notiere", "erinnere", "erinner", "erinnerung",
    "notiz", "liste", "zur", "zum", "dem", "den", "minuten", "minute", "stunde", "stunden", "sekunden", "uhr", "nen", "pack", "packe", "streich", "entferne",
];
const ENGLISH: &[&str] = &[
    "the", "what", "whats", "is", "my", "open", "please", "how", "weather", "search", "for", "to", "me", "you", "do", "and", "of", "in", "on",
    "start", "play", "call", "set", "tell", "joke", "translate", "lyrics", "much", "many", "are", "this", "that", "why", "who",
];

/// "de" or "en" by the words of the sentence; `default` when they do not tell.
pub fn detect(text: &str, default: &str) -> String {
    let lowered = re(r"[^\p{L}' ]").sub(&text.to_lowercase(), " ");
    let words: Vec<&str> = lowered.split(' ').filter(|w| !w.trim().is_empty()).collect();
    if words.is_empty() {
        return default.to_string();
    }
    let de = words.iter().filter(|w| GERMAN.contains(w)).count() + if text.chars().any(|c| "äöüß".contains(c)) { 2 } else { 0 };
    let en = words.iter().filter(|w| ENGLISH.contains(w)).count();
    let lang = if de >= 2 && de > en {
        "de"
    } else if en >= 2 && en > de {
        "en"
    } else if de == 1 && en == 0 && words.len() <= 4 {
        // a short command with a single telling word and nothing of the other language ("Open WhatsApp", "Öffne Kamera")
        "de"
    } else if en == 1 && de == 0 && words.len() <= 4 {
        "en"
    } else {
        default
    };
    lang.to_string()
}

/// "Please show me the um the text" becomes "show me the the text". Also fixes the recogniser's most
/// common slip, "whether" for "weather", and "nein" for "neun" in a sum.
pub fn polite(raw: &str) -> String {
    let mut t = raw.trim().to_string();
    t = re(r"(?iu)\b(?:um|uh|uhm|er|äh|ähm|ehm|also|mal eben)\b[,]?\s*").sub(&t, "");
    t = re(r"(?iu)^(?:(?:hey |hi |ok |okay |and |und |also |dann )?(?:iris[, ]+)?)(?:(?:please|bitte|could you|can you|would you|will you|kannst du|könntest du|koenntest du|würdest du|magst du|i want you to|i would like you to|ich möchte dass du|ich will dass du|ich möchte|ich will)\s+)+").sub(&t, "");
    t = re(r"(?iu)[\s,]+(?:please|bitte|mal|doch|eben)$").sub(&t, "");
    if re(r"(?iu)\b(?:plus|minus|mal|durch|geteilt|times|divided|multiplied)\b").contains(&t) {
        t = re(r"(?iu)\bnein\b").sub(&t, "neun");
    }
    t = re(r"(?iu)^(?:feather|beather|ther|leather|vether|wether)(?= (?:the )?(?:next|this|today|tomorrow|week|weekend|in |for ))").sub(&t, "weather");
    t = re(r"(?iu)^whether\b").sub(&t, "weather");
    t = re(r"(?iu)\b(?:the )?whether (today|tomorrow|in|for|forecast)").sub_with(&t, &|c| format!("weather {}", group(c, 1)));
    let out = t.trim();
    if out.trim().is_empty() {
        raw.trim().to_string()
    } else {
        out.to_string()
    }
}
