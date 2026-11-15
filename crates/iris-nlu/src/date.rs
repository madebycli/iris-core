//! The day (or days) a sentence is about: "morgen", "am 12. November", "nächste Woche", "letzten Montag",
//! "in den nächsten 3 Stunden", "am Wochenende", with the part of the day ("abends").

use crate::normalize::{int, long};
use crate::re::{group, nlu, replace_range, Find};
use jiff::civil::{Date, Weekday};
use jiff::Span;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Period {
    Now,
    Day,
    Morning,
    Afternoon,
    Evening,
    NextHours,
    Week,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DateSlot {
    pub date: Date,
    pub period: Period,
    pub hours: Option<i64>,
    pub days: i64,
}

impl DateSlot {
    pub fn day(date: Date) -> Self {
        Self { date, period: Period::Day, hours: None, days: 1 }
    }
    pub fn with(date: Date, period: Period) -> Self {
        Self { date, period, hours: None, days: 1 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DateExtraction {
    pub slot: Option<DateSlot>,
    pub rest: String,
    pub invalid: bool,
}

pub(crate) const DAYS: [(&str, i8); 14] = [
    ("montag", 1), ("monday", 1), ("dienstag", 2), ("tuesday", 2), ("mittwoch", 3), ("wednesday", 3), ("donnerstag", 4), ("thursday", 4),
    ("freitag", 5), ("friday", 5), ("samstag", 6), ("saturday", 6), ("sonntag", 7), ("sunday", 7),
];
const MONTHS: [(&str, i8); 20] = [
    ("januar", 1), ("january", 1), ("februar", 2), ("february", 2), ("märz", 3), ("march", 3), ("april", 4), ("mai", 5), ("may", 5), ("juni", 6), ("june", 6),
    ("juli", 7), ("july", 7), ("august", 8), ("september", 9), ("oktober", 10), ("october", 10), ("november", 11), ("dezember", 12), ("december", 12),
];

fn month_names() -> String {
    MONTHS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("|")
}

fn day_names() -> String {
    DAYS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("|")
}

fn month_of(name: &str) -> i64 {
    MONTHS.iter().find(|(n, _)| *n == name).map(|(_, m)| *m as i64).unwrap_or(0)
}

pub(crate) fn weekday_of(name: &str) -> Weekday {
    let n = DAYS.iter().find(|(d, _)| *d == name).map(|(_, n)| *n).unwrap_or(1);
    Weekday::from_monday_one_offset(n).unwrap()
}

pub(crate) fn plus_days(d: Date, n: i64) -> Date {
    d.checked_add(Span::new().days(n)).unwrap_or(d)
}

pub(crate) fn plus_months(d: Date, n: i64) -> Date {
    d.checked_add(Span::new().months(n)).unwrap_or(d)
}

/// The next day with this weekday after `d` (never `d` itself).
pub(crate) fn next(d: Date, w: Weekday) -> Date {
    let ahead = (w.to_monday_zero_offset() as i64 - d.weekday().to_monday_zero_offset() as i64).rem_euclid(7);
    plus_days(d, if ahead == 0 { 7 } else { ahead })
}

pub(crate) fn next_or_same(d: Date, w: Weekday) -> Date {
    if d.weekday() == w {
        d
    } else {
        next(d, w)
    }
}

pub(crate) fn previous(d: Date, w: Weekday) -> Date {
    let back = (d.weekday().to_monday_zero_offset() as i64 - w.to_monday_zero_offset() as i64).rem_euclid(7);
    plus_days(d, -(if back == 0 { 7 } else { back }))
}

fn make(y: i64, m: i64, d: i64) -> Option<Date> {
    Date::new(i16::try_from(y).ok()?, i8::try_from(m).ok()?, i8::try_from(d).ok()?).ok()
}

/// Next date with this day and month on or after `today` (or in the given year); none if it does not exist.
fn calendar_date(today: Date, day: i64, month: i64, year: Option<i64>) -> Option<Date> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if let Some(y) = year {
        return make(y, month, day).filter(|d| (2000..=2100).contains(&(d.year() as i64)));
    }
    let this_year = make(today.year() as i64, month, day)?;
    if this_year < today {
        make(today.year() as i64 + 1, month, day)
    } else {
        Some(this_year)
    }
}

pub fn extract(text: &str, today: Date) -> DateExtraction {
    let mut rest = text.to_string();
    let mut slot: Option<DateSlot> = None;
    let mut invalid = false;
    let days = day_names();
    let months = month_names();

    macro_rules! remove {
        ($pattern:expr, $create:expr) => {{
            let r = nlu(&$pattern);
            if let Some(m) = r.found(&rest) {
                let (s, e) = (m.get(0).unwrap().start(), m.get(0).unwrap().end());
                let create: &dyn Fn(&crate::re::Caps) -> Option<DateSlot> = &$create;
                slot = create(&m);
                invalid = invalid || slot.is_none();
                rest = replace_range(&rest, s, e, " ");
            }
        }};
    }

    remove!(r"\b(?:in den nächsten|nächsten|nächste|in the next|over the next|next)\s+(?:(\d+)\s+)?(?:stunden|hours)\b", |m| {
        if group(m, 1).is_empty() {
            Some(DateSlot { date: today, period: Period::NextHours, hours: Some(6), days: 1 })
        } else {
            int(group(m, 1)).filter(|v| (1..=168).contains(v)).map(|h| DateSlot { date: today, period: Period::NextHours, hours: Some(h), days: 1 })
        }
    });
    if slot.is_none() {
        remove!(r"\b(?:(?:in den|in the|über die|für die|over the|for the)\s+)?(?:nächsten|kommenden|next)\s+(\d+)\s+(?:tagen?|days?)\b", |m| {
            int(group(m, 1)).filter(|n| (1..=14).contains(n)).map(|n| DateSlot { date: today, period: Period::Day, hours: None, days: n })
        });
    }
    // explicit dates: "12. November", "12.11.", "November 12th", "am Donnerstag den 12."
    if slot.is_none() {
        remove!(format!(r"\b(?:am |on |den |the )?(\d{{1,2}})(?:\.|st|nd|rd|th)?\s*(?:of\s+)?({months})(?:\s+(\d{{4}}))?\b"), |m| {
            calendar_date(today, group(m, 1).parse().unwrap_or(0), month_of(group(m, 2)), int(group(m, 3))).map(DateSlot::day)
        });
    }
    if slot.is_none() {
        remove!(format!(r"\b(?:on )?({months})\s+(\d{{1,2}})(?:st|nd|rd|th)?(?:,?\s+(\d{{4}}))?\b"), |m| {
            calendar_date(today, group(m, 2).parse().unwrap_or(0), month_of(group(m, 1)), int(group(m, 3))).map(DateSlot::day)
        });
    }
    if slot.is_none() {
        remove!(r"\b(?:am |den )?(\d{1,2})\.(\d{1,2})(?:\.(\d{4})?)?(?![\d:])(?!\s*(?:uhr|h)\b)", |m| {
            calendar_date(today, group(m, 1).parse().unwrap_or(0), group(m, 2).parse().unwrap_or(0), int(group(m, 3))).map(DateSlot::day)
        });
    }
    if slot.is_none() {
        remove!(format!(r"\b(?:am|den)\s+(?:(?:{days})\s+(?:den\s+)?)?(\d{{1,2}})(?!\d)(?!\s*(?:uhr|h|minuten|stunden|prozent|grad|mal|euro)\b)"), |m| {
            let day: i64 = group(m, 1).parse().unwrap_or(0);
            let this_month = calendar_date(today, day, today.month() as i64, Some(today.year() as i64));
            let found = match this_month {
                Some(d) if d >= today => Some(d),
                _ => {
                    let mut month = plus_months(today, 1).first_of_month();
                    let mut hit = None;
                    for _ in 0..12 {
                        if let Some(d) = calendar_date(today, day, month.month() as i64, Some(month.year() as i64)) {
                            hit = Some(d);
                            break;
                        }
                        month = plus_months(month, 1);
                    }
                    hit
                }
            };
            found.map(DateSlot::day)
        });
    }
    // "Donnerstag den 12." is one date, not two
    let weekday_words = nlu(&format!(r"\b(?:am |on |den )?(?:{days})\b,?"));
    if slot.is_some() && weekday_words.contains(&rest) {
        rest = weekday_words.sub(&rest, " ");
    }
    if slot.is_none() {
        remove!(r"\b(?:(?:am|dieses|this|next|nächstes) )?(?:wochenende|weekend)\b", |_| {
            let sat = next_or_same(today, Weekday::Saturday);
            Some(if today.weekday() == Weekday::Sunday { DateSlot::day(today) } else { DateSlot { date: sat, period: Period::Day, hours: None, days: 2 } })
        });
    }
    if slot.is_none() {
        remove!(r"\b(?:in)\s+(\d+)\s+(?:tagen?|days?)\b", |m| long(group(m, 1)).filter(|v| (0..=36500).contains(v)).map(|d| DateSlot::day(plus_days(today, d))));
    }
    if slot.is_none() {
        remove!(r"\b(?:(?:in|für|über) (?:der|die|den) )?(?:nächste|nächsten|kommende|kommenden) woche\b|\b(?:in the (?:next|coming) week|over the next week|next week)\b", |_| {
            Some(DateSlot::with(next(today, Weekday::Monday), Period::Week))
        });
    }
    if slot.is_none() {
        remove!(r"\b(?:(?:in|für|über) (?:der|die|den) )?(?:diese|dieser|diesen) woche\b|\bthis week\b", |_| Some(DateSlot::with(today, Period::Week)));
    }
    // the past, for history questions ("was kam gestern raus", "what aired last Monday")
    if slot.is_none() {
        remove!(r"\b(?:vorgestern|day before yesterday)\b", |_| Some(DateSlot::day(plus_days(today, -2))));
    }
    if slot.is_none() {
        remove!(r"\b(?:gestern|yesterday)\b", |_| Some(DateSlot::day(plus_days(today, -1))));
    }
    if slot.is_none() {
        remove!(r"\b(?:vor\s+(\d+)\s+tagen?|(\d+)\s+days?\s+ago)\b", |m| {
            let n = if group(m, 1).is_empty() { group(m, 2) } else { group(m, 1) };
            long(n).filter(|n| (1..=3650).contains(n)).map(|n| DateSlot::day(plus_days(today, -n)))
        });
    }
    if slot.is_none() {
        remove!(r"\b(?:letzte|letzter|vergangene|vergangener|last|past)\s+(?:woche|week)\b", |_| Some(DateSlot::with(plus_days(today, -7), Period::Week)));
    }
    if slot.is_none() {
        remove!(format!(r"\b(?:letzten|letzte|vergangenen|last)\s+({days})\b"), |m| Some(DateSlot::day(previous(today, weekday_of(group(m, 1))))));
    }
    if slot.is_none() {
        remove!(r"\b(?:übermorgen|day after tomorrow|tomorrow|morgen|today|heute|tonight|jetzt|now)\b", |m| {
            Some(match m.get(0).unwrap().as_str() {
                "übermorgen" | "day after tomorrow" => DateSlot::day(plus_days(today, 2)),
                "morgen" | "tomorrow" => DateSlot::day(plus_days(today, 1)),
                "tonight" => DateSlot::with(today, Period::Evening),
                "jetzt" | "now" => DateSlot::with(today, Period::Now),
                _ => DateSlot::day(today),
            })
        });
    }
    if slot.is_none() {
        remove!(format!(r"\b(?:(?:am|on|next|nächsten) )?({days})\b"), |m| {
            let day = weekday_of(group(m, 1));
            let v = m.get(0).unwrap().as_str();
            Some(DateSlot::day(if v.starts_with("next ") || v.starts_with("nächsten ") { next(today, day) } else { next_or_same(today, day) }))
        });
    }
    if let Some(part) = nlu(r"\b(?:this )?(?:früh|morgens|vormittag|vormittags|morning|nachmittag|nachmittags|afternoon|abend|abends|evening)\b").found(&rest) {
        let p = part.get(0).unwrap();
        let v = p.as_str();
        let period = if v.contains("früh") || v.contains("morg") || v.contains("morning") || v.contains("vormittag") {
            Period::Morning
        } else if v.contains("nachm") || v.contains("afternoon") {
            Period::Afternoon
        } else {
            Period::Evening
        };
        let mut s = slot.unwrap_or_else(|| DateSlot::day(today));
        s.period = period;
        slot = Some(s);
        rest = replace_range(&rest, p.start(), p.end(), " ");
    }
    let rest = nlu(r"\s+").sub(&rest, " ").trim().to_string();
    DateExtraction { slot, rest, invalid }
}
