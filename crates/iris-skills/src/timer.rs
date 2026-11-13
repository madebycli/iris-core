//! Timers: set, ask how long is left, cancel. Several can run at once, each with a name.

use iris_core::time::{describe_duration, scan_duration};
use iris_core::{Card, Cx, Entry, EntryKind, Reply, Skill};
use serde_json::json;

pub struct Timer;

impl Skill for Timer {
    fn manifest(&self) -> &'static str {
        include_str!("../data/timer/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/timer/de.toml")),
            "en" => Some(include_str!("../data/timer/en.toml")),
            _ => None,
        }
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        match intent {
            "set" => set(cx, None),
            "set.duration" => {
                let name = cx.state["name"].as_str().unwrap_or_default().to_string();
                set(cx, Some(name))
            }
            "query" => Some(query(cx)),
            "cancel" | "cancel.which" => Some(cancel(cx, intent == "cancel.which")),
            _ => None,
        }
    }
}

fn running(cx: &Cx) -> Vec<Entry> {
    cx.store.all(Some(EntryKind::Timer)).into_iter().filter(|e| e.pending).collect()
}

/// `answering` carries the name from the first turn when this is the reply to "how long?".
fn set(cx: &mut Cx, answering: Option<String>) -> Option<Reply> {
    let norms = cx.utt.norms();
    let mut found = scan_duration(&norms, cx.lang);
    if found.is_none() && answering.is_some() && norms.len() == 1 {
        // "ten" as an answer means minutes
        found = cx.lang.number(norms[0]).map(|n| (n * 60, vec![true]));
    }
    let used = found.as_ref().map(|f| f.1.clone()).unwrap_or_else(|| vec![false; norms.len()]);
    let name = answering.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| name_in(cx, &used));
    let Some((secs, _)) = found else {
        cx.ask("set.duration", json!({ "name": name }));
        return Some(Reply::question(cx.msg("ask_duration", &[])));
    };
    if !(1..=604_800).contains(&secs) {
        return Some(Reply::failed(cx.msg("too_long", &[])));
    }
    let now = cx.now_ms();
    let title = if name.is_empty() { cx.msg("default_name", &[]) } else { name.clone() };
    let mut entry = Entry::new(cx.new_id(), EntryKind::Timer, title, now, &cx.host.zone());
    entry.due_at = Some(now + secs * 1000);
    entry.duration_ms = secs * 1000;
    cx.store.put(entry.clone());
    let _ = cx.host.schedule(&entry);
    let duration = describe_duration(secs, cx.lang);
    let text = if name.is_empty() { cx.msg("set", &[("duration", &duration)]) } else { cx.msg("set_named", &[("name", &name), ("duration", &duration)]) };
    Some(Reply::done(text).with_card(Card::Timer { entry }))
}

/// What is left of the sentence once the duration and the skill's own words are gone: the timer's name.
fn name_in(cx: &Cx, used: &[bool]) -> String {
    let (noise, suffixes) = (cx.list("noise"), cx.list("name_suffix"));
    let mut parts = Vec::new();
    for (i, token) in cx.utt.tokens.iter().enumerate() {
        if used[i] || cx.lang.number(&token.norm).is_some() {
            continue;
        }
        // "Nudeltimer" names the timer in one word
        if let Some(stem) = suffixes.iter().find_map(|s| token.norm.strip_suffix(s.as_str()).filter(|stem| stem.chars().count() >= 2)) {
            let stem = stem.trim_end_matches('-');
            parts.push(token.orig.chars().take(stem.chars().count()).collect::<String>());
            continue;
        }
        if !noise.contains(&token.norm) {
            parts.push(token.orig.clone());
        }
    }
    iris_core::text::capitalise(&parts.join(" "))
}

fn label(cx: &Cx, entry: &Entry) -> Option<String> {
    (entry.title != cx.msg("default_name", &[])).then(|| entry.title.clone())
}

fn query(cx: &mut Cx) -> Reply {
    let timers = running(cx);
    if timers.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let now = cx.now_ms();
    let lines: Vec<String> = timers
        .iter()
        .map(|t| {
            let left = describe_duration((t.left_ms(now) + 999) / 1000, cx.lang);
            match label(cx, t) {
                Some(name) if timers.len() > 1 || !name.is_empty() => cx.msg("left_named", &[("name", &name), ("left", &left)]),
                _ => cx.msg("left", &[("left", &left)]),
            }
        })
        .collect();
    Reply::done(lines.join(" ")).with_card(Card::Entries { title: cx.msg("default_name", &[]), items: timers })
}

fn cancel(cx: &mut Cx, answering: bool) -> Reply {
    let timers = running(cx);
    if timers.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let said = cx.utt.norm.clone();
    let named: Vec<&Entry> = timers.iter().filter(|t| label(cx, t).is_some_and(|n| said.contains(&n.to_lowercase()))).collect();
    let all = cx.utt.tokens.iter().any(|t| cx.list("all").contains(&t.norm));
    let targets: Vec<&Entry> = if !named.is_empty() {
        named
    } else if all || timers.len() == 1 {
        timers.iter().collect()
    } else if answering {
        return Reply::failed(cx.msg("not_found", &[]));
    } else {
        let names = timers.iter().map(|t| t.title.clone()).collect::<Vec<_>>().join(", ");
        cx.ask("cancel.which", json!({}));
        return Reply::question(cx.msg("which", &[("names", &names)]));
    };
    for t in &targets {
        cx.store.delete(&t.id);
        cx.host.unschedule(&t.id);
    }
    match targets.as_slice() {
        [one] => match label(cx, one) {
            Some(name) => Reply::done(cx.msg("cancelled_named", &[("name", &name)])),
            None => Reply::done(cx.msg("cancelled", &[])),
        },
        many => Reply::done(cx.messages.plural("cancelled_all", many.len() as i64, &[])),
    }
}
