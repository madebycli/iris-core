//! Who handles a sentence, and the contract in which the assistant AI may propose actions.
//!
//! The four levels of "AI use" say how readily a sentence goes to the online assistant instead of staying with
//! the local skills ([`route`]). When a sentence is handed over for planning, the core picks the tools (actions of
//! the registry, see [`crate::actions`]) the request needs and the host can carry out, and tells the assistant
//! ([`prompt`]). The assistant answers in a compact JSON contract. Nothing of that reply is trusted: [`validate`]
//! checks the whole proposal against the registry's schemas and the turn's clock before any side effect. A valid
//! action reaches its executor as an action id with checked values, never as a sentence to read again. A
//! clarification is a dialogue turn: it never comes with actions.
//!
//! No word of a spoken language is written here. Words, questions and the description of every action are data in
//! `core/lang/plan/<code>.toml`.

use crate::actions::{self, Action, Reject};
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The version of the action contract. A proposal with another version is rejected.
pub const CONTRACT_VERSION: u32 = 2;
/// Most actions one proposal may hold.
pub const MAX_ACTIONS: usize = 8;
const MAX_REPLY_BYTES: usize = 8 * 1024;
const SLOTS: [&str; 5] = ["time", "date", "duration", "title", "list"];
/// The slot of a question the assistant words itself ("which list do you mean?"): shown, never executed.
const OTHER: &str = "other";
const MAX_QUESTION: usize = 160;

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
    /// Valid actions, in order, each with its checked values.
    Actions { actions: Vec<Planned> },
    /// One question back to the user. No action comes with it.
    Ask { slot: String, question: String },
    /// The message is no task for these actions: the assistant answers it in text instead.
    NotAction,
    /// Not usable: nothing is carried out. `reason` is a stable code (see [`Rejection`]).
    Rejected { reason: String },
}

/// One validated action: its id (`timer.set`) and the checked values, with the moment as `at_ms` where it has one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Planned {
    pub id: String,
    pub args: Map<String, Value>,
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
    ask: HashMap<String, String>,
    prompt: PromptSource,
    actions: HashMap<String, String>,
    tools: HashMap<String, String>,
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
    weekdays: Vec<String>,
}

#[derive(Deserialize)]
struct PromptSource {
    head: String,
    actions_title: String,
    rules: String,
}

struct Pack {
    reference: Regex,
    sequence: Regex,
    command: Regex,
    question: Regex,
    simple: Vec<Regex>,
    format: Format,
    ask: HashMap<String, String>,
    prompt: PromptSource,
    actions: HashMap<String, String>,
    /// Which words in a request call for the tools of a family.
    tools: HashMap<String, Regex>,
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
        ask: p.ask,
        prompt: p.prompt,
        actions: p.actions,
        tools: p.tools.iter().map(|(family, s)| (family.clone(), re(s))).collect(),
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

/// Where a sentence goes at AI-use `level` (1 sparing, 2 balanced, 3 smart, 4 AI first). `direct`: the local network
/// is sure the sentence is one plain command it can carry out itself (`Engine::direct`).
///
/// `open_question`: a question of the core is open and this is its answer; that always stays local. At level 1
/// nothing is decided here: the older routing (a sentence nothing understood, knowledge questions, follow-ups)
/// applies. Simple commands and sentences of two words or fewer stay local at every level.
pub fn route(level: u8, text: &str, lang: &str, open_question: bool, direct: bool) -> Route {
    if level <= 1 || open_question {
        return Route::Local;
    }
    let Some(p) = pack(lang) else { return Route::Local };
    let words = text.split_whitespace().count();
    let multi = (p.sequence.is_match(text) && words >= 9) || words >= 25;
    // a simple command is local; the same words inside a larger order ("Taschenlampe an und danach ...") are one part of a plan
    if words <= 2 || (!multi && p.simple.iter().any(|r| r.is_match(text))) {
        return Route::Local;
    }
    let reference = p.reference.is_match(text);
    // A request the local network is sure of and that has one part ("Taschenlampe an", "nächstes Lied") needs no
    // interpretation at any level; the assistant may still propose the same action inside a larger plan. A question
    // about earlier turns is no such command, whatever label the network gives its words.
    if direct && !multi && !reference {
        return Route::Local;
    }
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

/// What the assistant is told for one request, and which actions it may use in its reply.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prompt {
    pub text: String,
    /// The ids of the actions on offer: the ones this request needs and the host can carry out.
    pub tools: Vec<String>,
}

/// The tools one request needs: every family whose words appear anywhere in `request` (a mixed order such as
/// "Termin im Kalender und Erinnerung" gets both), out of the `available` actions. A request that names no family
/// gets all available actions, so the assistant is never left without the one it needs.
pub fn select<'a>(lang: &str, request: &str, available: &[&'a Action]) -> Vec<&'a Action> {
    let Some(p) = pack(lang) else { return Vec::new() };
    let wanted: Vec<&str> = p.tools.iter().filter(|(_, re)| re.is_match(request)).map(|(family, _)| family.as_str()).collect();
    if wanted.is_empty() {
        return available.to_vec();
    }
    available.iter().copied().filter(|a| wanted.contains(&a.family())).collect()
}

/// The contract for the tools of `request` and the turn's clock (today, the weekday, the time zone), so a relative
/// word like "tomorrow" is turned into a fixed date against the same clock the core validates with. `None` for a
/// language without planner words or when no action is available.
pub fn prompt(lang: &str, now_ms: i64, zone: &str, request: &str, available: &[&Action]) -> Option<Prompt> {
    let p = pack(lang)?;
    let tools = select(lang, request, available);
    if tools.is_empty() {
        return None;
    }
    let now = turn_clock(now_ms, zone)?;
    let weekday = p.format.weekdays.get(now.weekday().to_monday_zero_offset() as usize)?;
    let lines: Vec<&str> = tools.iter().filter_map(|a| p.actions.get(&a.id).map(String::as_str)).collect();
    let text = format!("{}\n{}\n{}\n{}", p.prompt.head, p.prompt.actions_title, lines.join("\n"), p.prompt.rules);
    let text = text
        .replace("{weekday}", weekday)
        .replace("{today}", &now.date().to_string())
        .replace("{zone}", zone)
        .replace("{time}", &now.strftime("%H:%M").to_string());
    Some(Prompt { text, tools: tools.iter().map(|a| a.id.clone()).collect() })
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
    actions: Vec<Value>,
    #[serde(default)]
    ask: Option<RawAsk>,
    #[serde(default)]
    not_action: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsk {
    slot: String,
    /// Only with the slot `other`: the question in the user's language.
    #[serde(default)]
    question: Option<String>,
}

/// Checks the whole proposal in `raw` (the assistant's reply, possibly wrapped in a code fence or a sentence)
/// against the turn's clock `now_ms` in time zone `zone` and the `tools` that were on offer. Either everything is
/// valid or nothing is carried out.
pub fn validate(raw: &str, lang: &str, now_ms: i64, zone: &str, tools: &[String]) -> Outcome {
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
        // a question the assistant words itself: plain text for the user to read, never a command
        if ask.slot == OTHER {
            return match ask.question.as_deref().map(str::trim) {
                Some(q) if !q.is_empty() && q.chars().count() <= MAX_QUESTION && !q.chars().any(char::is_control) => Outcome::Ask { slot: ask.slot, question: q.to_string() },
                _ => rejected(Rejection::SLOT),
            };
        }
        if ask.question.is_some() {
            return rejected(Rejection::SLOT);
        }
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
    for value in &proposal.actions {
        match actions::registry().check(value, tools, &now) {
            Ok(checked) => planned.push(Planned { id: checked.id, args: checked.args }),
            Err(reject) => return rejected(reason_of(reject)),
        }
    }
    Outcome::Actions { actions: planned }
}

fn reason_of(reject: Reject) -> &'static str {
    match reject {
        Reject::UnknownAction => Rejection::UNKNOWN_ACTION,
        other => other.code(),
    }
}

/// The JSON object in `raw`: from the first `{` to the last `}`.
fn extract_json(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Wednesday 2026-11-18, 14:00 in Berlin: the turn's clock in every case.
    fn now() -> i64 {
        "2026-11-18T14:00[Europe/Berlin]".parse::<Zoned>().unwrap().timestamp().as_millisecond()
    }
    const Z: &str = "Europe/Berlin";

    fn every_tool() -> Vec<String> {
        actions::registry().all().iter().map(|a| a.id.clone()).collect()
    }

    fn planned(raw: &str) -> Vec<Planned> {
        match validate(raw, "de", now(), Z, &every_tool()) {
            Outcome::Actions { actions } => actions,
            other => panic!("expected actions, got {other:?}"),
        }
    }

    fn reason(raw: &str) -> String {
        match validate(raw, "de", now(), Z, &every_tool()) {
            Outcome::Rejected { reason } => reason,
            other => panic!("expected a rejection, got {other:?}"),
        }
    }

    #[test]
    fn the_language_data_is_valid_and_describes_every_action() {
        for lang in languages() {
            let p = pack(lang).unwrap_or_else(|| panic!("{lang}"));
            for a in actions::registry().all() {
                assert!(p.actions.contains_key(&a.id), "{lang}: no description of {}", a.id);
                assert!(p.tools.contains_key(a.family()), "{lang}: no words for the family {}", a.family());
            }
            let all: Vec<&Action> = actions::registry().all().iter().collect();
            let text = prompt(lang, now(), Z, "", &all).unwrap().text;
            assert!(text.contains("2026-11-18"));
        }
    }

    #[test]
    fn the_shoe_store_evening_from_the_log_becomes_three_checked_actions() {
        let raw = r#"{"version":2,"actions":[
            {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
            {"type":"alarm.set","time":"15:40","date":"2026-11-19"},
            {"type":"list.add","list":"Einkauf","items":["Schuhe"]}]}"#;
        let got = planned(raw);
        assert_eq!(got.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["reminder.create", "alarm.set", "list.add"]);
        let at = |s: &str| s.parse::<Zoned>().unwrap().timestamp().as_millisecond();
        assert_eq!(got[0].args["at_ms"], json!(at("2026-11-19T16:00[Europe/Berlin]")));
        assert_eq!(got[1].args["at_ms"], json!(at("2026-11-19T15:40[Europe/Berlin]")));
        assert_eq!(got[2].args["items"], json!(["Schuhe"]));
    }

    #[test]
    fn a_proposal_in_a_code_fence_or_a_sentence_is_read() {
        let fenced = "Hier ist der Plan:\n```json\n{\"version\":2,\"actions\":[{\"type\":\"note.add\",\"text\":\"Milch kaufen\"}]}\n```";
        assert_eq!(planned(fenced)[0].args["text"], json!("Milch kaufen"));
    }

    #[test]
    fn a_clarification_is_a_turn_of_its_own() {
        match validate(r#"{"version":2,"ask":{"slot":"time"}}"#, "de", now(), Z, &every_tool()) {
            Outcome::Ask { slot, question } => assert_eq!((slot.as_str(), question.as_str()), ("time", "Um wie viel Uhr?")),
            other => panic!("{other:?}"),
        }
        let mixed = r#"{"version":2,"ask":{"slot":"time"},"actions":[{"type":"note.add","text":"x"}]}"#;
        assert_eq!(reason(mixed), Rejection::MIXED);
        assert_eq!(reason(r#"{"version":2,"ask":{"slot":"password"}}"#), Rejection::SLOT);
    }

    #[test]
    fn a_question_the_assistant_words_itself_is_plain_text_and_never_comes_with_actions() {
        match validate(r#"{"version":2,"ask":{"slot":"other","question":"Welche Liste meinst du: Einkauf oder Essen?"}}"#, "de", now(), Z, &every_tool()) {
            Outcome::Ask { slot, question } => assert_eq!((slot.as_str(), question.as_str()), ("other", "Welche Liste meinst du: Einkauf oder Essen?")),
            other => panic!("{other:?}"),
        }
        assert_eq!(reason(r#"{"version":2,"ask":{"slot":"other"}}"#), Rejection::SLOT);
        assert_eq!(reason(r#"{"version":2,"ask":{"slot":"other","question":""}}"#), Rejection::SLOT);
        assert_eq!(reason(r#"{"version":2,"ask":{"slot":"time","question":"Wann?"}}"#), Rejection::SLOT);
        let long = format!(r#"{{"version":2,"ask":{{"slot":"other","question":"{}"}}}}"#, "x".repeat(161));
        assert_eq!(reason(&long), Rejection::SLOT);
        assert_eq!(reason(r#"{"version":2,"ask":{"slot":"other","question":"a\u0007b"}}"#), Rejection::SLOT);
        let mixed = r#"{"version":2,"ask":{"slot":"other","question":"Was?"},"actions":[{"type":"note.add","text":"x"}]}"#;
        assert_eq!(reason(mixed), Rejection::MIXED);
    }

    #[test]
    fn not_a_task_is_passed_back() {
        assert_eq!(validate(r#"{"version":2,"not_action":true}"#, "de", now(), Z, &every_tool()), Outcome::NotAction);
        assert_eq!(reason(r#"{"version":2,"not_action":true,"actions":[{"type":"note.add","text":"x"}]}"#), Rejection::MIXED);
    }

    #[test]
    fn nothing_is_carried_out_unless_everything_is_valid() {
        let one_bad = r#"{"version":2,"actions":[{"type":"note.add","text":"Milch"},{"type":"reminder.create","title":"Oma","date":"2026-11-17","time":"09:00"}]}"#;
        assert_eq!(reason(one_bad), "past");
        assert_eq!(reason(r#"{"version":2,"actions":[{"type":"phone.format","all":true}]}"#), Rejection::UNKNOWN_ACTION);
        assert_eq!(reason(r#"{"version":1,"actions":[{"type":"note.add","text":"x"}]}"#), Rejection::VERSION);
        assert_eq!(reason(r#"{"version":2,"actions":[]}"#), Rejection::EMPTY);
        assert_eq!(reason("kein json"), Rejection::MALFORMED);
        let many = format!(r#"{{"version":2,"actions":[{}]}}"#, [r#"{"type":"note.add","text":"x"}"#; 9].join(","));
        assert_eq!(reason(&many), Rejection::TOO_MANY);
    }

    #[test]
    fn unknown_fields_are_not_ignored() {
        assert_eq!(reason(r#"{"version":2,"actions":[{"type":"note.add","text":"x","run":"rm"}]}"#), Rejection::MALFORMED);
        assert_eq!(reason(r#"{"version":2,"actions":[{"type":"note.add","text":"x"}],"extra":1}"#), Rejection::MALFORMED);
    }

    #[test]
    fn a_language_without_planner_words_stays_local() {
        assert_eq!(route(4, "Dites-moi ce que vous pensez de cette idée", "fr", false, false), Route::Local);
        let all: Vec<&Action> = actions::registry().all().iter().collect();
        assert!(prompt("fr", now(), Z, "", &all).is_none());
        assert_eq!(validate("{}", "fr", now(), Z, &every_tool()), rejected(Rejection::LANGUAGE));
    }

    // ---- the tools of a request ----

    fn names(lang: &str, request: &str, have: &[&str]) -> Vec<String> {
        let all: Vec<&Action> = actions::registry().all().iter().filter(|a| have.contains(&a.family())).collect();
        let mut ids: Vec<String> = select(lang, request, &all).iter().map(|a| a.id.clone()).collect();
        ids.sort();
        ids
    }

    #[test]
    fn a_mixed_order_gets_the_tools_of_every_part() {
        let have = ["calendar", "reminder", "timer", "flashlight", "list"];
        let got = names("de", "Trag morgen um 15 Uhr einen Termin in den Kalender ein und erinnere mich eine Stunde vorher", &have);
        assert!(got.contains(&"calendar.event".to_string()) && got.contains(&"reminder.create".to_string()), "{got:?}");
        assert!(!got.contains(&"timer.set".to_string()) && !got.contains(&"flashlight.set".to_string()), "{got:?}");
        let got = names("de", "Mach die Taschenlampe an und stell einen Timer auf zehn Minuten", &have);
        assert!(got.contains(&"flashlight.set".to_string()) && got.contains(&"timer.set".to_string()) && !got.contains(&"calendar.event".to_string()), "{got:?}");
        let got = names("en", "Turn on the flashlight and add milk to the shopping list", &have);
        assert!(got.contains(&"flashlight.set".to_string()) && got.contains(&"list.add".to_string()), "{got:?}");
    }

    #[test]
    fn a_request_that_names_no_family_gets_everything_available() {
        let have = ["timer", "note"];
        assert_eq!(names("de", "Mach das bitte", &have).len(), actions::registry().all().iter().filter(|a| have.contains(&a.family())).count());
    }

    #[test]
    fn the_prompt_names_only_what_the_host_can_do_and_the_request_needs() {
        let avail: Vec<&Action> = actions::registry().all().iter().filter(|a| a.family() != "alarm").collect();
        let p = prompt("de", now(), Z, "Stell einen Timer und notiere Milch", &avail).unwrap();
        assert!(p.text.contains("timer.set") && p.text.contains("note.add") && !p.text.contains("alarm.set") && !p.text.contains("flashlight.set"));
        assert!(p.tools.contains(&"timer.set".to_string()) && !p.tools.contains(&"list.add".to_string()));
        assert!(prompt("de", now(), Z, "x", &[]).is_none());
    }

    #[test]
    fn an_action_that_was_not_offered_rejects_the_whole_proposal() {
        let tools = vec!["note.add".to_string()];
        let raw = r#"{"version":2,"actions":[{"type":"note.add","text":"Milch"},{"type":"alarm.set","time":"15:40","date":"2026-11-19"}]}"#;
        assert_eq!(validate(raw, "de", now(), Z, &tools), rejected("unsupported"));
    }

    // ---- routing, with the sentences of the user's conversation log ----

    const LONG_TASKS: &str = "Kannst du mir bitte einen Eintrag für morgen erstellen oder mich daran erinnern, dass ich morgen noch zur Schule muss und danach mit dem Hund rausgehen muss. Und danach muss ich auch noch Döner kaufen.";
    const ALARM: &str = "Kannst du bitte einen Wecker für 16 und 1 erstellen? Er soll mich wecken halt, das freue ich mich schon auf.";
    const REFERENCE: &str = "Ja, aber wann wollte ich den denn haben?";

    #[test]
    fn level_one_changes_nothing() {
        for s in [LONG_TASKS, ALARM, REFERENCE, "wer hat den Eiffelturm gebaut"] {
            assert_eq!(route(1, s, "de", false, false), Route::Local);
        }
    }

    #[test]
    fn an_answer_to_an_open_question_and_simple_commands_stay_local() {
        for level in 2..=4 {
            assert_eq!(route(level, LONG_TASKS, "de", true, false), Route::Local);
            for s in ["Stell einen Timer auf 5 Minuten", "Wie spät ist es", "Mach das Licht aus", "Lauter bitte", "Zeig mir meine Erinnerungen", "Stopp", "Ja"] {
                assert_eq!(route(level, s, "de", false, false), Route::Local, "{s}");
            }
            assert_eq!(route(level, "Set a timer for 10 minutes", "en", false, false), Route::Local);
        }
    }

    #[test]
    fn the_levels_hand_over_more_and_more() {
        assert_eq!(route(2, REFERENCE, "de", false, false), Route::Talk);
        assert_eq!(route(2, LONG_TASKS, "de", false, false), Route::Plan);
        assert_eq!(route(2, ALARM, "de", false, false), Route::Local);
        assert_eq!(route(3, ALARM, "de", false, false), Route::Plan);
        assert_eq!(route(3, "Erinnere mich morgen an den Zahnarzt", "de", false, false), Route::Local);
        assert_eq!(route(4, "Erinnere mich morgen an den Zahnarzt", "de", false, false), Route::Plan);
        assert_eq!(route(4, "Wer hat den Eiffelturm gebaut?", "de", false, false), Route::Talk);
    }

    #[test]
    fn a_request_put_as_a_question_is_planned_not_answered() {
        let polite = "Und kannst mir eine Erinnerung erstellen morgen um 16 Uhr und gleichzeitig eine Uhr stellen dafür?";
        assert_eq!(route(3, polite, "de", false, false), Route::Plan);
        assert_eq!(route(4, "Kannst du mir bitte einen Wecker stellen für morgen früh?", "de", false, false), Route::Plan);
    }

    #[test]
    fn a_plain_command_the_local_network_is_sure_of_stays_local_at_every_level() {
        for level in 2..=4 {
            assert_eq!(route(level, "Mach bitte mal die Taschenlampe an für mich", "de", false, true), Route::Local, "level {level}");
        }
        // inside a larger order the assistant is asked, and may propose the same action as a step
        // a question about earlier turns stays a question, even if the network reads a command into its words
        assert_eq!(route(2, "Ja, aber wann wollte ich den denn haben?", "de", false, true), Route::Talk);
        let mixed = "Mach die Taschenlampe an und danach stelle mir einen Timer auf zehn Minuten für die Nudeln bitte";
        assert_eq!(route(4, mixed, "de", false, true), Route::Plan);
    }
}
