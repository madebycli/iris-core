//! Timers: set, ask how long is left, cancel. Several can run at once, each with a name.

use iris_core::time::{describe, describe_duration, resolve, scan, scan_duration, HalfDay};
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
            "set.name" => {
                let secs = cx.state["secs"].as_i64().unwrap_or(0);
                let said = cx.utt.norm.clone();
                let name = if cx.list("no_name").contains(&said) { String::new() } else { name_in(cx, &vec![false; cx.utt.len()]) };
                Some(start(cx, secs, name, None))
            }
            "query" => Some(query(cx)),
            "pause" | "resume" | "extend" | "restart" => Some(control(cx, intent, false)),
            "control.which" => {
                let action = cx.state["action"].as_str().unwrap_or("pause").to_string();
                Some(control(cx, &action, true))
            }
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
    let mut until = None;
    if found.is_none() {
        // "für 16:30 Uhr morgen": a clock time makes a timer that runs until then
        let times = scan(&norms, cx.lang);
        if times.expr.clock.is_some() || times.expr.day.is_some() {
            if let Ok(at) = resolve(&times.expr, &cx.now, HalfDay::Nearest) {
                let secs = (at.at.as_second() - cx.now.timestamp().as_second()).max(1);
                found = Some((secs, times.used));
                until = Some(at.at);
            }
        }
    }
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
    // asked for step by step ("Erstelle einen Timer" … "zehn Minuten"): the name is asked too
    if answering.as_ref().is_some_and(|n| n.is_empty()) && until.is_none() {
        cx.ask("set.name", json!({ "secs": secs }));
        return Some(Reply::question(cx.msg("ask_name", &[])));
    }
    Some(start(cx, secs, name, until))
}

/// Starts the timer and says so.
fn start(cx: &mut Cx, secs: i64, name: String, until: Option<jiff::Timestamp>) -> Reply {
    let now = cx.now_ms();
    let title = if name.is_empty() { cx.msg("default_name", &[]) } else { name.clone() };
    let mut entry = Entry::new(cx.new_id(), EntryKind::Timer, title, now, &cx.host.zone());
    entry.due_at = Some(now + secs * 1000);
    entry.duration_ms = secs * 1000;
    cx.store.put(entry.clone());
    let _ = cx.host.schedule(&entry);
    let duration = match until {
        Some(at) => describe(at, &cx.now, cx.lang, false),
        None => describe_duration(secs, cx.lang),
    };
    let key = match (until.is_some(), name.is_empty()) {
        (false, true) => "set",
        (false, false) => "set_named",
        (true, true) => "set_until",
        (true, false) => "set_until_named",
    };
    let text = cx.msg(key, &[("name", &name), ("duration", &duration)]);
    Reply::done(text).with_card(Card::Timer { entry })
}

/// What is left of the sentence once the duration and the skill's own words are gone: the timer's name.
fn name_in(cx: &Cx, used: &[bool]) -> String {
    let (noise, suffixes) = (cx.list("noise"), cx.list("name_suffix"));
    let mut parts = Vec::new();
    for (i, token) in cx.utt.tokens.iter().enumerate() {
        if used[i] || cx.lang.number(&token.norm).is_some() || token.norm.chars().any(|c| c.is_ascii_digit()) {
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

/// Whether the sentence names this timer: its name, or a word like "Nudeltimer" whose first part begins its name.
fn names(cx: &Cx, entry: &Entry, said: &str) -> bool {
    let Some(name) = label(cx, entry).map(|n| n.to_lowercase()) else { return false };
    said.contains(&name)
        || said.split(' ').any(|w| cx.list("name_suffix").iter().any(|s| w.strip_suffix(s.as_str()).is_some_and(|stem| stem.chars().count() >= 3 && name.starts_with(stem.trim_end_matches('-')))))
}

fn label(cx: &Cx, entry: &Entry) -> Option<String> {
    (entry.title != cx.msg("default_name", &[])).then(|| entry.title.clone())
}

fn query(cx: &mut Cx) -> Reply {
    let mut timers = running(cx);
    if timers.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    // "Wie lange läuft der Nudeltimer noch?": only that one
    let said = cx.utt.norm.clone();
    let named: Vec<Entry> = timers.iter().filter(|t| names(cx, t, &said)).cloned().collect();
    if !named.is_empty() {
        timers = named;
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
    let named: Vec<&Entry> = timers.iter().filter(|t| names(cx, t, &said)).collect();
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
        cx.host.unschedule(&t.id);
        cx.store.delete(&t.id);
    }
    match targets.as_slice() {
        [one] => match label(cx, one) {
            Some(name) => Reply::done(cx.msg("cancelled_named", &[("name", &name)])),
            None => Reply::done(cx.msg("cancelled", &[])),
        },
        many => Reply::done(cx.messages.plural("cancelled_all", many.len() as i64, &[])),
    }
}

/// Pause, resume, extend or restart the timer the sentence names, or the only one; asks which when there are several.
fn control(cx: &mut Cx, action: &str, answering: bool) -> Reply {
    let timers = running(cx);
    if timers.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let said = cx.utt.norm.clone();
    let named: Vec<&Entry> = timers.iter().filter(|t| names(cx, t, &said)).collect();
    let target = match (named.as_slice(), timers.len()) {
        ([one], _) => (*one).clone(),
        ([], 1) => timers[0].clone(),
        _ if answering => return Reply::failed(cx.msg("not_found", &[])),
        _ => {
            let names = timers.iter().map(|t| t.title.clone()).collect::<Vec<_>>().join(", ");
            cx.ask("control.which", json!({ "action": action }));
            return Reply::question(cx.msg("which_control", &[("names", &names)]));
        }
    };
    let now = cx.now_ms();
    let mut entry = target;
    let paused = entry.remaining_ms > 0;
    let key = match action {
        "pause" if paused => "already_paused",
        "pause" => {
            entry.remaining_ms = entry.left_ms(now).max(1);
            "paused"
        }
        "resume" if !paused => "not_paused",
        "resume" => {
            entry.due_at = Some(now + entry.remaining_ms);
            entry.remaining_ms = 0;
            "resumed"
        }
        "extend" => {
            let Some((secs, _)) = scan_duration(&cx.utt.norms(), cx.lang) else {
                return Reply::failed(cx.msg("ask_more", &[]));
            };
            if paused {
                entry.remaining_ms += secs * 1000;
            } else {
                entry.due_at = Some(entry.due_at.unwrap_or(now) + secs * 1000);
            }
            "extended"
        }
        _ => {
            entry.due_at = Some(now + entry.duration_ms.max(1000));
            entry.remaining_ms = 0;
            "restarted"
        }
    };
    cx.store.put(entry.clone());
    // the host plans a running timer's alarm and shows a paused one as paused (remaining_ms > 0)
    if !matches!(key, "already_paused" | "not_paused") {
        cx.host.unschedule(&entry.id);
        let _ = cx.host.schedule(&entry);
    }
    let more = scan_duration(&cx.utt.norms(), cx.lang).map(|(s, _)| describe_duration(s, cx.lang)).unwrap_or_default();
    let left = describe_duration((entry.left_ms(now) + 999) / 1000, cx.lang);
    let duration = describe_duration(entry.duration_ms / 1000, cx.lang);
    let name = entry.title.clone();
    Reply::done(cx.msg(key, &[("name", &name), ("left", &left), ("more", &more), ("duration", &duration)])).with_card(Card::Timer { entry })
}
