//! The action plan against the real engine: the sentences the core makes from a validated proposal do what
//! the proposal says, and a dismissed question leaves what exists alone.

use iris_cli::Session;
use iris_core::plan::{self, Families, Outcome};
use iris_core::{Entry, EntryKind, ReplyKind};
use jiff::Zoned;

fn now() -> i64 {
    "2026-11-18T14:00[Europe/Berlin]".parse::<Zoned>().unwrap().timestamp().as_millisecond()
}

fn ms(local: &str) -> i64 {
    format!("{local}[Europe/Berlin]").parse::<Zoned>().unwrap().timestamp().as_millisecond()
}

/// Carries out every action of `raw` the directed way (`process_planned`) and returns the replies.
fn run_directed(s: &mut Session, raw: &str) -> Vec<iris_core::Reply> {
    let families = s.engine.plan_families();
    let Outcome::Actions { actions } = plan::validate(raw, &s.lang, s.host.0.lock().unwrap().now_ms, "Europe/Berlin", &families) else {
        panic!("not valid: {raw}")
    };
    actions.iter().map(|a| s.engine.process_planned(a, &s.lang.clone())).collect()
}

/// Validates `raw`, says every sentence to the engine and returns the replies' text.
fn run(s: &mut Session, raw: &str) -> Vec<String> {
    let families = s.engine.plan_families();
    let Outcome::Actions { actions } = plan::validate(raw, &s.lang, s.host.0.lock().unwrap().now_ms, "Europe/Berlin", &families) else {
        panic!("not valid: {raw}")
    };
    actions.iter().map(|a| s.say(&a.sentence).text).collect()
}

fn titles(s: &Session, kind: EntryKind) -> Vec<Entry> {
    s.store.all(Some(kind))
}

#[test]
fn the_shoe_store_evening_creates_a_reminder_an_alarm_and_a_list_entry() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    let raw = r#"{"version":1,"actions":[
        {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
        {"type":"alarm.set","time":"15:40","date":"2026-11-19"},
        {"type":"list.add","list":"Einkauf","item":"Schuhe"}]}"#;
    let replies = run(&mut s, raw);
    assert_eq!(replies.len(), 3, "{replies:?}");
    let reminders = titles(&s, EntryKind::Reminder);
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].title, "Schuhhaus Müller");
    assert_eq!(reminders[0].due_at, Some(ms("2026-11-19T16:00")));
    let alarms = s.host.0.lock().unwrap().alarms.clone();
    assert_eq!(alarms.len(), 1);
    assert_eq!(alarms[0].at_ms, ms("2026-11-19T15:40"));
    let items = titles(&s, EntryKind::ListItem);
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].title.as_str(), items[0].collection.as_str()), ("Schuhe", "Einkauf"));
}

#[test]
fn every_kind_of_action_is_understood_by_the_engine_in_german_and_english() {
    let de = [
        r#"{"type":"reminder.create","title":"Oma","date":"2026-11-27","time":"09:05"}"#,
        r#"{"type":"alarm.set","time":"07:00","date":"2026-11-20"}"#,
        r#"{"type":"timer.set","seconds":600,"label":"Nudeln"}"#,
        r#"{"type":"timer.set","seconds":90}"#,
        r#"{"type":"list.add","list":"Einkauf","item":"Milch"}"#,
        r#"{"type":"note.add","text":"Milch kaufen"}"#,
    ];
    let en = [
        r#"{"type":"reminder.create","title":"Grandma","date":"2026-11-27","time":"09:05"}"#,
        r#"{"type":"alarm.set","time":"07:00","date":"2026-11-20"}"#,
        r#"{"type":"timer.set","seconds":600,"label":"Pasta"}"#,
        r#"{"type":"timer.set","seconds":90}"#,
        r#"{"type":"list.add","list":"Shopping","item":"Milk"}"#,
        r#"{"type":"note.add","text":"Buy milk"}"#,
    ];
    for (lang, actions) in [("de", de), ("en", en)] {
        for action in actions {
            // a fresh engine per action, so one that fails says which
            let raw = format!(r#"{{"version":1,"actions":[{action}]}}"#);
            let outcome = plan::validate(&raw, lang, now(), "Europe/Berlin", &Families::ALL);
            let Outcome::Actions { actions } = outcome else { panic!("{lang}: {outcome:?}") };
            let mut s = Session::new(now(), "Europe/Berlin", lang);
            let reply = s.say(&actions[0].sentence);
            let created = s.store.all(None).len() + s.host.0.lock().unwrap().alarms.len();
            assert!(created >= 1 && reply.kind == ReplyKind::Done, "{lang} {}: {:?} -> {}", actions[0].kind, actions[0].sentence, reply.text);
        }
    }
}

#[test]
fn the_dates_come_out_as_the_proposal_said() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    run(&mut s, r#"{"version":1,"actions":[{"type":"reminder.create","title":"Oma","date":"2026-11-27","time":"09:05"},{"type":"alarm.set","time":"07:00","date":"2026-11-20"}]}"#);
    assert_eq!(titles(&s, EntryKind::Reminder)[0].due_at, Some(ms("2026-11-27T09:05")));
    assert_eq!(s.host.0.lock().unwrap().alarms[0].at_ms, ms("2026-11-20T07:00"));
}

#[test]
fn a_timer_amount_is_what_the_proposal_said() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    run(&mut s, r#"{"version":1,"actions":[{"type":"timer.set","seconds":600,"label":"Nudeln"},{"type":"timer.set","seconds":90},{"type":"timer.set","seconds":7200}]}"#);
    let mut seconds: Vec<i64> = titles(&s, EntryKind::Timer).iter().map(|t| t.duration_ms / 1000).collect();
    seconds.sort();
    assert_eq!(seconds, vec![90, 600, 7200]);
}

#[test]
fn dismissing_a_question_drops_only_the_question() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    s.say("Stell einen Timer auf 10 Minuten");
    assert_eq!(titles(&s, EntryKind::Timer).len(), 1);
    // an unfinished reminder: the core asks what it should say
    assert_eq!(s.say("Erstelle eine Erinnerung").kind, ReplyKind::Question);
    assert!(s.engine.has_question());
    assert!(s.engine.cancel_question());
    assert!(!s.engine.has_question());
    // the timer stays, nothing was created from the half-finished request
    assert_eq!(titles(&s, EntryKind::Timer).len(), 1);
    assert!(titles(&s, EntryKind::Reminder).is_empty());
    // and with nothing open there is nothing to drop
    assert!(!s.engine.cancel_question());
}

#[test]
fn dismissing_the_setup_of_a_timer_leaves_the_running_timers_alone() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    s.say("Stell einen Timer auf 5 Minuten");
    s.say("Stell einen Timer für Nudeln auf 10 Minuten");
    assert_eq!(titles(&s, EntryKind::Timer).len(), 2);
    assert_eq!(s.say("Stelle einen Timer").kind, ReplyKind::Question);
    assert!(s.engine.cancel_question());
    assert_eq!(titles(&s, EntryKind::Timer).len(), 2);
}

#[test]
fn without_a_clock_app_the_plan_has_no_alarms() {
    let s = Session::new(now(), "Europe/Berlin", "de");
    s.host.0.lock().unwrap().clock_missing = true;
    let families = s.engine.plan_families();
    assert!(!families.alarm && families.reminder && families.timer && families.list && families.note);
    let (now_ms, zone) = s.engine.turn_clock();
    assert!(!plan::prompt("de", now_ms, &zone, &families).unwrap().contains("alarm.set"));
    let raw = r#"{"version":1,"actions":[{"type":"alarm.set","time":"15:40","date":"2026-11-19"}]}"#;
    assert_eq!(plan::validate(raw, "de", now_ms, &zone, &families), Outcome::Rejected { reason: "unsupported".into() });
}

#[test]
fn a_planned_action_reaches_its_own_skill_and_nothing_else() {
    for lang in ["de", "en"] {
        let mut s = Session::new(now(), "Europe/Berlin", lang);
        let raw = r#"{"version":1,"actions":[
            {"type":"reminder.create","title":"Oma","date":"2026-11-27","time":"09:05"},
            {"type":"alarm.set","time":"07:00","date":"2026-11-20"},
            {"type":"timer.set","seconds":600,"label":"Nudeln"},
            {"type":"list.add","list":"Einkauf","item":"Milch"},
            {"type":"note.add","text":"Milch kaufen"}]}"#;
        let replies = run_directed(&mut s, raw);
        let named: Vec<(String, String)> = replies.iter().map(|r| (r.skill.clone(), r.intent.clone())).collect();
        assert!(replies.iter().all(|r| r.kind != ReplyKind::Unhandled), "{lang}: {named:?}");
        assert_eq!(named[0].0, "reminder", "{named:?}");
        assert_eq!(named[2].0, "timer", "{named:?}");
        assert_eq!(named[3].0, "lists", "{named:?}");
        assert_eq!(titles(&s, EntryKind::Reminder).len(), 1);
        assert_eq!(titles(&s, EntryKind::Timer).len(), 1);
        assert_eq!(titles(&s, EntryKind::ListItem).len(), 1);
        assert_eq!(titles(&s, EntryKind::Note).len(), 1);
    }
}

#[test]
fn a_planned_action_does_nothing_while_a_question_is_open_or_when_its_skill_does_not_take_it() {
    let mut s = Session::new(now(), "Europe/Berlin", "de");
    let open = s.say("Stell einen Timer");
    assert_eq!(open.kind, ReplyKind::Question);
    let planned = plan::Planned { kind: "note.add".into(), sentence: "Notiere Milch".into() };
    assert_eq!(s.engine.process_planned(&planned, "de").kind, ReplyKind::Unhandled);
    assert!(titles(&s, EntryKind::Note).is_empty());
    s.engine.cancel_question();
    // a sentence that is no note must not be read as another request
    let wrong = plan::Planned { kind: "note.add".into(), sentence: "Wie spät ist es".into() };
    assert_eq!(s.engine.process_planned(&wrong, "de").kind, ReplyKind::Unhandled);
    assert!(titles(&s, EntryKind::Note).is_empty());
}
