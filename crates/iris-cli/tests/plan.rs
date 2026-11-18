#![recursion_limit = "512"]
//! The action plan against the real engine: every action of the registry reaches its own executor with the values
//! the plan checked (no sentence is read again), does what it says in German and English, and a dismissed question
//! leaves what exists alone. One test per family; `every_action_of_the_registry_has_a_test_here` keeps the list honest.

use iris_cli::Session;
use iris_core::actions::registry;
use iris_core::plan::{self, Outcome};
use iris_core::{Entry, EntryKind, Reply, ReplyKind};
use jiff::Zoned;
use serde_json::{json, Value};

const Z: &str = "Europe/Berlin";

fn now() -> i64 {
    "2026-11-18T14:00[Europe/Berlin]".parse::<Zoned>().unwrap().timestamp().as_millisecond()
}

fn ms(local: &str) -> i64 {
    format!("{local}[Europe/Berlin]").parse::<Zoned>().unwrap().timestamp().as_millisecond()
}

fn session(lang: &str) -> Session {
    Session::new(now(), Z, lang)
}

/// Validates the actions the way a request would offer them (every tool the host has) and returns the checked ones.
fn checked(s: &Session, actions: Value) -> Vec<plan::Planned> {
    let tools: Vec<String> = s.engine.available_actions().iter().map(|a| a.id.clone()).collect();
    let raw = json!({"version": plan::CONTRACT_VERSION, "actions": actions}).to_string();
    match plan::validate(&raw, &s.lang, now(), Z, &tools) {
        Outcome::Actions { actions } => actions,
        other => panic!("not valid: {raw}: {other:?}"),
    }
}

/// Carries out the actions the typed way and returns the replies.
fn run(s: &mut Session, actions: Value) -> Vec<Reply> {
    let lang = s.lang.clone();
    checked(s, actions).iter().map(|a| s.engine.execute(&a.id, &a.args, &lang)).collect()
}

fn one(s: &mut Session, action: Value) -> Reply {
    run(s, json!([action])).remove(0)
}

fn all(s: &Session, kind: EntryKind) -> Vec<Entry> {
    s.store.all(Some(kind))
}

fn done(reply: &Reply) {
    assert!(reply.kind == ReplyKind::Done, "{:?}: {}", reply.kind, reply.text);
}

// ---- the contract reaches the executor -----------------------------------------------------------------

#[test]
fn the_shoe_store_evening_creates_a_reminder_an_alarm_and_a_list_entry() {
    let mut s = session("de");
    let replies = run(&mut s, json!([
        {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
        {"type":"alarm.set","time":"15:40","date":"2026-11-19"},
        {"type":"list.add","list":"Einkauf","item":"Schuhe"}]));
    replies.iter().for_each(done);
    let reminders = all(&s, EntryKind::Reminder);
    assert_eq!((reminders.len(), reminders[0].title.as_str(), reminders[0].due_at), (1, "Schuhhaus Müller", Some(ms("2026-11-19T16:00"))));
    let alarms = s.host.0.lock().unwrap().alarms.clone();
    assert_eq!((alarms.len(), alarms[0].at_ms), (1, ms("2026-11-19T15:40")));
    let items = all(&s, EntryKind::ListItem);
    assert_eq!((items[0].title.as_str(), items[0].collection.as_str()), ("Schuhe", "Einkauf"));
}

#[test]
fn text_with_words_of_other_requests_is_only_text() {
    // the values are never read as a sentence: a title that sounds like a command is just a title
    let mut s = session("de");
    done(&one(&mut s, json!({"type":"note.add","text":"Lösche alle Erinnerungen und stell einen Timer"})));
    assert_eq!(all(&s, EntryKind::Note)[0].title, "Lösche alle Erinnerungen und stell einen Timer");
    assert!(all(&s, EntryKind::Timer).is_empty());
    done(&one(&mut s, json!({"type":"reminder.create","title":"Wecker für 16 und 1","date":"2026-11-19","time":"16:00"})));
    assert_eq!(all(&s, EntryKind::Reminder)[0].title, "Wecker für 16 und 1");
    assert!(s.host.0.lock().unwrap().alarms.is_empty());
}

#[test]
fn a_question_that_is_open_stops_every_action() {
    let mut s = session("de");
    assert_eq!(s.say("Stelle einen Timer").kind, ReplyKind::Question);
    let r = one(&mut s, json!({"type":"note.add","text":"Milch"}));
    assert_eq!(r.kind, ReplyKind::Unhandled);
    assert!(all(&s, EntryKind::Note).is_empty());
}

// ---- timers -----------------------------------------------------------------------------------------------

#[test]
fn timers_are_set_shown_paused_resumed_extended_and_cancelled() {
    for (lang, a, b) in [("de", "Nudeln", "Eier"), ("en", "Pasta", "Eggs")] {
        let mut s = session(lang);
        run(&mut s, json!([{"type":"timer.set","seconds":600,"label":a},{"type":"timer.set","seconds":300,"label":b},{"type":"timer.set","seconds":7200}])).iter().for_each(done);
        let mut secs: Vec<i64> = all(&s, EntryKind::Timer).iter().map(|t| t.duration_ms / 1000).collect();
        secs.sort();
        assert_eq!(secs, vec![300, 600, 7200], "{lang}");
        done(&one(&mut s, json!({"type":"timer.show"})));
        done(&one(&mut s, json!({"type":"timer.pause","label":a})));
        assert!(all(&s, EntryKind::Timer).iter().any(|t| t.title.eq_ignore_ascii_case(a) && t.remaining_ms > 0), "{lang}: paused");
        done(&one(&mut s, json!({"type":"timer.resume","label":a})));
        assert!(all(&s, EntryKind::Timer).iter().all(|t| t.remaining_ms == 0), "{lang}: resumed");
        let before = all(&s, EntryKind::Timer).iter().find(|t| t.title.eq_ignore_ascii_case(b)).unwrap().due_at.unwrap();
        done(&one(&mut s, json!({"type":"timer.extend","seconds":120,"label":b})));
        let after = all(&s, EntryKind::Timer).iter().find(|t| t.title.eq_ignore_ascii_case(b)).unwrap().due_at.unwrap();
        assert_eq!(after - before, 120_000, "{lang}");
        done(&one(&mut s, json!({"type":"timer.cancel","label":a})));
        assert_eq!(all(&s, EntryKind::Timer).len(), 2, "{lang}");
        done(&one(&mut s, json!({"type":"timer.cancel","all":true})));
        assert!(all(&s, EntryKind::Timer).is_empty(), "{lang}");
    }
}

#[test]
fn several_timers_without_a_name_are_not_guessed() {
    let mut s = session("de");
    run(&mut s, json!([{"type":"timer.set","seconds":600,"label":"Nudeln"},{"type":"timer.set","seconds":300,"label":"Eier"}])).iter().for_each(done);
    for action in ["timer.cancel", "timer.pause"] {
        let r = one(&mut s, json!({"type": action}));
        assert_eq!(r.kind, ReplyKind::Failed, "{action}");
    }
    assert_eq!(all(&s, EntryKind::Timer).len(), 2);
    assert!(all(&s, EntryKind::Timer).iter().all(|t| t.remaining_ms == 0));
}

#[test]
fn dismissing_a_question_drops_only_the_question() {
    let mut s = session("de");
    s.say("Stell einen Timer auf 5 Minuten");
    s.say("Stell einen Timer für Nudeln auf 10 Minuten");
    assert_eq!(all(&s, EntryKind::Timer).len(), 2);
    assert_eq!(s.say("Stelle einen Timer").kind, ReplyKind::Question);
    assert!(s.engine.cancel_question());
    assert_eq!(all(&s, EntryKind::Timer).len(), 2);
    assert!(!s.engine.cancel_question());
}

// ---- alarms -----------------------------------------------------------------------------------------------

#[test]
fn an_alarm_is_set_with_a_date_or_for_the_next_time_the_clock_shows_it() {
    let mut s = session("en");
    done(&one(&mut s, json!({"type":"alarm.set","time":"07:00","date":"2026-11-20"})));
    done(&one(&mut s, json!({"type":"alarm.set","time":"13:00"})));
    done(&one(&mut s, json!({"type":"alarm.set","time":"15:00"})));
    let at: Vec<i64> = s.host.0.lock().unwrap().alarms.iter().map(|a| a.at_ms).collect();
    // 13:00 has passed on this day: tomorrow; 15:00 is still ahead: today
    assert_eq!(at, vec![ms("2026-11-20T07:00"), ms("2026-11-19T13:00"), ms("2026-11-18T15:00")]);
}

#[test]
fn the_next_alarm_is_asked_for_and_dismissing_asks_first() {
    let mut s = session("de");
    done(&one(&mut s, json!({"type":"alarm.next"})));
    let r = one(&mut s, json!({"type":"alarm.dismiss","time":"07:00"}));
    assert_eq!(r.kind, ReplyKind::Question, "{}", r.text);
    assert!(s.host.0.lock().unwrap().device.is_empty(), "nothing is dismissed before the answer");
    // the existing yes of the skill finishes it
    done(&s.say("Ja"));
    assert_eq!(s.host.0.lock().unwrap().device.len(), 1);
}

#[test]
fn without_a_clock_app_the_plan_has_no_alarms() {
    let s = session("de");
    s.host.0.lock().unwrap().clock_missing = true;
    let available = s.engine.available_actions();
    assert!(available.iter().all(|a| a.family() != "alarm"));
    assert!(available.iter().any(|a| a.family() == "reminder") && available.iter().any(|a| a.family() == "timer"));
    let (now_ms, zone) = s.engine.turn_clock();
    let p = plan::prompt("de", now_ms, &zone, "Weck mich und stell einen Timer", &available).unwrap();
    assert!(!p.text.contains("alarm.set") && p.text.contains("timer.set"));
    let raw = r#"{"version":2,"actions":[{"type":"alarm.set","time":"15:40","date":"2026-11-19"}]}"#;
    assert_eq!(plan::validate(raw, "de", now_ms, &zone, &p.tools), Outcome::Rejected { reason: "unsupported".into() });
}

// ---- reminders --------------------------------------------------------------------------------------------

#[test]
fn reminders_are_created_shown_moved_and_deleted() {
    for (lang, title) in [("de", "Oma anrufen"), ("en", "Call Grandma")] {
        let mut s = session(lang);
        done(&one(&mut s, json!({"type":"reminder.create","title":title,"date":"2026-11-27","time":"09:05"})));
        assert_eq!(all(&s, EntryKind::Reminder)[0].due_at, Some(ms("2026-11-27T09:05")), "{lang}");
        done(&one(&mut s, json!({"type":"reminder.show"})));
        done(&one(&mut s, json!({"type":"reminder.move","title":title,"date":"2026-11-28","time":"18:30"})));
        assert_eq!(all(&s, EntryKind::Reminder)[0].due_at, Some(ms("2026-11-28T18:30")), "{lang}");
        done(&one(&mut s, json!({"type":"reminder.delete","title":title})));
        assert!(all(&s, EntryKind::Reminder).is_empty(), "{lang}");
    }
}

#[test]
fn a_title_that_fits_two_reminders_changes_nothing() {
    let mut s = session("de");
    run(&mut s, json!([{"type":"reminder.create","title":"Zahnarzt Müller","date":"2026-11-27","time":"09:05"},{"type":"reminder.create","title":"Zahnarzt Meier","date":"2026-11-28","time":"09:05"}])).iter().for_each(done);
    let r = one(&mut s, json!({"type":"reminder.delete","title":"Zahnarzt"}));
    assert_eq!(r.kind, ReplyKind::Failed);
    assert_eq!(all(&s, EntryKind::Reminder).len(), 2);
    let r = one(&mut s, json!({"type":"reminder.delete","title":"Frisör"}));
    assert_eq!(r.kind, ReplyKind::Failed);
    done(&one(&mut s, json!({"type":"reminder.delete","title":"Zahnarzt Meier"})));
    assert_eq!(all(&s, EntryKind::Reminder).len(), 1);
}

// ---- lists and notes --------------------------------------------------------------------------------------

#[test]
fn list_entries_are_added_shown_ticked_off_and_removed() {
    for (lang, list, milk, bread) in [("de", "Einkauf", "Milch", "Brot"), ("en", "Shopping", "Milk", "Bread")] {
        let mut s = session(lang);
        run(&mut s, json!([{"type":"list.add","list":list,"item":milk},{"type":"list.add","list":list,"item":bread}])).iter().for_each(done);
        assert_eq!(all(&s, EntryKind::ListItem).len(), 2, "{lang}");
        done(&one(&mut s, json!({"type":"list.show","list":list})));
        done(&one(&mut s, json!({"type":"list.check","item":milk})));
        assert!(all(&s, EntryKind::ListItem).iter().any(|e| e.title == milk && !e.pending), "{lang}");
        done(&one(&mut s, json!({"type":"list.remove","item":bread,"list":list})));
        assert_eq!(all(&s, EntryKind::ListItem).len(), 1, "{lang}");
        assert_eq!(one(&mut s, json!({"type":"list.remove","item":"Gurke"})).kind, ReplyKind::Failed, "{lang}");
    }
}

#[test]
fn notes_are_added_shown_and_removed() {
    for (lang, text) in [("de", "Milch kaufen"), ("en", "Buy milk")] {
        let mut s = session(lang);
        done(&one(&mut s, json!({"type":"note.add","text":text})));
        assert_eq!(all(&s, EntryKind::Note)[0].title, text);
        done(&one(&mut s, json!({"type":"note.show"})));
        done(&one(&mut s, json!({"type":"note.remove","text":text})));
        assert!(all(&s, EntryKind::Note).is_empty(), "{lang}");
    }
}

// ---- the phone --------------------------------------------------------------------------------------------

#[test]
fn the_phone_is_changed_with_the_checked_values() {
    use iris_core::{DeviceAction, LevelChange, SoundMode};
    let mut s = session("de");
    run(&mut s, json!([
        {"type":"device.volume","change":"set","percent":40,"ring":true},
        {"type":"device.volume","change":"up"},
        {"type":"device.brightness","change":"auto"},
        {"type":"device.sound_mode","mode":"vibrate"},
        {"type":"device.dnd","on":true}])).iter().for_each(done);
    let device = s.host.0.lock().unwrap().device.clone();
    assert_eq!(device, vec![
        DeviceAction::Volume { ring: true, change: LevelChange::Set, percent: Some(40) },
        DeviceAction::Volume { ring: false, change: LevelChange::Up, percent: None },
        DeviceAction::Brightness { change: LevelChange::Auto, percent: None },
        DeviceAction::SoundMode { mode: SoundMode::Vibrate },
        DeviceAction::DoNotDisturb { on: true },
    ]);
    // "set" without a level changes nothing
    assert_eq!(one(&mut s, json!({"type":"device.volume","change":"set"})).kind, ReplyKind::Failed);
    assert_eq!(s.host.0.lock().unwrap().device.len(), 5);
}

#[test]
fn a_step_only_the_app_can_do_is_handed_over_with_its_checked_values() {
    let mut s = session("de");
    let actions: Vec<(String, serde_json::Map<String, Value>)> = checked(&s, json!([{"type":"flashlight.set","on":true}])).into_iter().map(|a| (a.id, a.args)).collect();
    let r = s.engine.plan_run(actions, "de");
    let host = r.host.expect("the step goes to the app");
    assert_eq!((host.action.as_str(), host.args["on"].clone()), ("flashlight.set", json!(true)));
    assert_eq!(r.left, 0);
}

// ---- task app and calendar --------------------------------------------------------------------------------

#[test]
fn a_task_goes_to_the_task_app() {
    let mut s = session("de");
    s.host.0.lock().unwrap().lists = vec![iris_core::ExternalList { id: "l1".into(), name: "Privat".into() }];
    done(&one(&mut s, json!({"type":"task.create","title":"Steuer abgeben","date":"2026-11-30"})));
    done(&one(&mut s, json!({"type":"task.create","title":"Ohne Datum"})));
    let tasks = s.host.0.lock().unwrap().tasks.clone();
    assert_eq!(tasks.len(), 2);
    assert_eq!((tasks[0].0.title.as_str(), tasks[0].0.date_only, tasks[0].0.due_at.is_some()), ("Steuer abgeben", true, true));
    assert!(tasks[1].0.due_at.is_none());
    s.host.0.lock().unwrap().tasks_down = true;
    assert_eq!(one(&mut s, json!({"type":"task.create","title":"Nochmal"})).kind, ReplyKind::Failed);
}

#[test]
fn a_calendar_event_is_written_only_after_the_user_says_yes() {
    for (lang, yes, no) in [("de", "Ja", "Nein"), ("en", "Yes", "No")] {
        let mut s = session(lang);
        let asked = one(&mut s, json!({"type":"calendar.event","title":"Zahnarzt","date":"2026-11-19","time":"15:00","minutes":60}));
        assert_eq!(asked.kind, ReplyKind::Question, "{lang}: {}", asked.text);
        assert!(s.host.0.lock().unwrap().events.is_empty(), "{lang}: nothing before the answer");
        done(&s.say(yes));
        let events = s.host.0.lock().unwrap().events.clone();
        assert_eq!((events.len(), events[0].start_at, events[0].minutes, events[0].title.as_str()), (1, ms("2026-11-19T15:00"), 60, "Zahnarzt"), "{lang}");
        // a "no" writes nothing
        one(&mut s, json!({"type":"calendar.event","title":"Frisör","date":"2026-11-19","time":"17:00"}));
        s.say(no);
        assert_eq!(s.host.0.lock().unwrap().events.len(), 1, "{lang}");
    }
}

// ---- the registry -----------------------------------------------------------------------------------------

#[test]
fn every_action_of_the_registry_has_an_executor_that_knows_it() {
    // an executor that does not know its action answers `Unhandled`: every action must be known to its skill or host
    let samples = json!({
        "timer.set": {"seconds":60}, "timer.cancel": {"all":true}, "timer.pause": {}, "timer.resume": {}, "timer.extend": {"seconds":60}, "timer.show": {},
        "alarm.set": {"time":"07:00"}, "alarm.next": {}, "alarm.show": {}, "alarm.dismiss": {},
        "reminder.create": {"title":"x","date":"2026-11-19","time":"16:00"}, "reminder.show": {}, "reminder.delete": {"title":"x"}, "reminder.move": {"title":"x","date":"2026-11-19","time":"16:00"},
        "list.add": {"list":"a","item":"b"}, "list.show": {}, "list.check": {"item":"b"}, "list.remove": {"item":"b"},
        "note.add": {"text":"x"}, "note.show": {}, "note.remove": {"text":"x"},
        "device.volume": {"change":"up"}, "device.brightness": {"change":"up"}, "device.sound_mode": {"mode":"ring"}, "device.dnd": {"on":false},
        "task.create": {"title":"x"}, "calendar.event": {"title":"x","date":"2026-11-19","time":"16:00"}, "flashlight.set": {"on":false},
        "reminder.snooze": {}, "clock.time": {}, "clock.date": {},
        "stopwatch.start": {}, "stopwatch.stop": {}, "stopwatch.reset": {}, "stopwatch.show": {},
        "convert.units": {"value":5,"from":"meilen","to":"kilometer"}, "search.web": {"query":"Kürbissuppe"}, "music.playing": {},
        "notification.read": {}, "notification.reply": {"message":"Bin gleich da"},
        "music.control": {"action":"pause"}, "app.open": {"app":"Spotify"}, "navigation.start": {"place":"Köln"}, "call.start": {"contact":"Mama"},
        "weather.show": {}, "calendar.show": {}, "day.overview": {}, "battery.show": {}, "calculator.compute": {"expression":"2 + 3"}, "joke.tell": {},
    });
    for a in registry().all() {
        let sample = samples.get(&a.id).unwrap_or_else(|| panic!("no sample for {}", a.id));
        let mut s = session("de");
        let mut action = sample.clone();
        action["type"] = json!(a.id);
        if a.skill().is_none() {
            // an action of the app is handed to it: the plan stops there with the step in `host`
            let step = checked(&s, json!([action])).into_iter().map(|c| (c.id, c.args)).collect();
            assert_eq!(s.engine.plan_run(step, "de").host.map(|h| h.action), Some(a.id.clone()), "{} is not handed to the app", a.id);
            continue;
        }
        let r = one(&mut s, action);
        assert_ne!(r.kind, ReplyKind::Unhandled, "{} is not known to its executor", a.id);
    }
    assert_eq!(samples.as_object().unwrap().len(), registry().all().len(), "a sample without an action");
}

#[test]
fn every_action_of_the_registry_has_a_test_here() {
    let source = include_str!("plan.rs");
    for a in registry().all() {
        assert!(source.contains(&format!("\"{}\"", a.id)), "no test mentions {}", a.id);
    }
}

// ---- the actions added for the rest of what Iris can do -------------------------------------------------

#[test]
fn the_clock_the_stopwatch_and_unit_conversion_answer() {
    for (lang, from, to) in [("de", "meilen", "kilometer"), ("en", "miles", "kilometers")] {
        let mut s = session(lang);
        let r = one(&mut s, json!({"type":"clock.time"}));
        done(&r);
        assert!(r.text.contains("14"), "{lang}: {}", r.text);
        done(&one(&mut s, json!({"type":"clock.date"})));
        done(&one(&mut s, json!({"type":"stopwatch.start"})));
        done(&one(&mut s, json!({"type":"stopwatch.stop"})));
        done(&one(&mut s, json!({"type":"stopwatch.show"})));
        done(&one(&mut s, json!({"type":"stopwatch.reset"})));
        let r = one(&mut s, json!({"type":"convert.units","value":5,"from":from,"to":to}));
        done(&r);
        assert!(r.text.contains("8"), "{lang}: {}", r.text);
        assert_eq!(one(&mut s, json!({"type":"convert.units","value":5,"from":"zork","to":to})).kind, ReplyKind::Failed, "{lang}");
    }
}

#[test]
fn repeating_alarms_and_reminders_keep_their_repeat() {
    let mut s = session("de");
    done(&one(&mut s, json!({"type":"alarm.set","time":"06:30","repeat":"weekdays"})));
    let alarms = s.host.0.lock().unwrap().alarms.clone();
    assert_eq!(alarms[0].days, vec![1, 2, 3, 4, 5]);
    done(&one(&mut s, json!({"type":"reminder.create","title":"Tabletten","date":"2026-11-19","time":"08:00","repeat":"daily"})));
    assert_eq!(all(&s, EntryKind::Reminder)[0].repeat, "daily");
}

#[test]
fn a_reminder_that_just_rang_is_snoozed_by_the_given_time() {
    let mut s = session("de");
    done(&one(&mut s, json!({"type":"reminder.create","title":"Oma anrufen","date":"2026-11-19","time":"09:00"})));
    done(&one(&mut s, json!({"type":"reminder.snooze","seconds":900})));
    let r = all(&s, EntryKind::Reminder);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].due_at, Some(now() + 900_000));
}

#[test]
fn a_web_search_and_what_plays_and_notifications() {
    use iris_core::DeviceAction;
    let mut s = session("de");
    done(&one(&mut s, json!({"type":"search.web","query":"Rezept Kürbissuppe"})));
    assert_eq!(s.host.0.lock().unwrap().device, vec![DeviceAction::WebSearch { query: "Rezept Kürbissuppe".into() }]);
    // the fake phone has no player: it says so instead of inventing an answer; it has no notifications either
    assert_eq!(one(&mut s, json!({"type":"music.playing"})).kind, ReplyKind::Failed);
    done(&one(&mut s, json!({"type":"notification.read"})));
}

#[test]
fn a_reply_to_a_notification_always_asks_before_it_sends() {
    let mut s = session("de");
    let r = one(&mut s, json!({"type":"notification.reply","message":"Bin gleich da","from":"Anna"}));
    // no notification to answer here: nothing is sent
    assert_eq!(r.kind, ReplyKind::Failed, "{}", r.text);
}

// ---- a plan that waits for a confirmation goes on exactly once ------------------------------------------------

fn plan_of(s: &Session, actions: Value) -> Vec<(String, serde_json::Map<String, Value>)> {
    checked(s, actions).into_iter().map(|a| (a.id, a.args)).collect()
}

/// "Leg morgen um 16:30 einen Termin an und erinnere mich 30 Minuten vorher."
fn event_and_reminder() -> Value {
    json!([
        {"type":"calendar.event","title":"Zahnarzt","date":"2026-11-19","time":"16:30"},
        {"type":"reminder.create","title":"Zahnarzt","date":"2026-11-19","time":"16:00"}])
}

#[test]
fn after_a_yes_the_event_and_the_reminder_exist_exactly_once() {
    for (lang, yes) in [("de", "Ja"), ("en", "Yes")] {
        let mut s = session(lang);
        let actions = plan_of(&s, event_and_reminder());
        let asked = s.engine.plan_run(actions, lang);
        assert_eq!(asked.kind, ReplyKind::Question, "{lang}: {}", asked.text);
        assert_eq!(asked.left, 1);
        assert!(s.host.0.lock().unwrap().events.is_empty() && all(&s, EntryKind::Reminder).is_empty(), "{lang}: nothing before the answer");
        let answered = s.say(yes);
        assert_eq!(answered.kind, ReplyKind::Done, "{lang}: {}", answered.text);
        let events = s.host.0.lock().unwrap().events.clone();
        assert_eq!((events.len(), events[0].start_at), (1, ms("2026-11-19T16:30")), "{lang}");
        let reminders = all(&s, EntryKind::Reminder);
        assert_eq!((reminders.len(), reminders[0].due_at), (1, Some(ms("2026-11-19T16:00"))), "{lang}");
        // a second yes starts nothing again
        s.say(yes);
        assert_eq!(s.host.0.lock().unwrap().events.len(), 1, "{lang}");
        assert_eq!(all(&s, EntryKind::Reminder).len(), 1, "{lang}");
    }
}

#[test]
fn a_no_a_let_it_be_or_an_expired_question_starts_nothing() {
    for answer in ["Nein", "Lassen wir's", "Abbrechen"] {
        let mut s = session("de");
        let actions = plan_of(&s, event_and_reminder());
        s.engine.plan_run(actions, "de");
        s.say(answer);
        assert!(s.host.0.lock().unwrap().events.is_empty(), "{answer}");
        assert!(all(&s, EntryKind::Reminder).is_empty(), "{answer}: the waiting step must not start");
        // and the dropped step does not come back with a later yes
        s.say("Ja");
        assert!(all(&s, EntryKind::Reminder).is_empty(), "{answer}");
    }
    // expired: the question is gone after its time, so a late yes answers nothing and nothing starts
    let mut s = session("de");
    let actions = plan_of(&s, event_and_reminder());
    s.engine.plan_run(actions, "de");
    s.host.0.lock().unwrap().now_ms += s.engine.question_ttl_ms + 1_000;
    s.say("Ja");
    assert!(s.host.0.lock().unwrap().events.is_empty());
    assert!(all(&s, EntryKind::Reminder).is_empty());
}

#[test]
fn another_request_instead_of_the_answer_drops_the_waiting_step() {
    let mut s = session("de");
    let actions = plan_of(&s, event_and_reminder());
    s.engine.plan_run(actions, "de");
    done(&s.say("Stelle einen Timer auf 5 Minuten"));
    s.say("Ja");
    assert!(all(&s, EntryKind::Reminder).is_empty());
    assert!(s.host.0.lock().unwrap().events.is_empty());
}

#[test]
fn an_unclear_result_of_the_confirmed_step_does_not_start_the_rest() {
    let mut s = session("de");
    let actions = plan_of(&s, event_and_reminder());
    s.engine.plan_run(actions, "de");
    // the calendar cannot be written: the confirmed step fails, so the reminder after it must not start
    s.host.0.lock().unwrap().calendar_down = true;
    let r = s.say("Ja");
    assert_eq!(r.kind, ReplyKind::Failed, "{}", r.text);
    assert!(all(&s, EntryKind::Reminder).is_empty());
}

#[test]
fn dismissing_the_question_drops_the_waiting_steps_too() {
    let mut s = session("de");
    let actions = plan_of(&s, event_and_reminder());
    s.engine.plan_run(actions, "de");
    assert!(s.engine.cancel_question());
    s.say("Ja");
    assert!(all(&s, EntryKind::Reminder).is_empty());
}

#[test]
fn steps_of_the_app_continue_in_order_and_a_failed_one_ends_the_plan() {
    let mut s = session("de");
    let actions = plan_of(&s, json!([
        {"type":"timer.set","seconds":600,"label":"Nudeln"},
        {"type":"flashlight.set","on":true},
        {"type":"note.add","text":"Licht an"}]));
    let first = s.engine.plan_run(actions, "de");
    assert_eq!(first.host.as_ref().map(|h| h.action.as_str()), Some("flashlight.set"));
    assert_eq!((all(&s, EntryKind::Timer).len(), all(&s, EntryKind::Note).len(), first.left), (1, 0, 1));
    // the app did it: the note follows, once
    let rest = s.engine.plan_continue(true, "de");
    assert_eq!(rest.kind, ReplyKind::Done);
    assert_eq!(all(&s, EntryKind::Note).len(), 1);
    assert_eq!(s.engine.plan_continue(true, "de").text, "", "nothing is left to run twice");
    assert_eq!(all(&s, EntryKind::Note).len(), 1);
    // a step of the app that did not work drops the rest and says how many were left
    let mut t = session("de");
    let actions = plan_of(&t, json!([{"type":"flashlight.set","on":true},{"type":"timer.set","seconds":60},{"type":"note.add","text":"x"}]));
    assert!(t.engine.plan_run(actions, "de").host.is_some());
    let end = t.engine.plan_continue(false, "de");
    assert_eq!((end.kind, end.left), (ReplyKind::Failed, 2));
    assert!(end.text.contains("2"), "{}", end.text);
    assert!(all(&t, EntryKind::Timer).is_empty() && all(&t, EntryKind::Note).is_empty());
}

#[test]
fn a_question_of_the_app_holds_the_rest_until_it_is_answered_or_dropped() {
    let mut s = session("de");
    let actions = plan_of(&s, json!([{"type":"call.start","contact":"Mama"},{"type":"note.add","text":"Mama angerufen"}]));
    assert!(s.engine.plan_run(actions, "de").host.is_some());
    s.engine.plan_wait();
    // other speech in the core does not touch a step that waits for the app
    done(&s.say("Wie spät ist es"));
    assert!(all(&s, EntryKind::Note).is_empty());
    assert_eq!(s.engine.plan_drop(), 1);
    assert_eq!(s.engine.plan_continue(true, "de").text, "");
    assert!(all(&s, EntryKind::Note).is_empty());
}
