//! What the network is told about the conversation so far, as one short sentence.
//! Must give exactly the text of the Kotlin `BrainContext.compose` (the golden test checks it).

#[derive(Debug, Clone, Default)]
pub struct Context {
    pub last: String,
    pub last2: String,
    pub shown: String,
    pub titles: Vec<String>,
    pub focus: i32,
    pub has: Vec<String>,
    pub age: i32,
    pub expect: String,
    pub slot: String,
}

const NICE: &[(&str, &str)] = &[
    ("school_next_lesson", "next lesson"),
    ("school_room", "lesson room"),
    ("school_timetable", "timetable"),
    ("school_homework", "homework"),
    ("school_exams", "exams"),
    ("school_cancelled", "cancelled lessons"),
    ("school_messages", "school messages"),
    ("weather", "weather"),
    ("calendar_query", "calendar"),
    ("calendar_create", "new appointment"),
    ("day_overview", "day overview"),
    ("anime_airing", "anime calendar"),
    ("anime_next", "next anime"),
    ("anime_behind", "anime behind"),
    ("flashlight_on", "flashlight on"),
    ("flashlight_off", "flashlight off"),
    ("music_play", "music playing"),
    ("music_pause", "music paused"),
    ("alarm_set", "alarm set"),
    ("timer_set", "timer set"),
    ("call", "call"),
    ("navigation", "navigation"),
    ("lyrics", "lyrics"),
    ("song_recognize", "song recognised"),
    ("tasks_list", "task list"),
    ("tasks_create", "new task"),
    ("joke", "joke"),
    ("time", "time"),
    ("battery", "battery"),
    ("search_web", "search"),
    ("open_app", "open app"),
    ("smalltalk_howareyou", "small talk"),
];

fn nice(act: &str) -> String {
    NICE.iter().find(|(a, _)| *a == act).map(|(_, n)| n.to_string()).unwrap_or_else(|| act.replace('_', " "))
}

impl Context {
    pub fn empty() -> Self {
        Self { focus: -1, ..Default::default() }
    }

    fn is_empty(&self) -> bool {
        self.last.is_empty() && self.shown.is_empty() && self.expect.is_empty()
    }

    /// Reads a golden-file context: `k=v;k=v` with `|` inside lists.
    pub fn parse(line: &str) -> Self {
        let mut c = Self::empty();
        for part in line.split(';').filter(|p| p.contains('=')) {
            let (k, v) = part.split_once('=').unwrap();
            let list = || v.split('|').filter(|x| !x.is_empty()).map(str::to_string).collect::<Vec<_>>();
            match k {
                "last" => c.last = v.into(),
                "last2" => c.last2 = v.into(),
                "shown" => c.shown = v.into(),
                "titles" => c.titles = list(),
                "focus" => c.focus = v.parse().unwrap_or(-1),
                "has" => c.has = list(),
                "age" => c.age = v.parse().unwrap_or(0),
                "expect" => c.expect = v.into(),
                "slot" => c.slot = v.into(),
                _ => {}
            }
        }
        c
    }

    pub fn compose(&self) -> String {
        if self.is_empty() && self.has.is_empty() {
            return "nothing yet".into();
        }
        let mut parts: Vec<String> = Vec::new();
        if !self.last.is_empty() {
            parts.push(format!("last: {}", nice(&self.last)));
        }
        if !self.last2.is_empty() {
            parts.push(format!("before: {}", nice(&self.last2)));
        }
        if !self.shown.is_empty() {
            let mut s = format!("screen: {} list", self.shown);
            if !self.titles.is_empty() {
                s += &format!(": {}", self.titles.iter().take(8).cloned().collect::<Vec<_>>().join(", "));
            }
            if self.focus >= 0 {
                s += &format!("; focus {}", self.focus + 1);
            }
            parts.push(s);
        }
        if !self.expect.is_empty() {
            parts.push(if self.slot.is_empty() { format!("waiting: {}", self.expect) } else { format!("waiting: {} {}", self.expect, self.slot) });
        }
        if !self.has.is_empty() {
            let mut has = self.has.clone();
            has.sort();
            parts.push(format!("known: {}", has.join(", ")));
        }
        if self.age >= 1 {
            parts.push("a while ago".into());
        }
        if parts.is_empty() {
            "nothing yet".into()
        } else {
            parts.join("; ")
        }
    }
}
