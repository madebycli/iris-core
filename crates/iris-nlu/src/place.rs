//! The place a weather question is about ("in Bad Homburg", "für Köln"), taken from its clause, not
//! from everything after the first preposition. "hier" and "dort" never silently mean a default city.

use crate::normalize::{len_utf16, trim};
use crate::re::{nlu, Find};

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceExtraction {
    pub place: Option<String>,
    pub needs_clarification: bool,
}

const NOT_PLACES: [&str; 15] = ["in", "für", "fuer", "for", "mich", "dich", "uns", "mir", "me", "us", "you", "hier", "dort", "here", "there"];
const LOWER_WORDS: [&str; 10] = ["am", "an", "der", "de", "upon", "im", "in", "la", "le", "of"];

const CLAUSE_BOUNDARY: &str = concat!(
    r"\b(?:kannst du|könntest du|können sie|can you|could you|would you|",
    r"wie|was|what|how|das wetter|the weather|wetter|weather|wird|werden|ist|is|will|regnet|regnen|rain|raining|",
    r"bin ich|ich bin|i am|i'm|bitte|please|und|and|aber|but|mit dem|with the|",
    r"sag|sagen|erzähl|tell|danke|thanks|here|there|hier|dort)\b"
);

fn clause_start(tail: &str) -> Option<usize> {
    nlu(CLAUSE_BOUNDARY).found(tail).map(|m| m.get(0).unwrap().start())
}

/// Short place replies ("Köln").
pub fn extract(without_date_and_time: &str) -> Option<String> {
    resolve(without_date_and_time).place
}

pub fn resolve(without_date_and_time: &str) -> PlaceExtraction {
    let t = nlu(r"\s+").sub(without_date_and_time, " ");
    let t = trim(&t).to_string();
    let mut named: Vec<String> = Vec::new();
    let mut contextual: Vec<String> = Vec::new();
    let mut invalid = false;
    let current = nlu(r"(?:\bich bin(?: gerade| jetzt| aktuell| zurzeit| momentan)?|\bich befinde mich(?: gerade| jetzt| aktuell)?|\bi(?: am|'m)(?: currently| now| staying)?)\s*$");
    for m in nlu(r"\b(?:in|für|fuer|for)\s+").found_all(&t) {
        let whole = m.get(0).unwrap();
        let tail = &t[whole.end()..];
        // a request's "for me" or the remaining "for in ..." is not another place
        let first_word = tail.split(' ').next().unwrap_or("");
        if NOT_PLACES.contains(&first_word) {
            continue;
        }
        match city(tail) {
            None => {
                if !trim(tail).is_empty() && clause_start(tail) != Some(0) {
                    invalid = true;
                }
            }
            Some(c) => {
                if current.contains(&t[..whole.start()]) {
                    contextual.push(c);
                } else {
                    named.push(c);
                }
            }
        }
    }
    let direct = distinct(named);
    let contexts = distinct(contextual);
    // an explicit forecast location overrides "I'm in Cologne ... weather in Paris"
    if direct.len() == 1 && !invalid {
        return PlaceExtraction { place: direct.into_iter().next(), needs_clarification: false };
    }
    if direct.len() > 1 || invalid {
        return PlaceExtraction { place: None, needs_clarification: true };
    }
    if contexts.len() == 1 {
        return PlaceExtraction { place: contexts.into_iter().next(), needs_clarification: false };
    }
    if contexts.len() > 1 {
        return PlaceExtraction { place: None, needs_clarification: true };
    }
    if let Some(m) = nlu(r"^(?:wetter|weather)\s+").found(&t) {
        let tail = &t[m.get(0).unwrap().end()..];
        if let Some(c) = city(tail) {
            return PlaceExtraction { place: Some(c), needs_clarification: false };
        }
        if !trim(tail).is_empty() && clause_start(tail) != Some(0) {
            return PlaceExtraction { place: None, needs_clarification: true };
        }
    }
    PlaceExtraction { place: None, needs_clarification: nlu(r"\b(?:hier|dort|da|here|there)\b").contains(&t) }
}

fn distinct(v: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for x in v {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    out
}

/// Kotlin's `trim(' ', '.', '?', '!')`.
fn trim_marks(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '.' | '?' | '!'))
}

/// Kotlin's `replaceFirstChar { it.titlecase(Locale.ROOT) }`.
pub(crate) fn title_first(word: &str) -> String {
    let mut chars = word.chars();
    let Some(c) = chars.next() else { return String::new() };
    let rest: String = chars.collect();
    let head = match c {
        'ǆ' | 'ǅ' | 'Ǆ' => "ǅ".to_string(),
        'ǉ' | 'ǈ' | 'Ǉ' => "ǈ".to_string(),
        'ǌ' | 'ǋ' | 'Ǌ' => "ǋ".to_string(),
        'ǳ' | 'ǲ' | 'Ǳ' => "ǲ".to_string(),
        _ => {
            let upper: String = c.to_uppercase().collect();
            if upper.chars().count() > 1 && c != 'ŉ' {
                let mut u = upper.chars();
                let first = u.next().unwrap();
                format!("{first}{}", u.as_str().to_lowercase())
            } else {
                upper
            }
        }
    };
    head + &rest
}

fn city(tail: &str) -> Option<String> {
    let end = clause_start(tail).unwrap_or(tail.len());
    let city = trim_marks(&tail[..end]);
    let city = nlu(r"\s+(?:um|at|on|am)$").sub(city, "");
    let city = trim(&city).to_string();
    if city.is_empty() || len_utf16(&city) > 100 || nlu(r"\p{Nd}").contains(&city) {
        return None;
    }
    let words: Vec<&str> = city.split(' ').collect();
    if words.len() > 8
        || NOT_PLACES.contains(&words[0])
        || city == "da"
        || nlu(r"\b(?:heute|morgen|today|tomorrow|hours|stunden|es|the|like)\b").contains(&city)
        || nlu(r"\b(?:oder|or)\s+\p{L}").contains(&city)
    {
        return None;
    }
    Some(words.iter().enumerate().map(|(i, w)| if i > 0 && LOWER_WORDS.contains(w) { w.to_string() } else { title_first(w) }).collect::<Vec<_>>().join(" "))
}
