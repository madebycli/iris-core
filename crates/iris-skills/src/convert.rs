//! Converting between units: "fünf Meilen in Kilometer", "20 Grad Celsius in Fahrenheit".
//!
//! What the units are is one table (`data/convert/units.toml`); what they are called is in the
//! language files. The skill only claims a sentence when both units are known and comparable.

use iris_core::{Cx, Reply, Skill};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

pub struct Convert;

#[derive(Deserialize)]
struct Unit {
    dimension: String,
    scale: f64,
    #[serde(default)]
    offset: f64,
}

fn units() -> &'static HashMap<String, Unit> {
    static UNITS: OnceLock<HashMap<String, Unit>> = OnceLock::new();
    UNITS.get_or_init(|| toml::from_str(include_str!("../data/convert/units.toml")).expect("the unit table is valid"))
}

/// `value` in unit `from`, said in unit `to`. `None` when the units are unknown or measure different things.
pub fn convert(value: f64, from: &str, to: &str) -> Option<f64> {
    let (a, b) = (units().get(from)?, units().get(to)?);
    if a.dimension != b.dimension || !value.is_finite() {
        return None;
    }
    Some((value * a.scale + a.offset - b.offset) / b.scale).filter(|v| v.is_finite())
}

impl Skill for Convert {
    fn manifest(&self) -> &'static str {
        include_str!("../data/convert/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/convert/de.toml")),
            "en" => Some(include_str!("../data/convert/en.toml")),
            _ => None,
        }
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        if intent != "convert" {
            return None;
        }
        let value = number(cx)?;
        let from_said = unit_words(cx, "from");
        let to_said = unit_words(cx, "to");
        let from = cx.table("units", &from_said)?.to_string();
        let to = cx.table("units", &to_said)?.to_string();
        let Some(result) = convert(value, &from, &to) else {
            return Some(Reply::failed(cx.msg("incompatible", &[])));
        };
        let shown_from = cx.table("names", &from).unwrap_or(&from_said).to_string();
        let shown_to = cx.table("names", &to).unwrap_or(&to_said).to_string();
        let sep = cx.msg("decimal", &[]);
        Some(Reply::done(cx.msg("result", &[("value", &format_number(value, &sep)), ("from", &shown_from), ("result", &format_number(result, &sep)), ("to", &shown_to)])))
    }
}

/// The unit's words with the "degree" word taken off ("grad celsius" is "celsius").
fn unit_words(cx: &Cx, slot: &str) -> String {
    let said = cx.utt.norm_of(cx.span(slot));
    let mut words: Vec<&str> = said.split(' ').collect();
    while words.len() > 1 && cx.list("degree").iter().any(|d| d == words[0]) {
        words.remove(0);
    }
    words.join(" ")
}

/// The number in the slot `value`: "5", "5,5", "-3", "minus drei", "fünf".
fn number(cx: &Cx) -> Option<f64> {
    let span = cx.slots.get("value")?;
    let words: Vec<&str> = cx.utt.tokens[span].iter().map(|t| t.norm.as_str()).collect();
    let (negative, rest) = match words.split_first() {
        Some((first, rest)) if cx.list("minus").iter().any(|m| m == first) => (true, rest),
        _ => (false, &words[..]),
    };
    let value = match rest {
        [one] => one.replace(',', ".").parse::<f64>().ok().or_else(|| cx.lang.number(one).map(|n| n as f64))?,
        _ => {
            let (n, used) = cx.lang.number_at(rest, 0)?;
            (used == rest.len()).then_some(n as f64)?
        }
    };
    Some(if negative { -value } else { value })
}

/// At most six decimals, no trailing zeros, with the language's decimal sign.
fn format_number(value: f64, sep: &str) -> String {
    let mut s = format!("{:.6}", value);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    s.replace('.', sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_relate() {
        assert!((convert(5.0, "mile", "kilometre").unwrap() - 8.04672).abs() < 1e-9);
        assert!((convert(100.0, "celsius", "fahrenheit").unwrap() - 212.0).abs() < 1e-9);
        assert!((convert(0.0, "kelvin", "celsius").unwrap() + 273.15).abs() < 1e-9);
        assert_eq!(convert(1.0, "metre", "gram"), None);
        assert_eq!(format_number(8.04672, ","), "8,04672");
        assert_eq!(format_number(212.0, ","), "212");
    }
}
