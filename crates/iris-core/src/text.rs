//! From what was said to tokens the engine can match, without losing how the words were written.

use crate::lang::LangPack;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct Token {
    /// Lower-case, punctuation removed: what patterns and the time parser see.
    pub norm: String,
    /// As the user wrote or said it, for titles ("Dr. Müller").
    pub orig: String,
    /// A comma or semicolon followed the word: where a spoken list breaks.
    pub sep: bool,
}

/// One utterance, tokenised. `norm` is the normalised tokens joined by single spaces.
#[derive(Debug, Clone)]
pub struct Utterance {
    pub raw: String,
    pub tokens: Vec<Token>,
    pub norm: String,
    starts: Vec<usize>,
}

const EDGE: &[char] = &[',', '.', '!', '?', ';', ':', '"', '„', '“', '”', '(', ')', '…', '\'', '‚', '‘', '’'];

impl Utterance {
    pub fn new(raw: &str, lang: &LangPack) -> Self {
        let mut tokens = Vec::new();
        for piece in raw.split_whitespace() {
            let orig = piece.trim_matches(EDGE);
            if orig.is_empty() {
                continue;
            }
            let norm = lang.repair(&orig.to_lowercase().replace('’', "'"));
            if lang.dialog.fillers.contains(&norm) {
                continue;
            }
            tokens.push(Token { norm, orig: orig.to_string(), sep: piece.ends_with([',', ';']) });
        }
        Self::from_tokens(raw, tokens)
    }

    fn from_tokens(raw: &str, tokens: Vec<Token>) -> Self {
        let mut norm = String::new();
        let mut starts = Vec::with_capacity(tokens.len());
        for t in &tokens {
            if !norm.is_empty() {
                norm.push(' ');
            }
            starts.push(norm.len());
            norm.push_str(&t.norm);
        }
        Self { raw: raw.to_string(), tokens, norm, starts }
    }

    pub fn norms(&self) -> Vec<&str> {
        self.tokens.iter().map(|t| t.norm.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// The tokens a byte range of `norm` (a regex capture) covers.
    pub fn token_range(&self, bytes: Range<usize>) -> Range<usize> {
        let first = (0..self.len()).find(|&i| self.starts[i] + self.tokens[i].norm.len() > bytes.start).unwrap_or(self.len());
        let end = (0..self.len()).find(|&i| self.starts[i] >= bytes.end).unwrap_or(self.len());
        first..end.max(first)
    }

    /// The original words of the tokens selected by `keep`, joined.
    pub fn orig_where(&self, range: Range<usize>, keep: impl Fn(usize) -> bool) -> String {
        range.filter(|i| keep(*i)).map(|i| self.tokens[i].orig.as_str()).collect::<Vec<_>>().join(" ")
    }

    pub fn norm_of(&self, range: Range<usize>) -> String {
        self.tokens[range].iter().map(|t| t.norm.as_str()).collect::<Vec<_>>().join(" ")
    }
}

/// First letter upper-case, the rest untouched.
pub fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
