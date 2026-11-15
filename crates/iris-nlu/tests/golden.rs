//! The readers must give what the app's Kotlin gave (`app/src/test/.../NluGoldenDump.kt` wrote the reference
//! over the NLU corpus, the spec sentences and edge cases). Where the JVM's ASCII-only `\b` and `(?i)` made the
//! Kotlin reference differ from the phone (Android's ICU is Unicode, like Rust), the case is listed in
//! `tests/golden/device.tsv` with the phone's answer and why.

use flate2::read::GzDecoder;
use iris_nlu::clarify;
use iris_nlu::date::DateSlot;
use iris_nlu::fallback::{self, Plan, Prediction};
use iris_nlu::intent::RoutedIntent;
use iris_nlu::{anime, date, duration, language, normalize, place, router, slots, time};
use jiff::civil::Date;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;

fn lines(name: &str) -> Vec<Value> {
    let mut s = String::new();
    GzDecoder::new(std::fs::File::open(format!("{}/tests/golden/{name}.jsonl.gz", env!("CARGO_MANIFEST_DIR"))).unwrap()).read_to_string(&mut s).unwrap();
    s.lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn slot(s: &Option<DateSlot>) -> Value {
    match s {
        None => Value::Null,
        Some(s) => json!({"date": s.date.to_string(), "period": s.period, "hours": s.hours, "days": s.days}),
    }
}

fn routed(r: &Option<RoutedIntent>) -> Value {
    match r {
        None => Value::Null,
        Some(r) => json!({
            "kind": r.kind, "date": slot(&r.date), "place": r.place, "time": r.time.map(|t| t.to_string()),
            "duration": r.duration, "clarification": r.clarification, "advice": r.advice, "timer_name": r.timer_name,
        }),
    }
}

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

/// Every place where Rust and the Kotlin reference differ: (what, input, rust, kotlin).
fn text_differences() -> Vec<(String, String, Value, Value)> {
    let mut diffs = Vec::new();
    let mut check = |what: &str, text: &str, got: Value, want: &Value| {
        if &got != want {
            diffs.push((what.to_string(), text.to_string(), got, want.clone()));
        }
    };
    for o in lines("text") {
        let text = o["text"].as_str().unwrap();
        let n = normalize(text);
        check("normalized", text, json!(n), &o["normalized"]);
        let want_n = o["normalized"].as_str().unwrap();
        check("duration", text, json!(duration::extract(want_n)), &o["duration"]);
        let t = time::extract(want_n);
        check("time", text, json!({"time": t.time.map(|c| c.to_string()), "rest": t.rest, "invalid": t.invalid, "ambiguous": t.ambiguous}), &o["time"]);
        for (i, w) in o["dates"].as_array().unwrap().iter().enumerate() {
            let today = d(w["today"].as_str().unwrap());
            let e = date::extract(want_n, today);
            let te = time::extract(&e.rest);
            let p = place::resolve(&te.rest);
            check(&format!("date[{i}]"), text, json!({"today": w["today"], "slot": slot(&e.slot), "rest": e.rest, "invalid": e.invalid, "place": p.place, "place_unclear": p.needs_clarification}), w);
        }
        check("clean", text, json!(slots::clean(text)), &o["clean"]);
        check("call", text, json!(slots::call_target(text)), &o["call"]);
        check("destination", text, json!(slots::destination(text)), &o["destination"]);
        check("app", text, json!(slots::app_name(text)), &o["app"]);
        check("search", text, json!(slots::search_query(text)), &o["search"]);
        check("fact", text, json!(slots::fact(text)), &o["fact"]);
        check("task", text, json!(slots::task_title(text)), &o["task"]);
        check("music", text, json!(slots::music_query(text)), &o["music"]);
        check("tomorrow", text, json!(slots::wants_tomorrow(text)), &o["tomorrow"]);
        let today = d("2026-11-06");
        let r = anime::range(text, today);
        check("anime", text, json!({"everything": anime::wants_everything(text), "episode": anime::episode(text), "one_more": anime::is_one_more(text), "title": anime::title_guess(text),
            "range": {"from": r.from.to_string(), "to": r.to_exclusive.to_string()}}), &o["anime"]);
        check("question", text, json!(fallback::is_question(text)), &o["question"]);
        check("words", text, json!(fallback::word_count(text)), &o["words"]);
        check("router", text, json!({"de": routed(&router::route(text, "de", today)), "en": routed(&router::route(text, "en", today))}), &o["router"]);
        check("lang", text, json!({"de": language::detect(text, "de"), "en": language::detect(text, "en")}), &o["lang"]);
        check("polite", text, json!(language::polite(text)), &o["polite"]);
        for (label, want) in o["routed"].as_object().unwrap() {
            check(&format!("routed {label}"), text, routed(&slots::routed(label, text, today)), want);
        }
    }
    diffs
}

fn plan_json(p: &Plan) -> Value {
    match p {
        Plan::Run(l) => json!({"plan": "run", "label": l}),
        Plan::Clarify(a, b) => json!({"plan": "clarify", "first": a, "second": b}),
        Plan::Suggest(ls) => json!({"plan": "suggest", "labels": ls}),
        Plan::Knowledge => json!({"plan": "knowledge"}),
        Plan::Fragment => json!({"plan": "fragment"}),
        Plan::Unknown => json!({"plan": "unknown"}),
    }
}

fn fallback_differences() -> Vec<(String, String, Value, Value)> {
    let mut diffs = Vec::new();
    for o in lines("fallback") {
        let top: Vec<Prediction> = o["top"].as_array().unwrap().iter().map(|p| Prediction { label: p["label"].as_str().unwrap().into(), p: p["p"].as_f64().unwrap() as f32 }).collect();
        let text = o["text"].as_str().unwrap();
        let last = o["last"].as_str();
        let heads: BTreeMap<String, String> = o["heads"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        let plan = fallback::plan(&top, text, last);
        let mut add = |what: &str, got: Value, want: &Value| {
            if &got != want {
                diffs.push((what.to_string(), format!("{text} | {}", o["top"]), got, want.clone()));
            }
        };
        add("plan", plan_json(&plan), &o["plan"]);
        let mut messages = serde_json::Map::new();
        for de in [true, false] {
            for online in [true, false] {
                messages.insert(format!("{}_{online}", if de { "de" } else { "en" }), json!(fallback::message(&plan, de, online, last)));
            }
        }
        add("messages", Value::Object(messages), &o["messages"]);
        let mut offers = serde_json::Map::new();
        for de in [true, false] {
            let list: Vec<Value> = clarify::offers(&top, text, de, &heads, d("2026-11-06"))
                .into_iter()
                .map(|f| json!({"display": f.display, "label": f.label, "text": f.text, "heads": f.heads, "day": f.day.to_string()}))
                .collect();
            offers.insert(if de { "de" } else { "en" }.into(), json!(list));
        }
        add("offers", Value::Object(offers), &o["offers"]);
        add("ambiguous", json!(clarify::ambiguous(&top)), &o["ambiguous"]);
    }
    diffs
}

/// The cases where the phone (Unicode) answers differently from the JVM reference: "what\tinput" -> why.
fn device_cases() -> BTreeMap<(String, String), String> {
    let path = format!("{}/tests/golden/device.tsv", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.splitn(3, '\t').collect();
            ((f[0].to_string(), f[1].to_string()), f.get(2).unwrap_or(&"").to_string())
        })
        .collect()
}

#[test]
fn the_readers_give_what_the_app_gave() {
    let known = device_cases();
    let all = text_differences();
    let unexplained: Vec<_> = all.iter().filter(|(w, t, _, _)| !known.contains_key(&(w.clone(), t.clone()))).collect();
    for (w, t, got, want) in unexplained.iter().take(40) {
        eprintln!("{w} {t:?}\n  rust:   {got}\n  kotlin: {want}");
    }
    let used: usize = all.iter().filter(|(w, t, _, _)| known.contains_key(&(w.clone(), t.clone()))).count();
    assert_eq!(used, known.len(), "device.tsv lists cases that no longer differ");
    assert!(unexplained.is_empty(), "{} differences", unexplained.len());
}

#[test]
fn the_fallback_gives_what_the_app_gave() {
    let diffs = fallback_differences();
    for (w, t, got, want) in diffs.iter().take(20) {
        eprintln!("{w} {t}\n  rust:   {got}\n  kotlin: {want}");
    }
    assert!(diffs.is_empty(), "{} differences", diffs.len());
}
