//! Language packs: every word the engine itself knows about a language, loaded from data.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct LangPack {
    pub code: String,
    pub numbers: Numbers,
    pub time: TimeWords,
    pub dialog: DialogWords,
    pub format: Format,
    #[serde(default)]
    pub messages: Messages,
    /// Spelling the speech recogniser gets wrong, repaired word by word before anything is matched.
    #[serde(default, rename = "repair")]
    pub repairs: Vec<RepairSource>,
    #[serde(skip)]
    pub repair_rx: Vec<(regex::Regex, String)>,
    /// Rewrites of the whole text before it is split into words ("einkaufs liste" is one word).
    #[serde(default, rename = "normalize")]
    pub normalizers: Vec<RepairSource>,
    #[serde(skip)]
    pub normalize_rx: Vec<(regex::Regex, String)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepairSource {
    pub re: String,
    pub to: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Numbers {
    #[serde(default)]
    pub joiner: Option<String>,
    pub words: HashMap<String, i64>,
    #[serde(default)]
    pub tens: HashMap<String, i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TimeWords {
    pub in_words: Vec<String>,
    pub and_words: Vec<String>,
    pub clock_markers: Vec<String>,
    pub oclock: Vec<String>,
    pub half: Vec<String>,
    /// "before": the half hour before the named hour (German "halb fünf" = 4:30). "after": English "half past".
    pub half_means: String,
    pub quarter_past: Vec<String>,
    pub quarter_to: Vec<String>,
    pub past: Vec<String>,
    pub to: Vec<String>,
    pub am: Vec<String>,
    pub pm: Vec<String>,
    pub date_fillers: Vec<String>,
    pub date_joiners: Vec<String>,
    pub next_words: Vec<String>,
    pub every_words: Vec<String>,
    pub units: Units,
    pub fractions: HashMap<String, i64>,
    pub day_offsets: HashMap<String, i32>,
    #[serde(default)]
    pub period_after_day: HashMap<String, String>,
    pub weekdays: HashMap<String, u8>,
    pub months: HashMap<String, u8>,
    pub named: HashMap<String, String>,
    pub periods: Periods,
    pub repeat: RepeatWords,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Units {
    pub second: Vec<String>,
    pub minute: Vec<String>,
    pub hour: Vec<String>,
    pub day: Vec<String>,
    pub week: Vec<String>,
}

impl Units {
    /// Seconds in the unit this word names.
    pub fn seconds(&self, word: &str) -> Option<i64> {
        let has = |v: &Vec<String>| v.iter().any(|w| w == word);
        if has(&self.second) {
            Some(1)
        } else if has(&self.minute) {
            Some(60)
        } else if has(&self.hour) {
            Some(3600)
        } else if has(&self.day) {
            Some(86_400)
        } else if has(&self.week) {
            Some(604_800)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Periods {
    pub morning: Vec<String>,
    pub afternoon: Vec<String>,
    pub evening: Vec<String>,
    pub night: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepeatWords {
    pub daily: Vec<String>,
    pub weekdays: Vec<String>,
    pub weekly: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DialogWords {
    pub fillers: Vec<String>,
    pub cancel: Vec<String>,
    #[serde(default)]
    pub undo: Vec<String>,
    /// Small words around a cancel word that do not change its meaning: "nein, abbrechen", "ok stopp".
    #[serde(default)]
    pub soft: Vec<String>,
    /// "Say that again": the last answer, read once more.
    #[serde(default)]
    pub say_again: Vec<String>,
    /// "Do that again": the last request, run once more.
    #[serde(default)]
    pub repeat: Vec<String>,
    pub yes: Vec<String>,
    pub no: Vec<String>,
    pub done: Vec<String>,
}

impl DialogWords {
    fn is(list: &[String], text: &str) -> bool {
        list.iter().any(|w| w == text)
    }
    pub fn is_cancel(&self, text: &str) -> bool {
        Self::is(&self.cancel, text)
    }
    /// Whether the words are a way of backing out: a cancel phrase, or several of them in a row ("stopp vergiss es"),
    /// with soft words ("nein", "ok") allowed around them.
    pub fn cancels(&self, words: &[String]) -> bool {
        fn covered(phrases: &[Vec<&str>], w: &[&str]) -> bool {
            if w.is_empty() {
                return true;
            }
            phrases.iter().any(|p| w.len() >= p.len() && w[..p.len()] == p[..] && covered(phrases, &w[p.len()..]))
        }
        let phrases: Vec<Vec<&str>> = self.cancel.iter().map(|c| c.split(' ').collect()).collect();
        let mut w: Vec<&str> = words.iter().map(String::as_str).collect();
        if !w.is_empty() && covered(&phrases, &w) {
            return true;
        }
        while w.first().is_some_and(|x| self.soft.iter().any(|s| s == x)) {
            w.remove(0);
        }
        while w.last().is_some_and(|x| self.soft.iter().any(|s| s == x)) {
            w.pop();
        }
        !w.is_empty() && covered(&phrases, &w)
    }

    /// Whether the words are only a "no" ("nein", "nee").
    pub fn only_no(&self, words: &[String]) -> bool {
        !words.is_empty() && words.iter().all(|w| self.no.iter().any(|n| n == w))
    }

    pub fn is_say_again(&self, text: &str) -> bool {
        Self::is(&self.say_again, text)
    }
    pub fn is_repeat(&self, text: &str) -> bool {
        Self::is(&self.repeat, text)
    }
    pub fn is_undo(&self, text: &str) -> bool {
        Self::is(&self.undo, text)
    }
    pub fn is_yes(&self, text: &str) -> bool {
        Self::is(&self.yes, text)
    }
    pub fn is_no(&self, text: &str) -> bool {
        Self::is(&self.no, text)
    }
    pub fn is_done(&self, text: &str) -> bool {
        Self::is(&self.done, text)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Format {
    pub clock: String,
    pub date: String,
    pub weekday_names: Vec<String>,
    pub month_names: Vec<String>,
    pub list_and: String,
}

/// Texts by key, with `{name}` placeholders. Plural forms are the keys `<key>.one` and `<key>.other`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct Messages(pub HashMap<String, String>);

impl Messages {
    /// The text for `key` with its placeholders filled. A missing key yields the key itself, so a gap is visible, not silent.
    pub fn get(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut out = self.0.get(key).cloned().unwrap_or_else(|| format!("⟨{key}⟩"));
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }

    pub fn plural(&self, key: &str, n: i64, args: &[(&str, &str)]) -> String {
        let form = if n == 1 { "one" } else { "other" };
        let count = n.to_string();
        let mut all: Vec<(&str, &str)> = vec![("n", &count)];
        all.extend_from_slice(args);
        self.get(&format!("{key}.{form}"), &all)
    }

    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
}

const DE: &str = include_str!("../../../lang/de.toml");
const EN: &str = include_str!("../../../lang/en.toml");

impl LangPack {
    pub fn from_toml(source: &str) -> Result<Self, toml::de::Error> {
        let mut pack: LangPack = toml::from_str(source)?;
        for r in &pack.normalizers {
            let re = regex::Regex::new(&r.re).map_err(|e| <toml::de::Error as serde::de::Error>::custom(format!("normalize {:?}: {e}", r.re)))?;
            pack.normalize_rx.push((re, r.to.clone()));
        }
        for r in &pack.repairs {
            let re = regex::Regex::new(&r.re).map_err(|e| <toml::de::Error as serde::de::Error>::custom(format!("repair {:?}: {e}", r.re)))?;
            pack.repair_rx.push((re, r.to.clone()));
        }
        Ok(pack)
    }

    /// The text with the language's whole-text rewrites applied.
    pub fn normalize(&self, text: &str) -> String {
        self.normalize_rx.iter().fold(text.to_string(), |t, (re, to)| re.replace_all(&t, to.as_str()).into_owned())
    }

    /// One normalised word with the language's repairs applied.
    pub fn repair(&self, word: &str) -> String {
        self.repair_rx.iter().fold(word.to_string(), |w, (re, to)| re.replace(&w, to.as_str()).into_owned())
    }

    /// The packs compiled into the engine. More can be added at run time with [`LangPack::from_toml`].
    pub fn builtin() -> Vec<LangPack> {
        [DE, EN].iter().map(|s| Self::from_toml(s).expect("built-in language pack is valid")).collect()
    }

    /// The number a token names: digits ("15", "3rd"), a number word, or a joined compound ("einundzwanzig").
    pub fn number(&self, token: &str) -> Option<i64> {
        let digits = token.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && (digits.len() == token.len() || token.len() - digits.len() <= 2) {
            return digits.parse().ok();
        }
        if let Some(v) = self.numbers.words.get(token).or_else(|| self.numbers.tens.get(token)) {
            return Some(*v);
        }
        let joiner = self.numbers.joiner.as_deref()?;
        let at = token.find(joiner)?;
        let unit = self.numbers.words.get(&token[..at])?;
        let tens = self.numbers.tens.get(&token[at + joiner.len()..])?;
        (1..10).contains(unit).then_some(unit + tens)
    }

    /// The number starting at `tokens[i]` and how many tokens it spans ("twenty one" spans two).
    pub fn number_at(&self, tokens: &[&str], i: usize) -> Option<(i64, usize)> {
        let first = self.number(tokens.get(i)?)?;
        if self.numbers.tens.contains_key(tokens[i]) {
            if let Some(unit) = tokens.get(i + 1).and_then(|t| self.numbers.words.get(*t)) {
                if (1..10).contains(unit) {
                    return Some((first + unit, 2));
                }
            }
        }
        Some((first, 1))
    }

    /// How many tokens the longest of `phrases` covers at `tokens[i]`, if any matches there.
    pub fn phrase_at(tokens: &[&str], i: usize, phrases: &[String]) -> Option<usize> {
        phrases
            .iter()
            .filter_map(|p| {
                let words: Vec<&str> = p.split(' ').collect();
                (tokens.len() >= i + words.len() && tokens[i..i + words.len()] == words[..]).then_some(words.len())
            })
            .max()
    }

    /// Like [`Self::phrase_at`] for a phrase-to-value table: the value and the tokens covered.
    pub fn lookup_at<'a, V>(tokens: &[&str], i: usize, table: &'a HashMap<String, V>) -> Option<(&'a V, usize)> {
        table
            .iter()
            .filter_map(|(p, v)| {
                let words: Vec<&str> = p.split(' ').collect();
                (tokens.len() >= i + words.len() && tokens[i..i + words.len()] == words[..]).then_some((v, words.len()))
            })
            .max_by_key(|(_, n)| *n)
    }
}
