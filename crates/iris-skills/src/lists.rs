//! Lists: add (in one sentence, as "and also …", or guided), read, tick off, remove, empty.
//!
//! A list is named by what is on it: "Einkaufsliste", "Einkaufszettel" and "Liste Einkauf" are
//! the list "Einkauf". The list talked about last is remembered for ten minutes, so "und noch
//! Käse" and "lösche Milch" need no name.

use crate::{join, strip};
use iris_core::actions::Args;
use iris_core::text::capitalise;
use iris_core::{Card, Cx, Entry, EntryKind, Reply, Skill};
use serde_json::json;

pub struct Lists;

const RECENT_MS: i64 = 600_000;

impl Skill for Lists {
    fn manifest(&self) -> &'static str {
        include_str!("../data/lists/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/lists/de.toml")),
            "en" => Some(include_str!("../data/lists/en.toml")),
            _ => None,
        }
    }

    fn act(&self, action: &str, args: &Args, cx: &mut Cx) -> Option<Reply> {
        let list = args.text("list").map(|l| canonical(cx, &l.to_lowercase()));
        Some(match action {
            "list.add" => add_items(cx, Some(list?), false, args.texts("items")),
            "list.check_all" => check_all(cx, &list?),
            "list.check_common" => check_common(cx, &list?, &canonical(cx, &args.text("other")?.to_lowercase())),
            "list.show" => show(cx, list.or_else(|| cx.recall("list", RECENT_MS)).or_else(|| collections(cx).into_iter().next())),
            "list.check" => mark_item(cx, list, args.text("item")?, true),
            "list.remove" => mark_item(cx, list, args.text("item")?, false),
            _ => return None,
        })
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        let state_list = cx.state["list"].as_str().map(str::to_string);
        match intent {
            "add" | "put" => Some(add(cx, None, false)),
            // "Ich muss Milch kaufen": the shopping list, without asking
            "buy" => {
                let list = cx.table("defaults", "list").map(str::to_string);
                Some(add(cx, list, false))
            }
            "more" => {
                // "und erstelle einen Wecker …" is a new command, not one more entry
                let rest = cx.span("rest");
                if rest.len() > 9 || cx.utt.tokens.get(rest.start).is_some_and(|t| cx.list("sentence_start").contains(&t.norm)) {
                    return None;
                }
                let list = cx.recall("list", RECENT_MS)?;
                Some(add(cx, Some(list), false))
            }
            "guided" => {
                cx.ask("add.list", json!({}));
                Some(Reply::question(cx.msg("ask_list", &[])))
            }
            "add.list" => {
                let list = named(cx).or_else(|| cx.utt.tokens.last().map(|t| canonical(cx, &t.norm)))?;
                // the entries said before the question are put on the list that was just named
                let kept: Vec<String> = cx.state["items"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
                if !kept.is_empty() {
                    return Some(add_items(cx, Some(list), false, kept));
                }
                cx.ask("add.items", json!({ "list": list }));
                Some(Reply::question(cx.msg("ask_items", &[("list", &list)])))
            }
            "add.items" | "add.more" => {
                if looks_like_a_new_sentence(cx) {
                    return None;
                }
                if cx.lang.dialog.is_done(&cx.utt.norm) {
                    return Some(Reply::done(cx.msg("finished", &[])));
                }
                Some(add(cx, state_list, true))
            }
            "read" | "peek" => Some(read(cx)),
            "read_all" => Some(read_all(cx)),
            "check" => mark(cx, true),
            "remove" => mark(cx, false),
            "check_all" => everything(cx, true),
            "clear" => everything(cx, false),
            "clear.confirm" => Some(clear_confirmed(cx)),
            _ => None,
        }
    }
}

fn open(cx: &Cx) -> Vec<Entry> {
    cx.store.all(Some(EntryKind::ListItem)).into_iter().filter(|e| e.pending).collect()
}

/// Lists that have entries, the fullest first.
fn collections(cx: &Cx) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for e in cx.store.all(Some(EntryKind::ListItem)) {
        match counts.iter_mut().find(|(name, _)| *name == e.collection) {
            Some(slot) => slot.1 += 1,
            None => counts.push((e.collection, 1)),
        }
    }
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    counts.into_iter().map(|(name, _)| name).collect()
}

fn key(name: &str) -> String {
    name.to_lowercase().trim().trim_end_matches('s').to_string()
}

/// The stored name for what was said: an alias, a list that exists already, or the word itself.
fn canonical(cx: &Cx, stem: &str) -> String {
    if let Some(alias) = cx.table("list_alias", stem) {
        return alias.to_string();
    }
    collections(cx).into_iter().find(|c| key(c) == key(stem)).unwrap_or_else(|| capitalise(stem))
}

/// The list the sentence names, if it names one.
fn named(cx: &Cx) -> Option<String> {
    let re = cx.expr("list_name")?;
    re.captures_iter(&cx.utt.norm).filter_map(|c| c.name("a").or_else(|| c.name("b")).map(|m| m.as_str().to_string())).find(|stem| !cx.list("list_stop").contains(stem)).map(|stem| canonical(cx, &stem))
}

/// The list meant when none is named: the recent one, the only one, or for a first entry the default.
fn implied(cx: &Cx) -> Option<String> {
    if let Some(recent) = cx.recall("list", RECENT_MS) {
        return Some(recent);
    }
    let all = collections(cx);
    // exactly one list: that one. Otherwise the user is asked ("Auf welche Liste?").
    match all.len() {
        1 => all.into_iter().next(),
        _ => None,
    }
}

fn in_list_phrase(cx: &Cx) -> Vec<bool> {
    let mut hit = vec![false; cx.utt.len()];
    if let Some(re) = cx.expr("list_phrase") {
        for m in re.find_iter(&cx.utt.norm) {
            for i in cx.utt.token_range(m.range()) {
                hit[i] = true;
            }
        }
    }
    hit
}

/// The entries a sentence names: split at "und" and commas, without the list's name and the verbs around it.
fn said_items(cx: &Cx) -> Vec<String> {
    let skip = in_list_phrase(cx);
    let separators = cx.list("separators");
    let mut groups: Vec<Vec<usize>> = vec![Vec::new()];
    for i in cx.span("rest") {
        if skip[i] {
            continue;
        }
        let token = &cx.utt.tokens[i];
        if separators.contains(&token.norm) {
            groups.push(Vec::new());
            continue;
        }
        groups.last_mut().unwrap().push(i);
        if token.sep {
            groups.push(Vec::new());
        }
    }
    // the target of the entry without the word "list": only when the sentence names no list
    if named(cx).is_none() {
        if let Some(re) = cx.expr("target") {
            if let Some(m) = re.find(&cx.utt.norm) {
                let cut = cx.utt.token_range(m.range());
                for g in groups.iter_mut() {
                    g.retain(|i| !cut.contains(i));
                }
            }
        }
    }
    groups
        .into_iter()
        .map(|g| strip(cx.utt, g, cx.list("drop"), cx.list("lead"), cx.list("trail")))
        .filter(|g| !g.is_empty() && !cx.list("placeholder").contains(&g.iter().map(|i| cx.utt.tokens[*i].norm.as_str()).collect::<Vec<_>>().join(" ")))
        .map(|g| cx.rewrite(&join(cx.utt, &g)))
        .collect()
}

fn spoken(cx: &Cx, titles: &[String]) -> String {
    match titles {
        [] => String::new(),
        [one] => one.clone(),
        [head @ .., last] => format!("{} {} {last}", head.join(", "), cx.lang.format.list_and),
    }
}

fn looks_like_a_new_sentence(cx: &Cx) -> bool {
    cx.utt.len() > 9 || cx.utt.tokens.first().is_some_and(|t| cx.list("sentence_start").contains(&t.norm))
}

fn add(cx: &mut Cx, list: Option<String>, guided: bool) -> Reply {
    let items = said_items(cx);
    add_items(cx, list, guided, items)
}

/// The entries are known (said in this sentence, or kept from the one that asked "which list?").
fn add_items(cx: &mut Cx, list: Option<String>, guided: bool, items: Vec<String>) -> Reply {
    if guided && items.is_empty() && cx.lang.dialog.is_yes(&cx.utt.norm) {
        let list = list.unwrap_or_default();
        cx.ask("add.items", json!({ "list": list }));
        return Reply::question(cx.msg("ask_items", &[("list", &list)]));
    }
    let Some(list) = list.or_else(|| named(cx)).or_else(|| implied(cx)) else {
        cx.ask("add.list", json!({ "items": items }));
        return Reply::question(cx.msg("ask_list", &[]));
    };
    if items.is_empty() {
        cx.ask("add.items", json!({ "list": list }));
        return Reply::question(cx.msg("ask_items", &[("list", &list)]));
    }
    let existing = open(cx);
    let (mut added, mut known) = (Vec::new(), Vec::new());
    for title in items {
        if existing.iter().any(|e| e.collection == list && e.title.to_lowercase() == title.to_lowercase()) || added.contains(&title) {
            known.push(title);
            continue;
        }
        let now = cx.now_ms();
        let mut entry = Entry::new(cx.new_id(), EntryKind::ListItem, title.clone(), now, &cx.host.zone());
        entry.collection = list.clone();
        cx.store.put(entry);
        added.push(title);
    }
    cx.remember("list", &list);
    let mut text = if added.is_empty() {
        cx.msg("already", &[("items", &spoken(cx, &known)), ("list", &list)])
    } else {
        cx.messages.plural("added", added.len() as i64, &[("items", &spoken(cx, &added)), ("list", &list)])
    };
    if guided {
        cx.ask("add.more", json!({ "list": list }));
        text = format!("{text} {}", cx.msg("ask_more", &[]));
    }
    let items = open(cx).into_iter().filter(|e| e.collection == list).collect();
    Reply::done(text).with_card(Card::Entries { title: list, items })
}

fn read(cx: &mut Cx) -> Reply {
    let list = named(cx).or_else(|| cx.recall("list", RECENT_MS)).or_else(|| collections(cx).into_iter().next());
    show(cx, list)
}

/// Says what is open on `list` (none: there are no lists).
fn show(cx: &mut Cx, list: Option<String>) -> Reply {
    let Some(list) = list else {
        return Reply::done(cx.msg("no_lists", &[]));
    };
    cx.remember("list", &list);
    let items: Vec<Entry> = open(cx).into_iter().filter(|e| e.collection == list).collect();
    if items.is_empty() {
        return Reply::done(cx.msg("empty", &[("list", &list)]));
    }
    let titles: Vec<String> = items.iter().map(|e| e.title.clone()).collect();
    Reply::done(cx.messages.plural("read", items.len() as i64, &[("list", &list), ("items", &spoken(cx, &titles))])).with_card(Card::Entries { title: list, items })
}

fn refers_to_a_list(cx: &Cx) -> bool {
    named(cx).is_some() || cx.utt.tokens.iter().any(|t| cx.list("entry_words").contains(&t.norm))
}

/// Ticks off (`done`) or removes the entries the sentence names. Not ours when it names none and no list either.
fn mark(cx: &mut Cx, done: bool) -> Option<Reply> {
    let list = named(cx);
    let said = format!(" {} ", cx.utt.norm);
    let mut hits: Vec<Entry> = open(cx).into_iter().filter(|e| list.as_ref().is_none_or(|l| *l == e.collection)).filter(|e| said.contains(&format!(" {} ", e.title.to_lowercase()))).collect();
    if hits.is_empty() {
        // homework comes from the school's timetable app: it is not Iris' to tick off
        if cx.utt.tokens.iter().any(|t| cx.list("school").contains(&t.norm)) {
            return Some(Reply::failed(cx.msg("school_items", &[])));
        }
        return refers_to_a_list(cx).then(|| Reply::failed(cx.msg("not_found", &[])));
    }
    // the same name on two lists: the recent list decides
    if let Some(recent) = cx.recall("list", RECENT_MS).filter(|r| hits.iter().any(|e| e.collection == *r)) {
        hits.retain(|e| e.collection == recent);
    }
    for entry in &hits {
        if done {
            cx.store.put(Entry { pending: false, ..entry.clone() });
        } else {
            cx.store.delete(&entry.id);
        }
    }
    cx.remember("list", &hits[0].collection.clone());
    let titles: Vec<String> = hits.iter().map(|e| e.title.clone()).collect();
    Some(Reply::done(cx.messages.plural(if done { "checked" } else { "removed" }, titles.len() as i64, &[("items", &spoken(cx, &titles))])))
}

/// Ticks off everything open on `list`.
fn check_all(cx: &mut Cx, list: &str) -> Reply {
    let items: Vec<Entry> = open(cx).into_iter().filter(|e| e.collection == list).collect();
    if items.is_empty() {
        return Reply::done(cx.msg("empty", &[("list", list)]));
    }
    for e in &items {
        cx.store.put(Entry { pending: false, ..e.clone() });
    }
    cx.remember("list", list);
    Reply::done(cx.msg("all_checked", &[("list", list)]))
}

/// Ticks off on `list` what is also open on `other`: decided here on the phone, so the assistant never needs to see either list.
fn check_common(cx: &mut Cx, list: &str, other: &str) -> Reply {
    let all = open(cx);
    let on_other: Vec<String> = all.iter().filter(|e| e.collection == other).map(|e| e.title.to_lowercase()).collect();
    let hits: Vec<Entry> = all.into_iter().filter(|e| e.collection == list && on_other.contains(&e.title.to_lowercase())).collect();
    if hits.is_empty() {
        return Reply::done(cx.msg("nothing_in_common", &[("list", list), ("other", other)]));
    }
    for e in &hits {
        cx.store.put(Entry { pending: false, ..e.clone() });
    }
    cx.remember("list", list);
    let titles: Vec<String> = hits.iter().map(|e| e.title.clone()).collect();
    Reply::done(cx.messages.plural("checked", titles.len() as i64, &[("items", &spoken(cx, &titles))]))
}

/// Ticks off (`done`) or removes the one open entry named `item` (on `list` when given). Several that fit: nothing happens.
fn mark_item(cx: &mut Cx, list: Option<String>, item: &str, done: bool) -> Reply {
    let items: Vec<Entry> = open(cx).into_iter().filter(|e| list.as_ref().is_none_or(|l| *l == e.collection)).collect();
    let entry = match crate::by_title(&items, item) {
        Ok(entry) => entry.clone(),
        Err(crate::Miss::None) => return Reply::failed(cx.msg("not_found", &[])),
        Err(crate::Miss::Several) => return Reply::failed(cx.lang.messages.get("action_unclear", &[])),
    };
    if done {
        cx.store.put(Entry { pending: false, ..entry.clone() });
    } else {
        cx.store.delete(&entry.id);
    }
    cx.remember("list", &entry.collection);
    Reply::done(cx.messages.plural(if done { "checked" } else { "removed" }, 1, &[("items", &entry.title)]))
}

fn today_only(cx: &Cx) -> bool {
    let said = format!(" {} ", cx.utt.norm);
    cx.list("today").iter().any(|p| said.contains(&format!(" {p} ")))
}

fn made_today(cx: &Cx, entry: &Entry) -> bool {
    jiff::Timestamp::from_millisecond(entry.created_at).is_ok_and(|t| t.to_zoned(cx.now.time_zone().clone()).date() == cx.now.date())
}

/// All entries of a list, or all of today's: tick them off, or (after asking) delete them.
fn everything(cx: &mut Cx, tick: bool) -> Option<Reply> {
    let today = today_only(cx);
    // "Hake alles ab": the list talked about last, the only list, or else every list
    let list = if today { None } else { named(cx).or_else(|| cx.recall("list", RECENT_MS)).or_else(|| implied(cx)) };
    if !today && list.is_none() && !tick && !refers_to_a_list(cx) {
        return None;
    }
    let items: Vec<Entry> = open(cx).into_iter().filter(|e| if today || list.is_none() { today && made_today(cx, e) || (!today && list.is_none()) } else { Some(&e.collection) == list.as_ref() }).collect();
    let name = list.clone().unwrap_or_default();
    if items.is_empty() {
        return Some(Reply::done(if list.is_some() { cx.msg("empty", &[("list", &name)]) } else { cx.msg("no_lists", &[]) }));
    }
    if tick {
        for e in &items {
            cx.store.put(Entry { pending: false, ..e.clone() });
        }
        return Some(Reply::done(if today { cx.msg("all_checked_today", &[]) } else { cx.msg("all_checked", &[("list", &name)]) }));
    }
    cx.ask("clear.confirm", json!({ "list": list, "today": today }));
    let key = if today { "confirm_clear_today" } else { "confirm_clear" };
    Some(Reply::question(cx.messages.plural(key, items.len() as i64, &[("list", &name)])))
}

fn clear_confirmed(cx: &mut Cx) -> Reply {
    if !cx.lang.dialog.is_yes(&cx.utt.norm) {
        return Reply::done(cx.msg("kept", &[]));
    }
    let today = cx.state["today"].as_bool().unwrap_or(false);
    let list = cx.state["list"].as_str().unwrap_or_default().to_string();
    for e in cx.store.all(Some(EntryKind::ListItem)) {
        if (today && made_today(cx, &e)) || (!today && e.collection == list) {
            cx.store.delete(&e.id);
        }
    }
    Reply::done(if today { cx.msg("cleared_today", &[]) } else { cx.msg("cleared", &[("list", &list)]) })
}

/// Every list with what is open on it.
fn read_all(cx: &mut Cx) -> Reply {
    let lists = collections(cx);
    let items = open(cx);
    if items.is_empty() {
        return Reply::done(cx.msg("no_lists", &[]));
    }
    let mut lines = Vec::new();
    for list in lists {
        let titles: Vec<String> = items.iter().filter(|e| e.collection == list).map(|e| e.title.clone()).collect();
        if !titles.is_empty() {
            lines.push(cx.msg("read_all_line", &[("list", &list), ("items", &spoken(cx, &titles))]));
        }
    }
    Reply::done(lines.join(" ")).with_card(Card::Entries { title: cx.msg("card_title", &[]), items })
}
