//! The time and the date, said in the language's own words.

use iris_core::{Cx, Reply, Skill};

pub struct Clock;

impl Skill for Clock {
    fn manifest(&self) -> &'static str {
        include_str!("../data/clock/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/clock/de.toml")),
            "en" => Some(include_str!("../data/clock/en.toml")),
            _ => None,
        }
    }

    fn act(&self, action: &str, _args: &iris_core::actions::Args, cx: &mut Cx) -> Option<Reply> {
        self.handle(action.strip_prefix("clock.")?, cx)
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        let now = &cx.now;
        match intent {
            "time" => {
                let time = cx.lang.format.clock.replace("{h}", &now.hour().to_string()).replace("{mm}", &format!("{:02}", now.minute()));
                Some(Reply::done(cx.msg("time", &[("time", &time)])))
            }
            "date" => {
                let weekday = cx.lang.format.weekday_names.get(now.date().weekday().to_monday_zero_offset() as usize).cloned().unwrap_or_default();
                let month = cx.lang.format.month_names.get(now.month() as usize - 1).cloned().unwrap_or_default();
                Some(Reply::done(cx.msg("date", &[("weekday", &weekday), ("day", &now.day().to_string()), ("month", &month), ("year", &now.year().to_string())])))
            }
            _ => None,
        }
    }
}
