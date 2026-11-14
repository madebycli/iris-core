//! Time expressions: finding them in a sentence and turning them into an instant.
//!
//! [`scan`] reads tokens with the words of a [`LangPack`] and reports which tokens were time
//! words, so a skill can take the rest as the title. [`resolve`] turns the result into an
//! instant against a clock and a time zone. The two are separate so a skill can hold a
//! half-finished request ("remind me at five" … "about what?") as plain data.

use crate::lang::LangPack;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan, Zoned};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Morning,
    Afternoon,
    Evening,
    Night,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayRef {
    /// Days from today.
    Offset(i32),
    /// ISO weekday, Monday = 1. `next` skips today.
    Weekday { dow: u8, next: bool },
    Date { day: u8, month: u8, year: Option<i16> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    pub hour: u8,
    pub minute: u8,
    /// Said in a way that leaves no doubt about the half of the day: "17 Uhr", "08:00", "5 pm".
    pub exact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Daily,
    Weekdays,
    Weekly,
    /// ISO weekdays.
    Days(Vec<u8>),
}

impl Repeat {
    /// The stored form, shared with the Android app's database.
    pub fn code(&self) -> String {
        match self {
            Repeat::Daily => "daily".into(),
            Repeat::Weekdays => "weekdays".into(),
            Repeat::Weekly => "weekly".into(),
            Repeat::Days(d) => format!("days:{}", d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")),
        }
    }
}

/// What was said about time, not yet tied to a date.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeExpr {
    pub relative_secs: Option<i64>,
    pub day: Option<DayRef>,
    pub clock: Option<Clock>,
    pub period: Option<Period>,
    pub repeat: Option<Repeat>,
}

impl TimeExpr {
    pub fn is_empty(&self) -> bool {
        self.relative_secs.is_none() && self.day.is_none() && self.clock.is_none() && self.period.is_none() && self.repeat.is_none()
    }

    /// Fills what `self` lacks from `other`: "tomorrow" then "at eight" is one time.
    pub fn merge(&mut self, other: TimeExpr) {
        self.relative_secs = self.relative_secs.or(other.relative_secs);
        self.day = self.day.take().or(other.day);
        self.clock = self.clock.or(other.clock);
        self.period = self.period.or(other.period);
        self.repeat = self.repeat.take().or(other.repeat);
    }
}

pub struct Scan {
    pub expr: TimeExpr,
    /// One flag per token: true where the token was part of the time expression.
    pub used: Vec<bool>,
}

/// Finds everything about time in `tokens`.
pub fn scan(tokens: &[&str], lang: &LangPack) -> Scan {
    let mut s = Scanner { t: tokens, lang, expr: TimeExpr::default(), used: vec![false; tokens.len()] };
    let mut i = 0;
    while i < tokens.len() {
        let n = s.step(i);
        i += n.max(1);
    }
    Scan { expr: s.expr, used: s.used }
}

/// A length of time ("5 minutes", "eine halbe Stunde", "1 hour 30 minutes") and the tokens it used.
pub fn scan_duration(tokens: &[&str], lang: &LangPack) -> Option<(i64, Vec<bool>)> {
    let mut used = vec![false; tokens.len()];
    let mut total = 0i64;
    let mut i = 0;
    while i < tokens.len() {
        if let Some((secs, n)) = duration_at(tokens, i, lang) {
            total += secs;
            used[i..i + n].iter_mut().for_each(|u| *u = true);
            i += n;
            if let Some(k) = LangPack::phrase_at(tokens, i, &lang.time.and_words) {
                if duration_at(tokens, i + k, lang).is_some() {
                    used[i..i + k].iter_mut().for_each(|u| *u = true);
                    i += k;
                }
            }
        } else {
            i += 1;
        }
    }
    (total > 0).then_some((total, used))
}

/// One "number unit" or fraction phrase at `i`: seconds and tokens covered.
fn duration_at(t: &[&str], i: usize, lang: &LangPack) -> Option<(i64, usize)> {
    if let Some((secs, n)) = LangPack::lookup_at(t, i, &lang.time.fractions) {
        return Some((*secs, n));
    }
    let (value, n) = lang.number_at(t, i)?;
    if value == 1 {
        if let Some((secs, k)) = LangPack::lookup_at(t, i + n, &lang.time.fractions) {
            return Some((*secs, n + k));
        }
    }
    let unit = lang.time.units.seconds(t.get(i + n)?)?;
    Some((value * unit, n + 1))
}

struct Scanner<'a> {
    t: &'a [&'a str],
    lang: &'a LangPack,
    expr: TimeExpr,
    used: Vec<bool>,
}

impl Scanner<'_> {
    fn mark(&mut self, from: usize, n: usize) -> usize {
        self.used[from..from + n].iter_mut().for_each(|u| *u = true);
        n
    }

    fn word(&self, i: usize, list: &[String]) -> bool {
        self.t.get(i).is_some_and(|w| list.iter().any(|x| x == w))
    }

    /// Tries every kind of time word at `i`; returns the tokens consumed, 0 for none.
    fn step(&mut self, i: usize) -> usize {
        let lang = self.lang;
        let w = &lang.time;
        // "um in 10 Minuten", "um morgen": a clock marker with nothing to mark
        if self.word(i, &w.clock_markers) && self.t.get(i + 1).is_some_and(|next| w.in_words.iter().any(|x| x == next) || w.day_offsets.contains_key(*next)) {
            return self.mark(i, 1);
        }
        // repetition
        for (list, rep) in [(&w.repeat.daily, Repeat::Daily), (&w.repeat.weekdays, Repeat::Weekdays), (&w.repeat.weekly, Repeat::Weekly)] {
            if let Some(n) = LangPack::phrase_at(self.t, i, list) {
                self.expr.repeat = Some(rep);
                return self.mark(i, n);
            }
        }
        if self.word(i, &w.every_words) {
            if let Some(dow) = self.t.get(i + 1).and_then(|x| w.weekdays.get(*x)) {
                let mut days = match self.expr.repeat.take() {
                    Some(Repeat::Days(d)) => d,
                    _ => Vec::new(),
                };
                days.push(*dow);
                self.expr.repeat = Some(Repeat::Days(days));
                return self.mark(i, 2);
            }
        }
        // "heute morgen": a day word that means a part of the day once the day is known
        if self.expr.day.is_some() && self.expr.period.is_none() {
            if let Some(p) = w.period_after_day.get(self.t[i]).and_then(|p| period_of(p)) {
                self.expr.period = Some(p);
                return self.mark(i, 1);
            }
        }
        // relative: "in 20 minutes", "in einer halben Stunde", "in 1 hour and 30 minutes"
        if self.word(i, &w.in_words) && self.expr.relative_secs.is_none() {
            let mut j = i + 1;
            let mut total = 0;
            while let Some((secs, n)) = duration_at(self.t, j, self.lang) {
                total += secs;
                j += n;
                match LangPack::phrase_at(self.t, j, &w.and_words) {
                    Some(k) if duration_at(self.t, j + k, self.lang).is_some() => j += k,
                    _ => break,
                }
            }
            if total > 0 {
                self.expr.relative_secs = Some(total);
                return self.mark(i, j - i);
            }
        }
        if self.expr.clock.is_none() {
            let n = self.clock(i);
            if n > 0 {
                return self.mark(i, n);
            }
        }
        if self.expr.day.is_none() {
            let n = self.day(i);
            if n > 0 {
                return self.mark(i, n);
            }
        }
        if self.expr.period.is_none() {
            for (list, p) in [(&w.periods.morning, Period::Morning), (&w.periods.afternoon, Period::Afternoon), (&w.periods.evening, Period::Evening), (&w.periods.night, Period::Night)] {
                if let Some(n) = LangPack::phrase_at(self.t, i, list) {
                    self.expr.period = Some(p);
                    return self.mark(i, n);
                }
            }
        }
        0
    }

    fn num(&self, i: usize) -> Option<(i64, usize)> {
        self.lang.number_at(self.t, i)
    }

    fn set_clock(&mut self, hour: i64, minute: i64, exact: bool) -> bool {
        if !(0..=24).contains(&hour) || !(0..60).contains(&minute) || (hour == 24 && minute != 0) {
            return false;
        }
        let hour = if hour == 24 { 0 } else { hour };
        self.expr.clock = Some(Clock { hour: hour as u8, minute: minute as u8, exact: exact || hour == 0 || hour >= 13 });
        true
    }

    /// am/pm after a time at `i`: converts the clock and returns the tokens it took.
    fn meridiem(&mut self, i: usize) -> usize {
        let lang = self.lang;
        let w = &lang.time;
        let (am, pm) = (self.word(i, &w.am), self.word(i, &w.pm));
        if let (true, Some(c)) = (am || pm, self.expr.clock.as_mut()) {
            if pm && c.hour < 12 {
                c.hour += 12;
            }
            if am && c.hour == 12 {
                c.hour = 0;
            }
            c.exact = true;
            return 1;
        }
        0
    }

    fn clock(&mut self, i: usize) -> usize {
        let lang = self.lang;
        let w = &lang.time;
        let marker = usize::from(self.word(i, &w.clock_markers));
        let j = i + marker;
        if let Some((named, n)) = LangPack::lookup_at(self.t, j, &w.named) {
            if let Some((h, m)) = split_clock(named, ':') {
                self.set_clock(h, m, true);
                return marker + n;
            }
        }
        let Some(tok) = self.t.get(j).copied() else { return 0 };
        let t = self.t;
        let follows_oclock = |k: usize| t.get(k).is_some_and(|x| w.oclock.iter().any(|o| o == x));
        // 16:30, 8:05 — and 16.30 when "Uhr" follows or a marker precedes
        for sep in [':', '.'] {
            if let Some((h, m)) = split_clock(tok, sep) {
                if sep == '.' && marker == 0 && !follows_oclock(j + 1) {
                    continue;
                }
                if self.set_clock(h, m, tok.starts_with('0')) {
                    let mut n = j + 1 - i;
                    n += usize::from(follows_oclock(i + n));
                    n += self.meridiem(i + n);
                    return n;
                }
            }
        }
        // 5pm, 5:30pm
        for (list, pm) in [(&w.pm, true), (&w.am, false)] {
            for suffix in list {
                if let Some(head) = tok.strip_suffix(suffix.as_str()) {
                    let (h, m) = split_clock(head, ':').or_else(|| head.parse().ok().map(|h| (h, 0))).unwrap_or((-1, 0));
                    if (1..=12).contains(&h) && self.set_clock(if pm && h < 12 { h + 12 } else if !pm && h == 12 { 0 } else { h }, m, true) {
                        if let Some(c) = self.expr.clock.as_mut() {
                            c.exact = true;
                        }
                        return j + 1 - i;
                    }
                }
            }
        }
        // halb fünf / half past four
        if let Some(n) = LangPack::phrase_at(self.t, j, &w.half) {
            if let Some((h, k)) = self.num(j + n) {
                let hour = if w.half_means == "before" { (h + 23) % 24 } else { h };
                if (1..=12).contains(&h) && self.set_clock(hour, 30, false) {
                    return j + n + k - i + self.meridiem(j + n + k);
                }
            }
        }
        for (list, minute, back) in [(&w.quarter_past, 15, false), (&w.quarter_to, 45, true)] {
            if let Some(n) = LangPack::phrase_at(self.t, j, list) {
                if let Some((h, k)) = self.num(j + n) {
                    if (1..=12).contains(&h) && self.set_clock(if back { (h + 23) % 24 } else { h }, minute, false) {
                        return j + n + k - i + self.meridiem(j + n + k);
                    }
                }
            }
        }
        let Some((first, n1)) = self.num(j) else { return 0 };
        // zehn nach acht / ten to six
        for (list, back) in [(&w.past, false), (&w.to, true)] {
            if self.word(j + n1, list) && (1..60).contains(&first) {
                if let Some((h, k)) = self.num(j + n1 + 1) {
                    let (hour, minute) = if back { ((h + 23) % 24, 60 - first) } else { (h, first) };
                    if (1..=12).contains(&h) && self.set_clock(hour, minute, false) {
                        return j + n1 + 1 + k - i + self.meridiem(j + n1 + 1 + k);
                    }
                }
            }
        }
        // 8 Uhr, 8 Uhr 30, 8 o'clock
        if follows_oclock(j + n1) {
            let mut n = j + n1 + 1 - i;
            let minute = match self.num(i + n) {
                Some((m, k)) if (0..60).contains(&m) && self.lang.time.units.seconds(self.t.get(i + n + k).copied().unwrap_or("")).is_none() => {
                    n += k;
                    m
                }
                _ => 0,
            };
            if self.set_clock(first, minute, false) {
                return n + self.meridiem(i + n);
            }
            return 0;
        }
        // 5 pm
        if (self.word(j + n1, &w.am) || self.word(j + n1, &w.pm))
            && (1..=12).contains(&first) && self.set_clock(first, 0, false) {
                return j + n1 - i + self.meridiem(j + n1);
            }
        // "um 8", "at five": only with the marker, and not when a unit follows ("um 5 Minuten")
        if marker == 1 && self.lang.time.units.seconds(self.t.get(j + n1).copied().unwrap_or("")).is_none() && self.set_clock(first, 0, false) {
            return j + n1 - i;
        }
        0
    }

    fn day(&mut self, i: usize) -> usize {
        let lang = self.lang;
        let w = &lang.time;
        if let Some((offset, n)) = LangPack::lookup_at(self.t, i, &w.day_offsets) {
            self.expr.day = Some(DayRef::Offset(*offset));
            return n;
        }
        let filler = usize::from(self.word(i, &w.date_fillers));
        let mut j = i + filler;
        // filler twice: "on the 3rd"
        j += usize::from(self.word(j, &w.date_fillers));
        let next = LangPack::phrase_at(self.t, j, &w.next_words).unwrap_or(0);
        if let Some(dow) = self.t.get(j + next).and_then(|x| w.weekdays.get(*x)) {
            self.expr.day = Some(DayRef::Weekday { dow: *dow, next: next > 0 });
            return j + next + 1 - i;
        }
        let Some(tok) = self.t.get(j).copied() else { return 0 };
        // 3.12. / 3.12.2026
        let parts: Vec<&str> = tok.split('.').filter(|p| !p.is_empty()).collect();
        if (2..=3).contains(&parts.len()) && tok.contains('.') && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            let (d, m) = (parts[0].parse::<u8>().unwrap_or(0), parts[1].parse::<u8>().unwrap_or(0));
            let year = parts.get(2).and_then(|y| y.parse::<i16>().ok()).map(|y| if y < 100 { y + 2000 } else { y });
            if (1..=31).contains(&d) && (1..=12).contains(&m) && !self.word(j + 1, &w.oclock) {
                self.expr.day = Some(DayRef::Date { day: d, month: m, year });
                return j + 1 - i;
            }
        }
        // 3 Dezember / the 3rd of December
        if let Some((d, n)) = self.num(j) {
            let joiner = usize::from(self.word(j + n, &w.date_joiners));
            if let Some(m) = self.t.get(j + n + joiner).and_then(|x| w.months.get(*x)) {
                if (1..=31).contains(&d) {
                    self.expr.day = Some(DayRef::Date { day: d as u8, month: *m, year: None });
                    return j + n + joiner + 1 - i;
                }
            }
        }
        // December 3rd
        if let Some(m) = w.months.get(tok) {
            if let Some((d, n)) = self.num(j + 1) {
                if (1..=31).contains(&d) {
                    self.expr.day = Some(DayRef::Date { day: d as u8, month: *m, year: None });
                    return j + 1 + n - i;
                }
            }
        }
        0
    }
}

fn period_of(name: &str) -> Option<Period> {
    match name {
        "morning" => Some(Period::Morning),
        "afternoon" => Some(Period::Afternoon),
        "evening" => Some(Period::Evening),
        "night" => Some(Period::Night),
        _ => None,
    }
}

fn split_clock(token: &str, sep: char) -> Option<(i64, i64)> {
    let (h, m) = token.split_once(sep)?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 || !h.chars().chain(m.chars()).all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((h.parse().ok()?, m.parse().ok()?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeError {
    /// Nothing about time was said.
    Missing,
    /// The time named has already passed ("today at 8" in the afternoon).
    Past,
    /// Not a real time: 31 February, an hour the clocks skip, "8 in the evening at 14:00".
    Invalid,
}

/// How a bare hour ("um fünf", "at 8") is read when the half of the day was not said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfDay {
    /// The next time a clock shows that hour: at 16:00, "um fünf" is 17:00 today. Reminders.
    Nearest,
    /// Exactly as said, on a 24-hour clock: "um fünf" is 5:00. Alarms.
    AsSaid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub at: Timestamp,
    /// Only a day was named; the time of day is the default.
    pub date_only: bool,
    /// The half of the day was chosen by the engine, so the reply should name the time in full.
    pub assumed_half_day: bool,
    pub repeat: Option<Repeat>,
}

/// Time of day given to "tomorrow" without a time.
pub const DEFAULT_TIME: (i8, i8) = (9, 0);

pub fn resolve(expr: &TimeExpr, now: &Zoned, policy: HalfDay) -> Result<Resolved, TimeError> {
    if expr.is_empty() {
        return Err(TimeError::Missing);
    }
    let repeat = expr.repeat.clone();
    if let (Some(secs), None, None) = (expr.relative_secs, &expr.day, expr.clock) {
        let at = now.timestamp().checked_add(secs.seconds()).map_err(|_| TimeError::Invalid)?;
        return Ok(Resolved { at, date_only: false, assumed_half_day: false, repeat });
    }
    let tz = now.time_zone().clone();
    let today = now.date();
    let fixed_day = match &expr.day {
        None => None,
        Some(DayRef::Offset(n)) => Some(today.checked_add((*n as i64).days()).map_err(|_| TimeError::Invalid)?),
        Some(DayRef::Weekday { dow, next }) => {
            let mut ahead = (*dow as i64 - today.weekday().to_monday_one_offset() as i64).rem_euclid(7);
            if ahead == 0 && *next {
                ahead = 7;
            }
            Some(today.checked_add(ahead.days()).map_err(|_| TimeError::Invalid)?)
        }
        Some(DayRef::Date { day, month, year }) => {
            let pick = |y: i16| Date::new(y, *month as i8, *day as i8).map_err(|_| TimeError::Invalid);
            match year {
                Some(y) => Some(pick(*y)?),
                None => {
                    let this = pick(today.year())?;
                    Some(if this < today { pick(today.year() + 1)? } else { this })
                }
            }
        }
    };
    let at_zone = |d: Date, t: Time| -> Result<Zoned, TimeError> { tz.to_ambiguous_zoned(DateTime::from_parts(d, t)).unambiguous().map_err(|_| TimeError::Invalid) };
    let Some(clock) = expr.clock else {
        // a day, a part of the day or a repetition, without a clock time
        let hour = match expr.period {
            Some(Period::Morning) => 8,
            Some(Period::Afternoon) => 15,
            Some(Period::Evening) => 19,
            Some(Period::Night) => 22,
            None => DEFAULT_TIME.0,
        };
        let time = Time::new(hour, DEFAULT_TIME.1, 0, 0).map_err(|_| TimeError::Invalid)?;
        let mut day = fixed_day.unwrap_or(today);
        let mut z = at_zone(day, time)?;
        if z <= *now {
            if matches!(expr.day, Some(DayRef::Offset(0))) && expr.period.is_some() {
                return Err(TimeError::Past);
            }
            if matches!(expr.day, Some(DayRef::Weekday { .. })) {
                day = day.checked_add(7.days()).map_err(|_| TimeError::Invalid)?;
            } else if fixed_day.is_none() || matches!(expr.day, Some(DayRef::Offset(0))) {
                day = day.checked_add(1.days()).map_err(|_| TimeError::Invalid)?;
            } else {
                return Err(TimeError::Past);
            }
            z = at_zone(day, time)?;
        }
        return Ok(Resolved { at: first_repeat(z, &repeat, &tz)?.timestamp(), date_only: expr.period.is_none(), assumed_half_day: false, repeat });
    };
    // the hours this clock time can mean
    let h = clock.hour as i8;
    let mut assumed = false;
    let hours: Vec<i8> = match expr.period {
        Some(Period::Morning) if h < 12 => vec![h],
        Some(Period::Afternoon) if (1..=6).contains(&h) => vec![h + 12],
        Some(Period::Afternoon) if (12..=18).contains(&h) => vec![h],
        Some(Period::Evening) if (5..=11).contains(&h) => vec![h + 12],
        Some(Period::Evening) if (17..=23).contains(&h) => vec![h],
        Some(Period::Night) if (9..=11).contains(&h) => vec![h + 12],
        Some(Period::Night) if h == 12 => vec![0],
        Some(Period::Night) if h <= 5 || h >= 21 => vec![h],
        Some(_) => return Err(TimeError::Invalid),
        // a routine ("every day at 7") is meant as said
        None if clock.exact || policy == HalfDay::AsSaid || repeat.is_some() => vec![h],
        None => {
            assumed = true;
            match (fixed_day.is_some(), h) {
                // "morgen um 3": nobody is reminded at three at night
                (true, 1..=6) => vec![h + 12],
                (true, _) => {
                    assumed = false;
                    vec![h]
                }
                (false, 12) => vec![0, 12],
                (false, _) => vec![h, h + 12],
            }
        }
    };
    let minute = clock.minute as i8;
    let candidates = |day: Date| -> Vec<Zoned> { hours.iter().filter_map(|hh| Time::new(*hh, minute, 0, 0).ok()).filter_map(|t| at_zone(day, t).ok()).collect() };
    let day = fixed_day.unwrap_or(today);
    let mut found = candidates(day).into_iter().filter(|z| z > now).min();
    if found.is_none() {
        let later = match &expr.day {
            None => Some(1),
            Some(DayRef::Weekday { .. }) => Some(7),
            _ => None,
        };
        let Some(days) = later else {
            return Err(if candidates(day).is_empty() { TimeError::Invalid } else { TimeError::Past });
        };
        let next_day = day.checked_add((days as i64).days()).map_err(|_| TimeError::Invalid)?;
        found = candidates(next_day).into_iter().min();
    }
    let z = found.ok_or(TimeError::Invalid)?;
    let at = match expr.relative_secs {
        Some(_) => return Err(TimeError::Invalid),
        None => first_repeat(z, &repeat, &tz)?.timestamp(),
    };
    Ok(Resolved { at, date_only: false, assumed_half_day: assumed, repeat })
}

/// The first instant at or after `z` that the repetition allows.
fn first_repeat(z: Zoned, repeat: &Option<Repeat>, tz: &TimeZone) -> Result<Zoned, TimeError> {
    let allowed = |dow: i8| match repeat {
        Some(Repeat::Weekdays) => dow <= 5,
        Some(Repeat::Days(days)) => days.contains(&(dow as u8)),
        _ => true,
    };
    let mut date = z.date();
    for _ in 0..8 {
        if allowed(date.weekday().to_monday_one_offset()) {
            return tz.to_ambiguous_zoned(DateTime::from_parts(date, z.time())).unambiguous().map_err(|_| TimeError::Invalid);
        }
        date = date.checked_add(1.days()).map_err(|_| TimeError::Invalid)?;
    }
    Err(TimeError::Invalid)
}

/// The next instant after `now` for a repeating entry that was due at `due`. `None` when it does not repeat.
pub fn next_occurrence(due: Timestamp, repeat_code: &str, zone: &str, now: Timestamp) -> Option<Timestamp> {
    let days: Vec<i8> = repeat_code.strip_prefix("days:").map(|d| d.split(',').filter_map(|x| x.parse().ok()).filter(|x| (1..=7).contains(x)).collect()).unwrap_or_default();
    if !matches!(repeat_code, "daily" | "weekdays" | "weekly") && days.is_empty() {
        return None;
    }
    let tz = TimeZone::get(zone).ok()?;
    let first = due.to_zoned(tz.clone());
    let (time, weekday) = (first.time(), first.date().weekday());
    let mut date = first.date().max(now.to_zoned(tz.clone()).date());
    for _ in 0..370 {
        let dow = date.weekday().to_monday_one_offset();
        let fits = match repeat_code {
            "weekdays" => dow <= 5,
            "weekly" => date.weekday() == weekday,
            "daily" => true,
            _ => days.contains(&dow),
        };
        if fits {
            if let Ok(z) = tz.to_ambiguous_zoned(DateTime::from_parts(date, time)).compatible() {
                if z.timestamp() > now && z.timestamp() > due {
                    return Some(z.timestamp());
                }
            }
        }
        date = date.checked_add(1.days()).ok()?;
    }
    None
}

/// "heute um 17:00 Uhr", "on Friday at 15:00": an instant in the words of `lang`, seen from `now`.
pub fn describe(at: Timestamp, now: &Zoned, lang: &LangPack, date_only: bool) -> String {
    let z = at.to_zoned(now.time_zone().clone());
    let days = (z.date() - now.date()).get_days();
    let time = lang.format.clock.replace("{h}", &z.hour().to_string()).replace("{mm}", &format!("{:02}", z.minute()));
    let weekday = lang.format.weekday_names.get(z.date().weekday().to_monday_zero_offset() as usize).cloned().unwrap_or_default();
    let date = lang.format.date.replace("{d}", &z.day().to_string()).replace("{m}", &z.month().to_string());
    let kind = match days {
        0 => "today",
        1 => "tomorrow",
        2..=6 => "weekday",
        _ => "date",
    };
    let key = format!("{}.{kind}", if date_only { "day" } else { "when" });
    lang.messages.get(&key, &[("time", &time), ("weekday", &weekday), ("date", &date)])
}

/// "5 Minuten", "1 hour 30 minutes".
pub fn describe_duration(secs: i64, lang: &LangPack) -> String {
    let mut parts = Vec::new();
    let mut rest = secs.max(0);
    for (unit, size) in [("day", 86_400), ("hour", 3600), ("minute", 60), ("second", 1)] {
        let n = rest / size;
        rest %= size;
        if n > 0 {
            parts.push(lang.messages.plural(&format!("unit.{unit}"), n, &[]));
        }
    }
    if parts.is_empty() {
        parts.push(lang.messages.plural("unit.second", 0, &[]));
    }
    parts.join(" ")
}
