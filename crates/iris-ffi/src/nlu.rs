//! The language readers of `iris-nlu` for the app: plain functions, structured values as JSON.

use iris_nlu::clarify;
use iris_nlu::date::DateSlot;
use iris_nlu::fallback::{self, Plan, Prediction};
use iris_nlu::intent::RoutedIntent;
use iris_nlu::{anime, date, duration, language, place, router, slots, time};
use jiff::civil::Date;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn day(s: &str) -> Date {
    s.parse().unwrap_or_else(|_| jiff::Zoned::now().date())
}

fn slot_json(s: &Option<DateSlot>) -> Value {
    match s {
        None => Value::Null,
        Some(s) => json!({"date": s.date.to_string(), "period": s.period, "hours": s.hours, "days": s.days}),
    }
}

fn routed_json(r: &Option<RoutedIntent>) -> Option<String> {
    r.as_ref().map(|r| {
        json!({
            "kind": r.kind, "date": slot_json(&r.date), "place": r.place, "time": r.time.map(|t| t.to_string()),
            "duration": r.duration, "clarification": r.clarification, "advice": r.advice, "timer_name": r.timer_name,
        })
        .to_string()
    })
}

fn predictions(top: &str) -> Vec<Prediction> {
    #[derive(serde::Deserialize)]
    struct P {
        label: String,
        p: f32,
    }
    serde_json::from_str::<Vec<P>>(top).unwrap_or_default().into_iter().map(|p| Prediction { label: p.label, p: p.p }).collect()
}

fn plan_json(p: &Plan) -> String {
    match p {
        Plan::Run(l) => json!({"plan": "run", "label": l}),
        Plan::Clarify(a, b) => json!({"plan": "clarify", "first": a, "second": b}),
        Plan::Suggest(ls) => json!({"plan": "suggest", "labels": ls}),
        Plan::Knowledge => json!({"plan": "knowledge"}),
        Plan::Fragment => json!({"plan": "fragment"}),
        Plan::Unknown => json!({"plan": "unknown"}),
    }
    .to_string()
}

fn plan_of(json: &str) -> Plan {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    match v["plan"].as_str().unwrap_or("") {
        "run" => Plan::Run(s("label")),
        "clarify" => Plan::Clarify(s("first"), s("second")),
        "suggest" => Plan::Suggest(v["labels"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()),
        "knowledge" => Plan::Knowledge,
        "fragment" => Plan::Fragment,
        _ => Plan::Unknown,
    }
}

/// The sentence as the readers see it: lower case, number words as digits.
#[uniffi::export]
pub fn nlu_normalize(text: String) -> String {
    iris_nlu::normalize(&text)
}

/// JSON `{"slot": {"date","period","hours","days"} | null, "rest", "invalid"}`.
#[uniffi::export]
pub fn nlu_date(text: String, today: String) -> String {
    let e = date::extract(&text, day(&today));
    json!({"slot": slot_json(&e.slot), "rest": e.rest, "invalid": e.invalid}).to_string()
}

/// JSON `{"time": "HH:MM" | null, "rest", "invalid", "ambiguous"}`.
#[uniffi::export]
pub fn nlu_time(text: String) -> String {
    let t = time::extract(&text);
    json!({"time": t.time.map(|c| c.to_string()), "rest": t.rest, "invalid": t.invalid, "ambiguous": t.ambiguous}).to_string()
}

/// Seconds.
#[uniffi::export]
pub fn nlu_duration(text: String) -> Option<i64> {
    duration::extract(&text)
}

/// JSON `{"place": string | null, "needs_clarification"}`.
#[uniffi::export]
pub fn nlu_place(text: String) -> String {
    let p = place::resolve(&text);
    json!({"place": p.place, "needs_clarification": p.needs_clarification}).to_string()
}

#[uniffi::export]
pub fn nlu_clean(text: String) -> String {
    slots::clean(&text)
}

/// A typed request for a label the network named (JSON of a routed intent), or none for labels read elsewhere.
#[uniffi::export]
pub fn nlu_routed(label: String, text: String, today: String) -> Option<String> {
    routed_json(&slots::routed(&label, &text, day(&today)))
}

/// The rule reading kept for the guard of the online AI.
#[uniffi::export]
pub fn nlu_route(text: String, language: String, today: String) -> Option<String> {
    routed_json(&router::route(&text, &language, day(&today)))
}

#[uniffi::export]
pub fn nlu_call_target(text: String) -> Option<String> {
    slots::call_target(&text)
}

#[uniffi::export]
pub fn nlu_destination(text: String) -> Option<String> {
    slots::destination(&text)
}

#[uniffi::export]
pub fn nlu_app_name(text: String) -> String {
    slots::app_name(&text)
}

#[uniffi::export]
pub fn nlu_search_query(text: String) -> String {
    slots::search_query(&text)
}

#[uniffi::export]
pub fn nlu_fact(text: String) -> Option<String> {
    slots::fact(&text)
}

#[uniffi::export]
pub fn nlu_task_title(text: String) -> Option<String> {
    slots::task_title(&text)
}

#[uniffi::export]
pub fn nlu_music_query(text: String) -> Option<String> {
    slots::music_query(&text)
}

#[uniffi::export]
pub fn nlu_wants_tomorrow(text: String) -> bool {
    slots::wants_tomorrow(&text)
}

#[uniffi::export]
pub fn nlu_anime_everything(text: String) -> bool {
    anime::wants_everything(&text)
}

/// JSON `{"from", "to"}` (to exclusive).
#[uniffi::export]
pub fn nlu_anime_range(text: String, today: String) -> String {
    let r = anime::range(&text, day(&today));
    json!({"from": r.from.to_string(), "to": r.to_exclusive.to_string()}).to_string()
}

#[uniffi::export]
pub fn nlu_anime_episode(text: String) -> Option<i64> {
    anime::episode(&text)
}

#[uniffi::export]
pub fn nlu_anime_one_more(text: String) -> bool {
    anime::is_one_more(&text)
}

#[uniffi::export]
pub fn nlu_anime_title(text: String) -> Option<String> {
    anime::title_guess(&text)
}

#[uniffi::export]
pub fn nlu_is_question(text: String) -> bool {
    fallback::is_question(&text)
}

#[uniffi::export]
pub fn nlu_word_count(text: String) -> u32 {
    fallback::word_count(&text) as u32
}

/// What to do when the network is not sure: `top` is JSON `[{"label","p"}]`; returns JSON of the plan.
#[uniffi::export]
pub fn nlu_fallback_plan(top: String, text: String, last: Option<String>) -> String {
    plan_json(&fallback::plan(&predictions(&top), &text, last.as_deref()))
}

#[uniffi::export]
pub fn nlu_fallback_message(plan: String, de: bool, online: bool, last_topic: Option<String>) -> String {
    fallback::message(&plan_of(&plan), de, online, last_topic.as_deref())
}

#[uniffi::export]
pub fn nlu_function_name(label: String, de: bool) -> String {
    fallback::name(&label, de)
}

/// Readings to tap: JSON `[{"display","label","text","heads","day"}]`.
#[uniffi::export]
pub fn nlu_offers(top: String, text: String, de: bool, heads: String, today: String) -> String {
    let heads: BTreeMap<String, String> = serde_json::from_str(&heads).unwrap_or_default();
    let list: Vec<Value> = clarify::offers(&predictions(&top), &text, de, &heads, day(&today))
        .into_iter()
        .map(|o| json!({"display": o.display, "label": o.label, "text": o.text, "heads": o.heads, "day": o.day.to_string()}))
        .collect();
    Value::Array(list).to_string()
}

#[uniffi::export]
pub fn nlu_ambiguous(top: String) -> bool {
    clarify::ambiguous(&predictions(&top))
}

#[uniffi::export]
pub fn nlu_offer_title(label: String, de: bool) -> Option<String> {
    clarify::title(&label, de).map(str::to_string)
}

#[uniffi::export]
pub fn nlu_spoken_language(text: String, default: String) -> String {
    language::detect(&text, &default)
}

#[uniffi::export]
pub fn nlu_polite(text: String) -> String {
    language::polite(&text)
}
