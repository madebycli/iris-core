//! A sentence as the readers see it: NFC, lower case, number words as digits, no punctuation but `:` `'` `.`.

use crate::re::{escape, nlu, Find};
use std::collections::HashMap;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

const SMALL_DE: [&str; 20] = ["null", "eins", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun", "zehn", "elf", "zwölf", "dreizehn", "vierzehn", "fünfzehn", "sechzehn", "siebzehn", "achtzehn", "neunzehn"];
const SMALL_EN: [&str; 20] = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];

static NUMBERS: LazyLock<HashMap<String, u32>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    for (n, w) in SMALL_DE.iter().enumerate() {
        m.insert(w.to_string(), n as u32);
    }
    for (n, w) in SMALL_EN.iter().enumerate() {
        m.insert(w.to_string(), n as u32);
    }
    for (i, tens) in ["zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig"].iter().enumerate() {
        let n = (i as u32 + 2) * 10;
        m.insert(tens.to_string(), n);
        for (unit, word) in SMALL_DE.iter().enumerate().take(10).skip(1) {
            let u = if unit == 1 { "ein" } else { word };
            m.insert(format!("{u}und{tens}"), n + unit as u32);
        }
    }
    for (i, tens) in ["twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"].iter().enumerate() {
        let n = (i as u32 + 2) * 10;
        m.insert(tens.to_string(), n);
        for (unit, word) in SMALL_EN.iter().enumerate().take(10).skip(1) {
            m.insert(format!("{tens} {word}"), n + unit as u32);
        }
    }
    for w in ["ein", "eine", "einen", "einer"] {
        m.insert(w.to_string(), 1);
    }
    m
});

static NUMBER_PATTERN: LazyLock<String> = LazyLock::new(|| {
    let mut keys: Vec<&String> = NUMBERS.keys().collect();
    keys.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    format!(r"(?<![\p{{L}}\d])(?:{})(?![\p{{L}}\d])", keys.iter().map(|k| escape(k)).collect::<Vec<_>>().join("|"))
});

/// The first `n` UTF-16 units of a text, as Kotlin's `take(n)` (never splitting a character).
pub fn take_utf16(text: &str, n: usize) -> &str {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &text[..i];
        }
    }
    text
}

pub fn normalize(input: &str) -> String {
    let text: String = take_utf16(input, 4000).nfc().collect::<String>().to_lowercase().replace('’', "'").replace('-', " ");
    let text = nlu(r"[^\p{L}\p{N}:' .]").sub(&text, " ");
    let text = nlu(r"\s+").sub(&text, " ");
    let text = text.trim();
    let text = nlu(&NUMBER_PATTERN).sub_with(text, &|c| NUMBERS[c.get(0).unwrap().as_str()].to_string());
    nlu(r"[.?!]+$").sub(&text, "").trim().to_string()
}

/// Length in UTF-16 units, as Kotlin's `String.length`.
pub fn len_utf16(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// Kotlin's `trim()`: whitespace at both ends.
pub fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace())
}

/// A whole number as Kotlin's `toIntOrNull` reads it (ASCII digits, optional sign).
pub fn int(s: &str) -> Option<i64> {
    if s.is_empty() || s.len() > 19 {
        return None;
    }
    s.parse::<i64>().ok().filter(|v| *v <= i32::MAX as i64 && *v >= i32::MIN as i64)
}

/// As Kotlin's `toLongOrNull`.
pub fn long(s: &str) -> Option<i64> {
    s.parse::<i64>().ok()
}
