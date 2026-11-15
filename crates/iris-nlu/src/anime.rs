//! Details of anime sentences: which days, whether everything was asked for, which episode, the show.

use crate::date::{self, plus_days, Period};
use crate::normalize::{int, len_utf16, normalize, trim};
use crate::re::{group, re, Find};
use crate::slots::clean;
use jiff::civil::Date;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub from: Date,
    pub to_exclusive: Date,
}

/// "alle" alone means everything in the calendar; "alle meine" / "all of my" still means the user's own list.
pub fn wants_everything(text: &str) -> bool {
    re(r"(?iu)\b(?:alle|alles|all|everything|every|komplett\w*|gesamte\w*|ganze\w*|vollständig\w*|full)\b(?!\s+(?:meine|meinen|meiner|my|mine|von mir|of my))").contains(text)
        && !re(r"(?iu)\b(?:meine liste|my list|am schauen|watching)\b").contains(text)
}

pub fn range(text: &str, today: Date) -> Range {
    let Some(slot) = date::extract(&normalize(text), today).slot else {
        return Range { from: today, to_exclusive: plus_days(today, 1) };
    };
    let span = if slot.period == Period::Week { 7 } else { slot.days.clamp(1, 31) };
    Range { from: slot.date, to_exclusive: plus_days(slot.date, span) }
}

pub fn episode(text: &str) -> Option<i64> {
    let n = normalize(text);
    // "noch eine Folge" is one more, not episode 1 (number words are digits in the normalised text)
    if is_one_more(text) && !re(r"(?iu)(?:folge|episode|ep)\s*\d").contains(&n) {
        return None;
    }
    let m = re(r"(?iu)(?:folge|episode|ep\.?|e)\s*(\d{1,4})\b|\b(\d{1,4})\.?\s*(?:folge|episode)\b").found(&n)?;
    int(if group(&m, 1).is_empty() { group(&m, 2) } else { group(&m, 1) })
}

/// "noch eine Folge", "the next episode": one more than before.
pub fn is_one_more(text: &str) -> bool {
    re(r"(?iu)\b(?:noch eine|eine weitere|nächste|naechste|nächsten|next|one more|another)\b.*\b(?:folge|episode)\b|\b(?:folge|episode)\b.*\b(?:weiter|more)\b").contains(text)
}

/// What is left once the function words are gone: most likely the show's name (matched fuzzily by the caller).
pub fn title_guess(text: &str) -> Option<String> {
    let noise = re(r"(?iu)\b(?:ich|habe|hab|have|i|just|gerade|jetzt|now|die|der|das|den|dem|the|a|eine?n?|noch|weitere?|nächste[ns]?|next|more|another|folge|episode|ep|geschaut|gesehen|watched|seen|finished|schaue|schau|watching|setze|setz|stelle|stell|markiere|mark|set|put|move|auf|to|as|als|von|of|bei|at|anime|serie|show|bitte|please|status|fortschritt|progress|pausiert|pausieren|pause|geplant|abgeschlossen|fertig|abgebrochen|dropped|completed|paused|planning|hold|on|nicht|mehr|durch|beendet|aufgegeben|aufgeben|kiyori|tracker|liste|list|meine?r?|my|mich|me|bis|until|up)\b|\d+\.?");
    let t = noise.sub(&clean(text), " ");
    let t = re(r"\s+").sub(&t, " ");
    let t = trim(&t);
    (len_utf16(t) >= 2).then(|| t.to_string())
}
