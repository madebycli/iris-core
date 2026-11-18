//! Who handles a sentence, and the contract in which the assistant AI may propose actions.
//!
//! The four levels of "AI use" say how readily a sentence goes to the online assistant instead of staying with
//! the local skills ([`route`]). When a sentence is handed over for planning, the assistant answers in a compact
//! JSON contract ([`prompt`] tells it the contract). Nothing of that reply is trusted: [`validate`] checks the
//! whole proposal before any side effect and turns every action into a sentence in the engine's own grammar, so
//! the skills that already exist carry it out. A clarification is a dialogue turn: it never comes with actions.
//!
//! No word of a spoken language is written here. Words, sentence templates and questions are data in
//! `core/lang/plan/<code>.toml`.

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The version of the action contract. A proposal with another version is rejected.
pub const CONTRACT_VERSION: u32 = 1;
/// Most actions one proposal may hold.
pub const MAX_ACTIONS: usize = 5;
const MAX_TITLE: usize = 80;
const MAX_NOTE: usize = 200;
const MAX_TIMER_SECONDS: u32 = 86_400;
const MAX_DAYS_AHEAD: i64 = 366;
const MAX_REPLY_BYTES: usize = 8 * 1024;
const SLOTS: [&str; 5] = ["time", "date", "duration", "title", "list"];

/// Where a sentence goes at a given level of AI use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// The local skills answer.
    Local,
    /// The assistant answers in text (a question about earlier turns, a real question).
    Talk,
    /// The assistant proposes actions in the contract; the core validates and carries them out.
    Plan,
}

/// What a proposal turned out to be.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// Valid actions, each as the sentence the engine understands, in order.
    Actions { actions: Vec<Planned> },
    /// One question back to the user. No action comes with it.
    Ask { slot: String, question: String },
    /// The message is no task for these actions: the assistant answers it in text instead.
    NotAction,
    /// Not usable: nothing is carried out. `reason` is a stable code (see [`Rejection`]).
    Rejected { reason: String },
}

/// One validated action as the sentence for the engine.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Planned {
    /// `reminder.create`, `alarm.set`, `timer.set`, `list.add` or `note.add`.
    pub kind: String,
    pub sentence: String,
}

/// Why a proposal was rejected (the `reason` code of [`Outcome::Rejected`]).
pub struct Rejection;

impl Rejection {
    pub const MALFORMED: &'static str = "malformed";
    pub const VERSION: &'static str = "version";
    pub const EMPTY: &'static str = "empty";
    pub const TOO_MANY: &'static str = "too_many";
    pub const UNKNOWN_ACTION: &'static str = "unknown_action";
    pub const MIXED: &'static str = "mixed";
    pub const TEXT: &'static str = "text";
    pub const DATE: &'static str = "date";
    pub const TIME: &'static str = "time";
    pub const PAST: &'static str = "past";
    pub const DURATION: &'static str = "duration";
    pub const SLOT: &'static str = "slot";
    pub const LANGUAGE: &'static str = "language";
    pub const ZONE: &'static str = "zone";
}

fn rejected(reason: &str) -> Outcome {
    Outcome::Rejected { reason: reason.to_string() }
}

// ---- the language data ----------------------------------------------------------------------------------

#[derive(Deserialize)]
struct PackSource {
    routing: RoutingSource,
    format: Format,
    render: Render,
    ask: HashMap<String, String>,
    prompt: PromptSource,
}

#[derive(Deserialize)]
struct RoutingSource {
    reference: String,
    sequence: String,
    command: String,
    question: String,
    simple: Vec<String>,
}

#[derive(Deserialize)]
struct Format {
    date: String,
    weekdays: Vec<String>,
}

#[derive(Deserialize)]
struct Render {
    reminder: String,
    when_today: String,
    when_tomorrow: String,
    when_date: String,
    alarm: String,
    alarm_today: String,
    alarm_tomorrow: String,
    alarm_date: String,
    timer: String,
    timer_label: String,
    seconds: String,
    minutes: String,
    hours: String,
    list: String,
    note: String,
}

#[derive(Deserialize)]
struct PromptSource {
    system: String,
}

struct Pack {
    reference: Regex,
    sequence: Regex,
    command: Regex,
    question: Regex,
    simple: Vec<Regex>,
    format: Format,
    render: Render,
    ask: HashMap<String, String>,
    system: String,
}

const DE: &str = include_str!("../../../lang/plan/de.toml");
const EN: &str = include_str!("../../../lang/plan/en.toml");

fn compile(source: &str) -> Pack {
    let p: PackSource = toml::from_str(source).expect("plan language data is valid");
    let re = |s: &str| Regex::new(s).expect("plan language data holds valid expressions");
    Pack {
        reference: re(&p.routing.reference),
        sequence: re(&p.routing.sequence),
        command: re(&p.routing.command),
        question: re(&p.routing.question),
        simple: p.routing.simple.iter().map(|s| re(s)).collect(),
        format: p.format,
        render: p.render,
        ask: p.ask,
        system: p.prompt.system,
    }
}

fn pack(lang: &str) -> Option<&'static Pack> {
    static PACKS: OnceLock<HashMap<&'static str, Pack>> = OnceLock::new();
    PACKS.get_or_init(|| HashMap::from([("de", compile(DE)), ("en", compile(EN))])).get(lang)
}

/// The languages the planner has words for.
pub fn languages() -> Vec<&'static str> {
    vec!["de", "en"]
}

// ---- routing --------------------------------------------------------------------------------------------

/// Where a sentence goes at AI-use `level` (1 sparing, 2 balanced, 3 smart, 4 AI first).
///
/// `open_question`: a question of the core is open and this is its answer; that always stays local. At level 1
/// nothing is decided here: the older routing (a sentence nothing understood, knowledge questions, follow-ups)
/// applies. Simple commands and sentences of two words or fewer stay local at every level.
pub fn route(level: u8, text: &str, lang: &str, open_question: bool) -> Route {
    if level <= 1 || open_question {
        return Route::Local;
    }
    let Some(p) = pack(lang) else { return Route::Local };
    let words = text.split_whitespace().count();
    if words <= 2 || p.simple.iter().any(|r| r.is_match(text)) {
        return Route::Local;
    }
    let reference = p.reference.is_match(text);
    let multi = (p.sequence.is_match(text) && words >= 9) || words >= 25;
    // A request put as a question is a command, not a question for the assistant to answer.
    let question = p.question.is_match(text) && !p.command.is_match(text);
    match level {
        2 => {
            if reference {
                Route::Talk
            } else if multi {
                Route::Plan
            } else {
                Route::Local
            }
        }
        3 => {
            if reference {
                Route::Talk
            } else if multi {
                Route::Plan
            } else if words >= 9 {
                if question {
                    Route::Talk
                } else {
                    Route::Plan
                }
            } else {
                Route::Local
            }
        }
        _ => {
            if reference || question {
                Route::Talk
            } else {
                Route::Plan
            }
        }
    }
}

// ---- the prompt -----------------------------------------------------------------------------------------

/// What the assistant is told: the contract, and the turn's clock (today, the weekday, the time zone), so a
/// relative word like "tomorrow" is turned into a fixed date against the same clock the core validates with.
pub fn prompt(lang: &str, now_ms: i64, zone: &str) -> Option<String> {
    let p = pack(lang)?;
    let now = turn_clock(now_ms, zone)?;
    let weekday = p.format.weekdays.get(now.weekday().to_monday_zero_offset() as usize)?;
    Some(
        p.system
            .replace("{weekday}", weekday)
            .replace("{today}", &now.date().to_string())
            .replace("{zone}", zone)
            .replace("{time}", &now.strftime("%H:%M").to_string()),
    )
}

fn turn_clock(now_ms: i64, zone: &str) -> Option<Zoned> {
    let tz = TimeZone::get(zone).ok()?;
    Some(Timestamp::from_millisecond(now_ms).ok()?.to_zoned(tz))
}

// ---- the contract ---------------------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    version: u32,
    #[serde(default)]
    actions: Vec<serde_json::Value>,
    #[serde(default)]
    ask: Option<RawAsk>,
    #[serde(default)]
    not_action: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsk {
    slot: String,
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum RawAction {
    #[serde(rename = "reminder.create")]
    ReminderCreate { title: String, date: String, time: String },
    #[serde(rename = "alarm.set")]
    AlarmSet { time: String, #[serde(default)] date: Option<String> },
    #[serde(rename = "timer.set")]
    TimerSet { seconds: u32, #[serde(default)] label: Option<String> },
    #[serde(rename = "list.add")]
    ListAdd { list: String, item: String },
    #[serde(rename = "note.add")]
    NoteAdd { text: String },
}

/// Checks the whole proposal in `raw` (the assistant's reply, possibly wrapped in a code fence or a sentence)
/// against the turn's clock `now_ms` in time zone `zone`, and turns valid actions into engine sentences in
/// `lang`. Either everything is valid or nothing is carried out.
pub fn validate(raw: &str, lang: &str, now_ms: i64, zone: &str) -> Outcome {
    let Some(p) = pack(lang) else { return rejected(Rejection::LANGUAGE) };
    let Some(now) = turn_clock(now_ms, zone) else { return rejected(Rejection::ZONE) };
    if raw.len() > MAX_REPLY_BYTES {
        return rejected(Rejection::MALFORMED);
    }
    let Some(json) = extract_json(raw) else { return rejected(Rejection::MALFORMED) };
    let Ok(proposal) = serde_json::from_str::<Proposal>(json) else { return rejected(Rejection::MALFORMED) };
    if proposal.version != CONTRACT_VERSION {
        return rejected(Rejection::VERSION);
    }
    let has_actions = !proposal.actions.is_empty();
    // A clarification is a dialogue turn: it cannot also create an entry, and a "no task" cannot carry actions.
    if (has_actions && (proposal.ask.is_some() || proposal.not_action)) || (proposal.ask.is_some() && proposal.not_action) {
        return rejected(Rejection::MIXED);
    }
    if proposal.not_action {
        return Outcome::NotAction;
    }
    if let Some(ask) = proposal.ask {
        return match (SLOTS.contains(&ask.slot.as_str()), p.ask.get(&ask.slot)) {
            (true, Some(question)) => Outcome::Ask { slot: ask.slot, question: question.clone() },
            _ => rejected(Rejection::SLOT),
        };
    }
    if !has_actions {
        return rejected(Rejection::EMPTY);
    }
    if proposal.actions.len() > MAX_ACTIONS {
        return rejected(Rejection::TOO_MANY);
    }
    let mut planned = Vec::with_capacity(proposal.actions.len());
    for value in proposal.actions {
        let action: RawAction = match serde_json::from_value(value) {
            Ok(a) => a,
            Err(e) if e.to_string().contains("unknown variant") => return rejected(Rejection::UNKNOWN_ACTION),
            Err(_) => return rejected(Rejection::MALFORMED),
        };
        match plan_one(p, &now, action) {
            Ok(one) => planned.push(one),
            Err(reason) => return rejected(reason),
        }
    }
    Outcome::Actions { actions: planned }
}

/// The JSON object in `raw`: from the first `{` to the last `}`.
fn extract_json(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

fn plan_one(p: &Pack, now: &Zoned, action: RawAction) -> Result<Planned, &'static str> {
    let fill = |template: &str, pairs: &[(&str, &str)]| {
        pairs.iter().fold(template.to_string(), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
    };
    match action {
        RawAction::ReminderCreate { title, date, time } => {
            let title = clean(&title, MAX_TITLE).ok_or(Rejection::TEXT)?;
            let (date, time) = (parse_date(&date, now)?, parse_time(&time)?);
            require_future(now, date, time)?;
            let clock = time.strftime("%H:%M").to_string();
            let when = when_phrase(p, now, date, &clock, [&p.render.when_today, &p.render.when_tomorrow, &p.render.when_date])?;
            Ok(Planned { kind: "reminder.create".into(), sentence: fill(&p.render.reminder, &[("when", &when), ("title", &title)]) })
        }
        RawAction::AlarmSet { time, date } => {
            let time = parse_time(&time)?;
            let clock = time.strftime("%H:%M").to_string();
            let when = match date {
                None => String::new(),
                Some(d) => {
                    let date = parse_date(&d, now)?;
                    require_future(now, date, time)?;
                    when_phrase(p, now, date, &clock, [&p.render.alarm_today, &p.render.alarm_tomorrow, &p.render.alarm_date])?
                }
            };
            Ok(Planned { kind: "alarm.set".into(), sentence: fill(&p.render.alarm, &[("time", &clock), ("when", &when)]) })
        }
        RawAction::TimerSet { seconds, label } => {
            if !(1..=MAX_TIMER_SECONDS).contains(&seconds) {
                return Err(Rejection::DURATION);
            }
            let amount = if seconds % 3600 == 0 {
                fill(&p.render.hours, &[("n", &(seconds / 3600).to_string())])
            } else if seconds % 60 == 0 {
                fill(&p.render.minutes, &[("n", &(seconds / 60).to_string())])
            } else {
                fill(&p.render.seconds, &[("n", &seconds.to_string())])
            };
            let label = match label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
                Some(l) => fill(&p.render.timer_label, &[("label", &clean(l, MAX_TITLE).ok_or(Rejection::TEXT)?)]),
                None => String::new(),
            };
            Ok(Planned { kind: "timer.set".into(), sentence: fill(&p.render.timer, &[("label", &label), ("amount", &amount)]) })
        }
        RawAction::ListAdd { list, item } => {
            let (list, item) = (clean(&list, MAX_TITLE).ok_or(Rejection::TEXT)?, clean(&item, MAX_TITLE).ok_or(Rejection::TEXT)?);
            Ok(Planned { kind: "list.add".into(), sentence: fill(&p.render.list, &[("item", &item), ("list", &list)]) })
        }
        RawAction::NoteAdd { text } => {
            let text = clean(&text, MAX_NOTE).ok_or(Rejection::TEXT)?;
            Ok(Planned { kind: "note.add".into(), sentence: fill(&p.render.note, &[("text", &text)]) })
        }
    }
}

/// Text from the assistant that ends up inside a sentence for the engine. It must be a plain phrase: no control
/// characters, no sentence punctuation or brackets that could turn it into a second command.
fn clean(text: &str, max: usize) -> Option<String> {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = joined
        .trim_matches(|c| matches!(c, '"' | '“' | '”' | '„' | '\'' | ' '))
        .trim_end_matches(['.', '!', '?'])
        .trim();
    if t.is_empty() || t.chars().count() > max {
        return None;
    }
    if t.chars().any(|c| c.is_control() || matches!(c, '.' | ';' | ':' | '!' | '?' | '<' | '>' | '{' | '}' | '[' | ']')) {
        return None;
    }
    Some(t.to_string())
}

fn parse_time(s: &str) -> Result<Time, &'static str> {
    let ok = s.len() == 5 && s.as_bytes()[2] == b':' && s.bytes().enumerate().all(|(i, b)| i == 2 || b.is_ascii_digit());
    if !ok {
        return Err(Rejection::TIME);
    }
    s.parse::<Time>().map_err(|_| Rejection::TIME)
}

/// An ISO date from today up to a year ahead, against the turn's clock.
fn parse_date(s: &str, now: &Zoned) -> Result<Date, &'static str> {
    let ok = s.len() == 10 && s.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
    if !ok {
        return Err(Rejection::DATE);
    }
    let date = s.parse::<Date>().map_err(|_| Rejection::DATE)?;
    let today = now.date();
    if date < today {
        return Err(Rejection::PAST);
    }
    if date > today.checked_add(MAX_DAYS_AHEAD.days()).map_err(|_| Rejection::DATE)? {
        return Err(Rejection::DATE);
    }
    Ok(date)
}

fn require_future(now: &Zoned, date: Date, time: Time) -> Result<(), &'static str> {
    let at = date.at(time.hour(), time.minute(), 0, 0).to_zoned(now.time_zone().clone()).map_err(|_| Rejection::TIME)?;
    if at.timestamp() <= now.timestamp() {
        return Err(Rejection::PAST);
    }
    Ok(())
}

fn when_phrase(p: &Pack, now: &Zoned, date: Date, clock: &str, templates: [&String; 3]) -> Result<String, &'static str> {
    let today = now.date();
    let template = if date == today {
        templates[0]
    } else if Some(date) == today.tomorrow().ok() {
        templates[1]
    } else {
        templates[2]
    };
    let shown = jiff::fmt::strtime::format(&p.format.date, date).map_err(|_| Rejection::DATE)?;
    Ok(template.replace("{time}", clock).replace("{date}", &shown))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wednesday 2026-11-18, 14:00 in Berlin: the turn's clock in every case.
    fn now() -> i64 {
        "2026-11-18T14:00[Europe/Berlin]".parse::<Zoned>().unwrap().timestamp().as_millisecond()
    }
    const Z: &str = "Europe/Berlin";

    fn sentences(raw: &str, lang: &str) -> Vec<String> {
        match validate(raw, lang, now(), Z) {
            Outcome::Actions { actions } => actions.into_iter().map(|a| a.sentence).collect(),
            other => panic!("expected actions, got {other:?}"),
        }
    }

    fn reason(raw: &str) -> String {
        match validate(raw, "de", now(), Z) {
            Outcome::Rejected { reason } => reason,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn the_language_data_is_valid() {
        for lang in languages() {
            assert!(pack(lang).is_some(), "{lang}");
            assert!(prompt(lang, now(), Z).unwrap().contains("2026-11-18"));
        }
    }

    #[test]
    fn the_shoe_store_evening_from_the_log_becomes_three_sentences() {
        let raw = r#"{"version":1,"actions":[
            {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
            {"type":"alarm.set","time":"15:40","date":"2026-11-19"},
            {"type":"list.add","list":"Einkauf","item":"Schuhe"}]}"#;
        assert_eq!(
            sentences(raw, "de"),
            vec![
                "Erinnere mich morgen um 16:00 Uhr an Schuhhaus Müller",
                "Stelle einen Wecker auf 15:40 Uhr morgen",
                "Setze Schuhe auf die Liste Einkauf",
            ]
        );
    }

    #[test]
    fn english_sentences_use_english_words() {
        let raw = r#"{"version":1,"actions":[
            {"type":"reminder.create","title":"Shoe store","date":"2026-11-20","time":"16:00"},
            {"type":"timer.set","seconds":600,"label":"Pasta"},
            {"type":"timer.set","seconds":90},
            {"type":"note.add","text":"Buy milk"}]}"#;
        assert_eq!(
            sentences(raw, "en"),
            vec!["Remind me on November 20 at 16:00 about Shoe store", "Set a timer called Pasta for 10 minutes", "Set a timer for 90 seconds", "Note Buy milk"]
        );
    }

    #[test]
    fn dates_and_amounts_are_rendered_for_the_engine() {
        let raw = r#"{"version":1,"actions":[
            {"type":"reminder.create","title":"Zahnarzt","date":"2026-11-18","time":"17:30"},
            {"type":"reminder.create","title":"Oma","date":"2026-11-27","time":"09:05"},
            {"type":"alarm.set","time":"07:00"},
            {"type":"timer.set","seconds":7200}]}"#;
        assert_eq!(
            sentences(raw, "de"),
            vec!["Erinnere mich um 17:30 Uhr an Zahnarzt", "Erinnere mich am 27.11.2026 um 09:05 Uhr an Oma", "Stelle einen Wecker auf 07:00 Uhr", "Stelle einen Timer auf 2 Stunden"]
        );
    }

    #[test]
    fn a_proposal_in_a_code_fence_or_a_sentence_is_read() {
        let fenced = "Hier ist der Plan:\n```json\n{\"version\":1,\"actions\":[{\"type\":\"note.add\",\"text\":\"Milch kaufen\"}]}\n```";
        assert_eq!(sentences(fenced, "de"), vec!["Notiere Milch kaufen"]);
    }

    #[test]
    fn a_clarification_is_a_turn_of_its_own() {
        match validate(r#"{"version":1,"ask":{"slot":"time"}}"#, "de", now(), Z) {
            Outcome::Ask { slot, question } => assert_eq!((slot.as_str(), question.as_str()), ("time", "Um wie viel Uhr?")),
            other => panic!("{other:?}"),
        }
        // never a question together with an action
        let mixed = r#"{"version":1,"ask":{"slot":"time"},"actions":[{"type":"note.add","text":"x"}]}"#;
        assert_eq!(reason(mixed), Rejection::MIXED);
        assert_eq!(reason(r#"{"version":1,"ask":{"slot":"password"}}"#), Rejection::SLOT);
    }

    #[test]
    fn not_a_task_is_passed_back() {
        assert_eq!(validate(r#"{"version":1,"not_action":true}"#, "de", now(), Z), Outcome::NotAction);
        assert_eq!(reason(r#"{"version":1,"not_action":true,"actions":[{"type":"note.add","text":"x"}]}"#), Rejection::MIXED);
    }

    #[test]
    fn nothing_is_carried_out_unless_everything_is_valid() {
        let one_bad = r#"{"version":1,"actions":[{"type":"note.add","text":"Milch"},{"type":"reminder.create","title":"Oma","date":"2026-11-17","time":"09:00"}]}"#;
        assert_eq!(reason(one_bad), Rejection::PAST);
        let unknown = r#"{"version":1,"actions":[{"type":"phone.format","all":true}]}"#;
        assert_eq!(reason(unknown), Rejection::UNKNOWN_ACTION);
        assert_eq!(reason(r#"{"version":2,"actions":[{"type":"note.add","text":"x"}]}"#), Rejection::VERSION);
        assert_eq!(reason(r#"{"version":1,"actions":[]}"#), Rejection::EMPTY);
        assert_eq!(reason("kein json"), Rejection::MALFORMED);
        let many = format!(r#"{{"version":1,"actions":[{}]}}"#, [r#"{"type":"note.add","text":"x"}"#; 6].join(","));
        assert_eq!(reason(&many), Rejection::TOO_MANY);
    }

    #[test]
    fn unknown_fields_are_not_ignored() {
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"note.add","text":"x","run":"rm"}]}"#), Rejection::MALFORMED);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"note.add","text":"x"}],"extra":1}"#), Rejection::MALFORMED);
    }

    #[test]
    fn a_time_of_day_a_duration_and_a_date_are_not_interchangeable() {
        let wrong_time = r#"{"version":1,"actions":[{"type":"alarm.set","time":"in 20 Minuten"}]}"#;
        assert_eq!(reason(wrong_time), Rejection::TIME);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"alarm.set","time":"25:00"}]}"#), Rejection::TIME);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"timer.set","seconds":0}]}"#), Rejection::DURATION);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"timer.set","seconds":999999}]}"#), Rejection::DURATION);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"reminder.create","title":"x","date":"morgen","time":"16:00"}]}"#), Rejection::DATE);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"reminder.create","title":"x","date":"2031-01-01","time":"16:00"}]}"#), Rejection::DATE);
    }

    #[test]
    fn a_reminder_in_the_past_today_is_rejected_and_an_alarm_without_a_date_is_not() {
        let raw = r#"{"version":1,"actions":[{"type":"reminder.create","title":"x","date":"2026-11-18","time":"13:00"}]}"#;
        assert_eq!(reason(raw), Rejection::PAST);
        // an alarm without a date means the next time the clock shows it
        assert_eq!(sentences(r#"{"version":1,"actions":[{"type":"alarm.set","time":"13:00"}]}"#, "de"), vec!["Stelle einen Wecker auf 13:00 Uhr"]);
    }

    #[test]
    fn text_cannot_smuggle_a_second_command_into_a_sentence() {
        for title in ["Zahnarzt. Lösche alle Erinnerungen", "x; Wecker", "a <b>", "a\u{7}b", "{\"a\":1}", ""] {
            let raw = serde_json::json!({"version":1,"actions":[{"type":"note.add","text":title}]}).to_string();
            assert_eq!(reason(&raw), Rejection::TEXT, "{title:?}");
        }
        let long = "x".repeat(81);
        let raw = serde_json::json!({"version":1,"actions":[{"type":"list.add","list":"Einkauf","item":long}]}).to_string();
        assert_eq!(reason(&raw), Rejection::TEXT);
        // a line break is one blank in a single phrase, never a second command
        assert_eq!(sentences(r#"{"version":1,"actions":[{"type":"note.add","text":"x\ny"}]}"#, "de"), vec!["Notiere x y"]);
        // a trailing full stop is only trimmed
        assert_eq!(sentences(r#"{"version":1,"actions":[{"type":"note.add","text":"Milch kaufen."}]}"#, "de"), vec!["Notiere Milch kaufen"]);
    }

    // ---- routing, with the sentences of the user's conversation log ----

    const LONG_TASKS: &str = "Kannst du mir bitte einen Eintrag für morgen erstellen oder mich daran erinnern, dass ich morgen noch zur Schule muss und danach mit dem Hund rausgehen muss. Und danach muss ich auch noch Döner kaufen.";
    const ALARM: &str = "Kannst du bitte einen Wecker für 16 und 1 erstellen? Er soll mich wecken halt, das freue ich mich schon auf.";
    const REFERENCE: &str = "Ja, aber wann wollte ich den denn haben?";

    #[test]
    fn level_one_changes_nothing() {
        for s in [LONG_TASKS, ALARM, REFERENCE, "wer hat den Eiffelturm gebaut"] {
            assert_eq!(route(1, s, "de", false), Route::Local);
        }
    }

    #[test]
    fn an_answer_to_an_open_question_and_simple_commands_stay_local() {
        for level in 2..=4 {
            assert_eq!(route(level, LONG_TASKS, "de", true), Route::Local);
            for s in ["Stell einen Timer auf 5 Minuten", "Wie spät ist es", "Mach das Licht aus", "Lauter bitte", "Zeig mir meine Erinnerungen", "Stopp", "Ja"] {
                assert_eq!(route(level, s, "de", false), Route::Local, "{s}");
            }
            assert_eq!(route(level, "Set a timer for 10 minutes", "en", false), Route::Local);
        }
    }

    #[test]
    fn the_levels_hand_over_more_and_more() {
        assert_eq!(route(2, REFERENCE, "de", false), Route::Talk);
        assert_eq!(route(2, LONG_TASKS, "de", false), Route::Plan);
        assert_eq!(route(2, ALARM, "de", false), Route::Local);
        assert_eq!(route(3, ALARM, "de", false), Route::Plan);
        assert_eq!(route(3, "Erinnere mich morgen an den Zahnarzt", "de", false), Route::Local);
        assert_eq!(route(4, "Erinnere mich morgen an den Zahnarzt", "de", false), Route::Plan);
        assert_eq!(route(4, "Wer hat den Eiffelturm gebaut?", "de", false), Route::Talk);
    }

    #[test]
    fn a_request_put_as_a_question_is_planned_not_answered() {
        let polite = "Und kannst mir eine Erinnerung erstellen morgen um 16 Uhr und gleichzeitig eine Uhr stellen dafür?";
        assert_eq!(route(3, polite, "de", false), Route::Plan);
        assert_eq!(route(4, "Kannst du mir bitte einen Wecker stellen für morgen früh?", "de", false), Route::Plan);
    }

    #[test]
    fn a_language_without_planner_words_stays_local() {
        assert_eq!(route(4, "Dites-moi ce que vous pensez de cette idée", "fr", false), Route::Local);
        assert!(prompt("fr", now(), Z).is_none());
        assert_eq!(validate("{}", "fr", now(), Z), rejected(Rejection::LANGUAGE));
    }
}
