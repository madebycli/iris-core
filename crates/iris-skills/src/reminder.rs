//! Reminders: create (in one sentence or guided, time first), list, delete.
//!
//! Where a reminder is kept is the user's choice (`reminder.destination`): in Iris itself, in
//! the task app, in the calendar, or in both of those. Iris' own reminders ring through the
//! host's scheduler; the others ring in the app they were given to. If that app cannot be
//! reached the reminder is kept in Iris and the reply says so — a reminder is never dropped.

use crate::{join, strip};
use iris_core::actions::Args;
use iris_core::time::{describe, resolve, scan, HalfDay, Resolved, TimeError, TimeExpr};
use iris_core::{Card, Cx, Entry, EntryKind, ExternalEvent, ExternalTask, Reply, Skill};
use serde_json::json;

pub struct Reminder;

impl Skill for Reminder {
    fn manifest(&self) -> &'static str {
        include_str!("../data/reminder/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/reminder/de.toml")),
            "en" => Some(include_str!("../data/reminder/en.toml")),
            _ => None,
        }
    }

    fn act(&self, action: &str, args: &Args, cx: &mut Cx) -> Option<Reply> {
        let title = args.text("title").map(str::to_string);
        let at = args.at_ms().and_then(|ms| jiff::Timestamp::from_millisecond(ms).ok());
        Some(match action {
            "reminder.create" => {
                let draft = Draft { title: title?, ..Draft::default() };
                let repeat = match args.text("repeat") {
                    Some("daily") => Some(iris_core::time::Repeat::Daily),
                    Some("weekdays") => Some(iris_core::time::Repeat::Weekdays),
                    Some("weekly") => Some(iris_core::time::Repeat::Weekly),
                    _ => None,
                };
                save(cx, &draft, &Resolved { at: at?, date_only: false, assumed_half_day: false, repeat })
            }
            "reminder.show" => list(cx),
            "reminder.snooze" => match snooze_by(cx, args.int("seconds").unwrap_or(600)) {
                Some(reply) => reply,
                None => Reply::failed(cx.msg("not_found", &[])),
            },
            "reminder.delete" => match picked(cx, &title?) {
                Ok(entry) => {
                    cx.host.unschedule(&entry.id);
                    cx.store.delete(&entry.id);
                    Reply::done(cx.msg("deleted", &[("title", &entry.title)]))
                }
                Err(reply) => *reply,
            },
            "reminder.move" => match picked(cx, &title?) {
                Ok(entry) => relocate(cx, entry, at?),
                Err(reply) => *reply,
            },
            "task.create" => {
                let date = args.text("date").and_then(|d| d.parse::<jiff::civil::Date>().ok());
                let due = date.and_then(|d| d.at(9, 0, 0, 0).to_zoned(cx.now.time_zone().clone()).ok()).map(|z| z.timestamp());
                let task = ExternalTask { title: title.clone()?, due_at: due.map(|t| t.as_millisecond()), date_only: date.is_some(), list_id: None, repeat: String::new() };
                match cx.host.create_task(&task) {
                    Ok(list) => {
                        let when = due.map(|t| describe(t, &cx.now, cx.lang, true)).unwrap_or_default();
                        Reply::done(cx.msg(if due.is_some() { "created_tasks" } else { "created_tasks_undated" }, &[("title", &title?), ("when", &when), ("list", &list)]))
                    }
                    Err(_) => Reply::failed(cx.msg("tasks_unreachable", &[])),
                }
            }
            "calendar.event" => {
                let minutes = args.int("minutes").unwrap_or(30);
                let when = describe(at?, &cx.now, cx.lang, false);
                // the calendar is written only after the user said yes: the confirmation of the calendar flow still applies
                cx.ask("event.confirm", json!({ "title": title.clone()?, "at_ms": args.at_ms()?, "minutes": minutes }));
                Reply::question(cx.msg("ask_event", &[("title", &title?), ("when", &when)]))
            }
            _ => return None,
        })
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        match intent {
            "create" => {
                // "do not forget …" is only a reminder when it names a time; "… put milk on the list" is not
                let weak = cx.list("weak").iter().any(|w| cx.utt.norm.starts_with(w.as_str()));
                if weak && scan(&cx.utt.norms(), cx.lang).expr.is_empty() {
                    return None;
                }
                Some(create(cx, Draft::default()))
            }
            "create.time" | "create.title" => {
                let draft: Draft = serde_json::from_value(cx.state.clone()).unwrap_or_default();
                Some(create(cx, draft))
            }
            "move" => Some(shift(cx, None, TimeExpr::default(), false)),
            "move.which" => {
                let expr: TimeExpr = serde_json::from_value(cx.state["time"].clone()).unwrap_or_default();
                Some(shift(cx, None, expr, true))
            }
            "move.time" => {
                let id = cx.state["id"].as_str().map(str::to_string);
                Some(shift(cx, id, TimeExpr::default(), true))
            }
            "snooze" => snooze(cx),
            "today" => Some(today(cx)),
            "list" => Some(list(cx)),
            "delete" => Some(delete(cx, false)),
            "delete.which" => Some(delete(cx, true)),
            "delete.confirm" => Some(delete_all(cx)),
            "event.confirm" if cx.lang.dialog.is_no(&cx.utt.norm) => Some(Reply::done(cx.lang.messages.get("cancelled", &[]))),
            "event.confirm" if !cx.lang.dialog.is_yes(&cx.utt.norm) => None,
            "event.confirm" => Some(write_event(cx)),
            _ => None,
        }
    }
}

/// A reminder being put together over several turns.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Draft {
    title: String,
    time: TimeExpr,
    list_hint: Option<String>,
    /// Which answer is awaited: "time" or "title". Empty on the first turn.
    awaiting: String,
}

fn create(cx: &mut Cx, mut draft: Draft) -> Reply {
    let norms = cx.utt.norms();
    let found = scan(&norms, cx.lang);
    let mut free: Vec<bool> = found.used.iter().map(|u| !u).collect();
    if let Some(m) = cx.expr("list_hint").and_then(|re| re.captures(&cx.utt.norm)) {
        draft.list_hint = m.name("a").or_else(|| m.name("b")).map(|x| x.as_str().to_string());
        for i in cx.utt.token_range(m.get(0).map(|x| x.range()).unwrap_or(0..0)) {
            free[i] = false;
        }
    }
    draft.time.merge(found.expr);
    let title_before = draft.title.clone();
    if draft.awaiting != "time" || draft.title.is_empty() {
        let tokens: Vec<usize> = cx.span("rest").filter(|i| free[*i]).collect();
        let said = cx.rewrite(&join(cx.utt, &strip(cx.utt, tokens, cx.list("drop"), cx.list("lead"), cx.list("trail"))));
        if !said.is_empty() {
            draft.title = said;
        }
    }
    // time first, then the name
    let resolved = match resolve(&draft.time, &cx.now, HalfDay::Nearest) {
        Ok(r) => r,
        Err(error) => {
            let key = match (error, draft.awaiting.as_str(), draft.title.is_empty()) {
                (TimeError::Past, _, _) => "time_past",
                (TimeError::Missing, "time", false) if title_before.is_empty() => "ask_time_for",
                (TimeError::Invalid, _, _) | (TimeError::Missing, "time", _) => "time_unclear",
                (TimeError::Missing, _, true) => "ask_time",
                (TimeError::Missing, _, false) => "ask_time_for",
            };
            let text = cx.msg(key, &[("title", &draft.title)]);
            draft.time = TimeExpr::default();
            draft.awaiting = "time".into();
            cx.ask("create.time", json!(draft));
            return Reply::question(text);
        }
    };
    if draft.title.is_empty() {
        draft.awaiting = "title".into();
        cx.ask("create.title", json!(draft));
        return Reply::question(cx.msg("ask_title", &[]));
    }
    save(cx, &draft, &resolved)
}

fn save(cx: &mut Cx, draft: &Draft, at: &Resolved) -> Reply {
    let due = at.at.as_millisecond();
    let when = describe(at.at, &cx.now, cx.lang, at.date_only);
    let repeat = at.repeat.as_ref().map(|r| r.code()).unwrap_or_default();
    let title = draft.title.as_str();
    let destination = cx.host.setting("reminder.destination").unwrap_or_else(|| "local".into());
    let to_tasks = |cx: &Cx| {
        let lists = cx.host.task_lists().unwrap_or_default();
        let list_id = draft.list_hint.as_ref().and_then(|hint| lists.iter().find(|l| l.name.to_lowercase().contains(hint.as_str()) || hint.contains(&l.name.to_lowercase()))).map(|l| l.id.clone());
        cx.host.create_task(&ExternalTask { title: title.into(), due_at: Some(due), date_only: at.date_only, list_id, repeat: repeat.clone() })
    };
    let to_calendar = |cx: &Cx| cx.host.create_event(&ExternalEvent { title: title.into(), start_at: due, minutes: 30, repeat: repeat.clone() });
    let outside = match destination.as_str() {
        "tasks" => Some(to_tasks(cx).map(|list| cx.msg("created_tasks", &[("title", title), ("when", &when), ("list", &list)]))),
        "calendar" => Some(to_calendar(cx).map(|cal| cx.msg("created_calendar", &[("title", title), ("when", &when), ("calendar", &cal)]))),
        "both" => Some(to_tasks(cx).and_then(|list| to_calendar(cx).map(|cal| cx.msg("created_both", &[("title", title), ("when", &when), ("list", &list), ("calendar", &cal)])))),
        _ => None,
    };
    if let Some(Ok(text)) = outside {
        return Reply::done(text);
    }
    let now = cx.now_ms();
    let mut entry = Entry::new(cx.new_id(), EntryKind::Reminder, title, now, &cx.host.zone());
    entry.due_at = Some(due);
    entry.repeat = repeat.clone();
    cx.store.put(entry.clone());
    cx.remember("last", &entry.id);
    let _ = cx.host.schedule(&entry);
    let text = if outside.is_some() {
        cx.msg("fallback_local", &[("title", title), ("when", &when)])
    } else if repeat.is_empty() {
        cx.msg("created", &[("title", title), ("when", &when)])
    } else {
        let every = cx.lang.messages.get(&format!("repeat.{}", if repeat.starts_with("days:") { "weekly" } else { repeat.as_str() }), &[]);
        cx.msg("created_repeat", &[("title", title), ("when", &when), ("repeat", &every)])
    };
    Reply::done(text).with_card(Card::Entries { title: cx.msg("card_title", &[]), items: vec![entry] })
}

fn open(cx: &Cx) -> Vec<Entry> {
    let mut all: Vec<Entry> = cx.store.all(Some(EntryKind::Reminder)).into_iter().filter(|e| e.pending).collect();
    all.sort_by_key(|e| e.due_at.unwrap_or(i64::MAX));
    all
}

fn spoken(cx: &Cx, entry: &Entry) -> String {
    let when = entry.due_at.and_then(|d| jiff::Timestamp::from_millisecond(d).ok()).map(|t| describe(t, &cx.now, cx.lang, false)).unwrap_or_default();
    cx.msg("item", &[("title", &entry.title), ("when", &when)])
}

fn list(cx: &mut Cx) -> Reply {
    let mut items = open(cx);
    let norms = cx.utt.norms();
    // "for tomorrow": only that day
    if let Some(iris_core::time::DayRef::Offset(days)) = scan(&norms, cx.lang).expr.day {
        let day = cx.now.date().checked_add(jiff::ToSpan::days(days as i64)).unwrap_or(cx.now.date());
        items.retain(|e| e.due_at.and_then(|d| jiff::Timestamp::from_millisecond(d).ok()).is_some_and(|t| t.to_zoned(cx.now.time_zone().clone()).date() == day));
    }
    if items.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let wants_next = cx.list("next").iter().any(|p| cx.utt.norm.contains(p.as_str()));
    if wants_next {
        let first = items.remove(0);
        let when = spoken(cx, &first);
        let when = when.strip_prefix(&first.title).unwrap_or(&when).trim().to_string();
        return Reply::done(cx.msg("next", &[("title", &first.title), ("when", &when)])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items: vec![first] });
    }
    let lines = items.iter().map(|e| spoken(cx, e)).collect::<Vec<_>>().join(", ");
    Reply::done(cx.messages.plural("list", items.len() as i64, &[("items", &lines)])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items })
}

fn delete(cx: &mut Cx, answering: bool) -> Reply {
    let items = open(cx);
    if items.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    if cx.utt.tokens.iter().any(|t| cx.list("all").contains(&t.norm)) {
        cx.ask("delete.confirm", json!({}));
        return Reply::question(cx.messages.plural("confirm_all", items.len() as i64, &[]));
    }
    let hit = crate::named(&items, &cx.utt.norm);
    match hit {
        Some(entry) => {
            cx.host.unschedule(&entry.id);
            cx.store.delete(&entry.id);
            Reply::done(cx.msg("deleted", &[("title", &entry.title)]))
        }
        None if answering => Reply::failed(cx.msg("not_found", &[])),
        None => {
            cx.ask("delete.which", json!({}));
            Reply::question(cx.msg("which_delete", &[])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items })
        }
    }
}

/// The open reminder an action names by title, or the reply that says why there is none.
fn picked(cx: &Cx, title: &str) -> Result<Entry, Box<Reply>> {
    let items = open(cx);
    match crate::by_title(&items, title) {
        Ok(entry) => Ok(entry.clone()),
        Err(crate::Miss::None) => Err(Box::new(Reply::failed(cx.msg("not_found", &[])))),
        Err(crate::Miss::Several) => Err(Box::new(Reply::failed(cx.lang.messages.get("action_unclear", &[])))),
    }
}

/// The user said yes to "write this to the calendar?": the event is created, or the reply says it could not be.
fn write_event(cx: &mut Cx) -> Reply {
    let title = cx.state["title"].as_str().unwrap_or_default().to_string();
    let start_at = cx.state["at_ms"].as_i64().unwrap_or(0);
    let minutes = cx.state["minutes"].as_u64().unwrap_or(30) as u32;
    let when = jiff::Timestamp::from_millisecond(start_at).map(|t| describe(t, &cx.now, cx.lang, false)).unwrap_or_default();
    match cx.host.create_event(&ExternalEvent { title: title.clone(), start_at, minutes, repeat: String::new() }) {
        Ok(calendar) => {
            cx.proceed();
            Reply::done(cx.msg("created_calendar", &[("title", &title), ("when", &when), ("calendar", &calendar)]))
        }
        Err(_) => Reply::failed(cx.msg("calendar_unreachable", &[])),
    }
}

fn delete_all(cx: &mut Cx) -> Reply {
    if !cx.lang.dialog.is_yes(&cx.utt.norm) {
        return Reply::done(cx.msg("kept", &[]));
    }
    let items = open(cx);
    for e in &items {
        cx.host.unschedule(&e.id);
        cx.store.delete(&e.id);
    }
    Reply::done(cx.messages.plural("deleted_all", items.len() as i64, &[]))
}

/// Moves a reminder to another time: "verschiebe die Erinnerung Oma auf 18 Uhr". `answering` is true
/// when this utterance answers one of the skill's own questions.
fn shift(cx: &mut Cx, id: Option<String>, mut expr: TimeExpr, answering: bool) -> Reply {
    let found = scan(&cx.utt.norms(), cx.lang);
    if id.is_none() || cx.state.is_null() || expr.is_empty() {
        expr.merge(found.expr.clone());
    }
    if id.is_some() && expr.is_empty() {
        expr.merge(found.expr);
    }
    let items = open(cx);
    if items.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let said = cx.utt.norm.clone();
    let target = id
        .and_then(|id| items.iter().find(|e| e.id == id).cloned())
        .or_else(|| crate::named(&items, &said).cloned())
        .or_else(|| (items.len() == 1).then(|| items[0].clone()));
    let Some(target) = target else {
        if answering {
            return Reply::failed(cx.msg("not_found", &[]));
        }
        cx.ask("move.which", json!({ "time": expr }));
        return Reply::question(cx.msg("which_move", &[])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items });
    };
    let resolved = match resolve(&expr, &cx.now, HalfDay::Nearest) {
        Ok(r) => r,
        Err(error) => {
            let key = match error {
                TimeError::Past => "time_past",
                TimeError::Invalid => "time_unclear",
                TimeError::Missing => "ask_move_time",
            };
            cx.ask("move.time", json!({ "id": target.id }));
            return Reply::question(cx.msg(key, &[("title", &target.title)]));
        }
    };
    // only a clock time ("auf 9 Uhr"): the same day as before, at the half of the day nearest the old time
    let at = same_day(&expr, &target, &cx.now).unwrap_or(resolved.at);
    let resolved = Resolved { at, ..resolved };
    relocate(cx, target, resolved.at)
}

/// Moves `target` to `at` and says so.
fn relocate(cx: &mut Cx, target: Entry, at: jiff::Timestamp) -> Reply {
    let mut moved = target;
    moved.due_at = Some(at.as_millisecond());
    cx.host.unschedule(&moved.id);
    cx.store.put(moved.clone());
    let _ = cx.host.schedule(&moved);
    cx.remember("last", &moved.id);
    let when = describe(at, &cx.now, cx.lang, false);
    Reply::done(cx.msg("moved", &[("title", &moved.title), ("when", &when)])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items: vec![moved] })
}

/// "Erinnere mich in 10 Minuten nochmal": the reminder that just rang, again later (ten minutes unless said).
fn snooze(cx: &mut Cx) -> Option<Reply> {
    let found = scan(&cx.utt.norms(), cx.lang);
    // a sentence that names a new thing to be reminded of is a new reminder
    let free: Vec<usize> = (0..cx.utt.len()).filter(|i| !found.used[*i]).collect();
    let title = join(cx.utt, &strip(cx.utt, free, cx.list("drop"), cx.list("lead"), cx.list("trail")));
    if !title.is_empty() {
        return Some(create(cx, Draft::default()));
    }
    snooze_by(cx, found.expr.relative_secs.unwrap_or(600))
}

/// The reminder that just rang (or was made last), again after `secs` seconds. `None` when there is none.
fn snooze_by(cx: &mut Cx, secs: i64) -> Option<Reply> {
    const WINDOW_MS: i64 = 30 * 60_000;
    let id = cx.recall("fired", WINDOW_MS).or_else(|| cx.recall("last", WINDOW_MS))?;
    let entry = cx.store.get(&id)?;
    let at = cx.now.timestamp().checked_add(jiff::ToSpan::seconds(secs)).ok()?;
    let mut again = if entry.repeat.is_empty() {
        entry.clone()
    } else {
        // a series keeps its own schedule: the snoozed ring is a one-off copy
        let mut copy = entry.clone();
        copy.id = cx.new_id();
        copy.repeat = String::new();
        copy
    };
    again.due_at = Some(at.as_millisecond());
    again.pending = true;
    cx.host.unschedule(&again.id);
    cx.store.put(again.clone());
    let _ = cx.host.schedule(&again);
    let duration = iris_core::time::describe_duration(secs, cx.lang);
    Some(Reply::done(cx.msg("snoozed", &[("title", &again.title), ("duration", &duration)])))
}

/// "Was muss ich heute tun?": today's reminders and how much is still open on the lists.
fn today(cx: &mut Cx) -> Reply {
    let tz = cx.now.time_zone().clone();
    let day = cx.now.date();
    let mut due: Vec<Entry> = open(cx).into_iter().filter(|e| e.due_at.and_then(|d| jiff::Timestamp::from_millisecond(d).ok()).is_some_and(|t| t.to_zoned(tz.clone()).date() == day)).collect();
    due.sort_by_key(|e| e.due_at);
    let open_items = cx.store.all(Some(EntryKind::ListItem)).into_iter().filter(|e| e.pending).count();
    if due.is_empty() && open_items == 0 {
        return Reply::done(cx.msg("nothing_today", &[]));
    }
    let mut said = String::new();
    if !due.is_empty() {
        let lines: Vec<String> = due
            .iter()
            .map(|e| {
                let z = jiff::Timestamp::from_millisecond(e.due_at.unwrap_or(0)).unwrap_or(jiff::Timestamp::UNIX_EPOCH).to_zoned(tz.clone());
                let time = cx.lang.format.clock.replace("{h}", &z.hour().to_string()).replace("{mm}", &format!("{:02}", z.minute()));
                cx.msg("today_at", &[("title", &e.title), ("time", &time)])
            })
            .collect();
        said = cx.msg("today_reminders", &[("items", &lines.join(", "))]);
    }
    if open_items > 0 {
        if !said.is_empty() {
            said.push(' ');
        }
        said.push_str(&cx.messages.plural("today_open", open_items as i64, &[]));
    }
    let reply = Reply::done(said);
    if due.is_empty() { reply } else { reply.with_card(Card::Entries { title: cx.msg("card_title", &[]), items: due }) }
}

/// The reminder's own day at the clock time said, when nothing but a clock time was said and that is still ahead.
fn same_day(expr: &TimeExpr, target: &Entry, now: &jiff::Zoned) -> Option<jiff::Timestamp> {
    let clock = expr.clock.as_ref()?;
    if expr.day.is_some() || expr.relative_secs.is_some() {
        return None;
    }
    let old = jiff::Timestamp::from_millisecond(target.due_at?).ok()?.to_zoned(now.time_zone().clone());
    let mut hours = vec![clock.hour];
    if !clock.exact && (1..12).contains(&clock.hour) {
        hours.push(clock.hour + 12);
    }
    hours
        .into_iter()
        .filter_map(|h| old.date().at(h as i8, clock.minute as i8, 0, 0).to_zoned(now.time_zone().clone()).ok())
        .filter(|z| z.timestamp() > now.timestamp())
        .min_by_key(|z| (z.timestamp().as_second() - old.timestamp().as_second()).abs())
        .map(|z| z.timestamp())
}
