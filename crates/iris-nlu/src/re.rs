//! Regular expressions as the app wrote them. Unicode throughout, as on Android (ICU): `\b`, `\w`, `\d`,
//! `\s` and `(?i)` know letters beyond ASCII.

use fancy_regex::Regex;

/// The groups of one match.
pub type Caps<'t> = fancy_regex::Captures<'t, str>;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const WORD: &str = r"[\p{L}\p{M}\p{N}_]";

/// Java's inline `u` flag (Unicode case) is Rust's default; the regex crates do not take it.
fn java_flags(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    while let Some(i) = rest.find("(?") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let flags: String = after.chars().take_while(|c| c.is_ascii_alphabetic() || *c == '-').collect();
        let next = after[flags.len()..].chars().next();
        if !flags.is_empty() && matches!(next, Some(')') | Some(':')) {
            out.push_str("(?");
            out.push_str(&flags.replace('u', ""));
        } else {
            out.push_str("(?");
            out.push_str(&flags);
        }
        rest = &after[flags.len()..];
    }
    out.push_str(rest);
    out.replace("(?)", "")
}

fn compile(pattern: &str) -> Arc<Regex> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<Regex>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(r) = cache.lock().unwrap().get(pattern) {
        return r.clone();
    }
    let r = Arc::new(Regex::new(pattern).unwrap_or_else(|e| panic!("pattern {pattern:?}: {e}")));
    cache.lock().unwrap().insert(pattern.to_string(), r.clone());
    r
}

/// A pattern of the app's `Regex(...)`.
pub fn re(pattern: &str) -> Arc<Regex> {
    compile(&java_flags(pattern))
}

/// A pattern of the app's `nluRegex(...)`: word boundaries, digits and spaces spelled out over Unicode classes.
pub fn nlu(pattern: &str) -> Arc<Regex> {
    let boundary = format!("(?:(?<!{WORD})(?={WORD})|(?<={WORD})(?!{WORD}))");
    let p = java_flags(pattern).replace(r"\b", &boundary).replace(r"\d", r"\p{Nd}").replace(r"\s", r"[\p{Z}\s\u{85}]");
    compile(&p)
}

/// Escapes a literal for use inside a pattern (Kotlin's `Regex.escape` quotes it; the effect is the same).
pub fn escape(s: &str) -> String {
    fancy_regex::escape(s).into_owned()
}

pub trait Find {
    fn found<'t>(&self, text: &'t str) -> Option<Caps<'t>>;
    fn found_all<'t>(&self, text: &'t str) -> Vec<Caps<'t>>;
    fn contains(&self, text: &str) -> bool;
    /// The whole text matches (Kotlin's `matches`).
    fn whole(&self, text: &str) -> bool;
    /// Every match replaced by `with` (literal).
    fn sub(&self, text: &str, with: &str) -> String;
    fn sub_with(&self, text: &str, f: &dyn Fn(&Caps) -> String) -> String;
    /// The pieces between the matches, empty ones included (Kotlin's `split(Regex)`).
    fn split_all<'t>(&self, text: &'t str) -> Vec<&'t str>;
}

impl Find for Regex {
    fn found<'t>(&self, text: &'t str) -> Option<Caps<'t>> {
        self.captures(text).ok().flatten()
    }
    fn found_all<'t>(&self, text: &'t str) -> Vec<Caps<'t>> {
        self.captures_iter(text).filter_map(Result::ok).collect()
    }
    fn contains(&self, text: &str) -> bool {
        self.is_match(text).unwrap_or(false)
    }
    fn whole(&self, text: &str) -> bool {
        compile(&format!("^(?:{})$", self.as_str())).is_match(text).unwrap_or(false)
    }
    fn sub(&self, text: &str, with: &str) -> String {
        self.sub_with(text, &|_| with.to_string())
    }
    fn sub_with(&self, text: &str, f: &dyn Fn(&Caps) -> String) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for c in self.found_all(text) {
            let m = c.get(0).unwrap();
            out.push_str(&text[last..m.start()]);
            out.push_str(&f(&c));
            last = m.end();
        }
        out.push_str(&text[last..]);
        out
    }
    fn split_all<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let mut out = Vec::new();
        let mut last = 0;
        for c in self.found_all(text) {
            let m = c.get(0).unwrap();
            out.push(&text[last..m.start()]);
            last = m.end();
        }
        out.push(&text[last..]);
        out
    }
}

/// The text of a group, empty when it did not take part (Kotlin's `groupValues`).
pub fn group<'t>(c: &Caps<'t>, i: usize) -> &'t str {
    c.get(i).map(|m| m.as_str()).unwrap_or("")
}

/// The text with the byte range replaced (Kotlin's `replaceRange`).
pub fn replace_range(text: &str, start: usize, end: usize, with: &str) -> String {
    format!("{}{}{}", &text[..start], with, &text[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_flags_lose_u() {
        assert_eq!(java_flags("(?iu)\\bx"), "(?i)\\bx");
        assert_eq!(java_flags("(?i)x(?:y)"), "(?i)x(?:y)");
        assert_eq!(java_flags("(?u)x"), "x");
        assert_eq!(java_flags("(?<!a)b(?=c)"), "(?<!a)b(?=c)");
    }

    #[test]
    fn unicode_boundaries_as_on_android() {
        assert!(re(r"(?iu)\büber\b").contains("Über"));
        assert!(nlu(r"\büber\b").contains("über morgen"));
        assert!(!nlu(r"\bber\b").contains("über"));
    }

    #[test]
    fn whole_means_the_entire_text() {
        assert!(re("(?i)ab|abc").whole("abc"));
        assert!(!re("ab").whole("abc"));
    }
}
