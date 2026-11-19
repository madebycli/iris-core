//! A stopwatch: start, stop, reset, read. It keeps running while the app is closed, because its
//! state is two values the host keeps across restarts.

use iris_core::{Cx, Reply, Skill};

pub struct Stopwatch;

const STARTED: &str = "stopwatch.started";
const ELAPSED: &str = "stopwatch.elapsed";

impl Skill for Stopwatch {
    fn manifest(&self) -> &'static str {
        include_str!("../data/stopwatch/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/stopwatch/de.toml")),
            "en" => Some(include_str!("../data/stopwatch/en.toml")),
            _ => None,
        }
    }

    fn act(&self, action: &str, _args: &iris_core::actions::Args, cx: &mut Cx) -> Option<Reply> {
        let intent = match action {
            "stopwatch.start" => "start",
            "stopwatch.stop" => "stop",
            "stopwatch.reset" => "reset",
            "stopwatch.show" => "read",
            _ => return None,
        };
        self.handle(intent, cx)
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        let now = cx.now_ms();
        // a start in the future means the clock was set back: the stopwatch cannot know how long it ran
        let mut started = cx.host.setting(STARTED).and_then(|v| v.parse::<i64>().ok()).filter(|t| *t <= now);
        let mut elapsed = cx.host.setting(ELAPSED).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0).max(0);
        match intent {
            "start" => {
                if started.is_none() {
                    started = Some(now);
                }
            }
            "stop" => {
                if let Some(at) = started.take() {
                    elapsed += now - at;
                }
            }
            "reset" => {
                started = None;
                elapsed = 0;
            }
            "read" => {}
            _ => return None,
        }
        cx.host.keep(STARTED, started.map(|t| t.to_string()).as_deref());
        cx.host.keep(ELAPSED, Some(&elapsed.to_string()));
        let secs = (elapsed + started.map(|t| now - t).unwrap_or(0)) / 1000;
        let shown = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
        let key = if started.is_some() { "running" } else { "stopped" };
        Some(Reply::done(cx.msg(key, &[("time", &shown)])))
    }
}
