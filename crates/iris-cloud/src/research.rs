//! Knowledge questions for a separate, stronger "research" model.
//!
//! The research model gets nothing but a short instruction and the question: no app protocol, no
//! memory, no history. Its answer is shown and read aloud as it is, after [clean] has removed what
//! does not read well (Markdown, lists, links, "Here is the answer:").

/// Answers longer than this are cut at a sentence end.
pub const MAX_ANSWER: usize = 900;
/// Questions longer than this are cut (a spoken question is never this long).
pub const MAX_QUESTION: usize = 600;

/// The whole message for the research model.
pub fn prompt(language: &str, question: &str) -> String {
    let q: String = question.trim().chars().take(MAX_QUESTION).collect();
    if language == "de" {
        format!(
            "Beantworte die Frage in drei bis vier kurzen, klaren Sätzen als Fließtext auf Deutsch. \
             Fasse das Wichtigste zusammen. Keine Einleitung wie „Hier ist die Antwort“, keine Überschriften, \
             Listen, Tabellen, Links, Quellenangaben oder Emojis. Der Text wird vorgelesen. \
             Wenn du dir nicht sicher bist, sag das in einem Satz, statt zu raten.\n\nFrage: {q}"
        )
    } else {
        format!(
            "Answer the question in three or four short, clear sentences of plain prose in English. \
             Sum up what matters most. No introduction such as \"Here is the answer\", no headings, lists, \
             tables, links, citations or emojis. The text will be read aloud. \
             If you are not sure, say so in one sentence instead of guessing.\n\nQuestion: {q}"
        )
    }
}

/// A conversation turn for a chat that is a web page (Lumo): a plain request in the user's own voice, no rules to adopt, no
/// persona. [memory] (facts the user saved) is given once, in the first message of a chat, as background.
pub fn chat(language: &str, text: &str, memory: &str) -> String {
    let t: String = text.trim().chars().take(MAX_QUESTION * 2).collect();
    let m: String = memory.trim().chars().take(1200).collect();
    if language == "de" {
        let bg = if m.is_empty() { String::new() } else { format!("Zur Info, was ich dir über mich erzählt habe (nur Hintergrund):\n{m}\n\n") };
        format!("{bg}Antworte kurz und natürlich in zwei bis vier Sätzen als Fließtext auf Deutsch, ohne Listen oder Überschriften; die Antwort wird vorgelesen.\n\n{t}")
    } else {
        let bg = if m.is_empty() { String::new() } else { format!("For context, what I told you about myself (background only):\n{m}\n\n") };
        format!("{bg}Answer briefly and naturally in two to four sentences of plain prose in English, no lists or headings; the answer will be read aloud.\n\n{t}")
    }
}

/// The answer as Iris shows and speaks it; none when nothing readable is left.
pub fn clean(answer: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for line in answer.lines() {
        let mut l = line.trim();
        if l.starts_with("```") || l.starts_with('|') || l.chars().all(|c| matches!(c, '-' | '=' | '*' | '_' | ' ')) {
            continue;
        }
        l = l.trim_start_matches('#').trim_start();
        l = l.trim_start_matches(['-', '*', '•', '>']).trim_start();
        // "1. " / "2) "
        let digits = l.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && digits < 3 && l[digits..].starts_with(['.', ')']) && l[digits + 1..].starts_with(' ') {
            l = l[digits + 1..].trim_start();
        }
        if l.is_empty() {
            continue;
        }
        let mut s = l.to_string();
        // a list item without a full stop becomes a sentence
        if !s.ends_with(['.', '!', '?', ':', ';']) {
            s.push('.');
        }
        parts.push(s);
    }
    let mut text = links(&parts.join(" "));
    text = text.replace(['*', '`'], "").replace("__", "");
    text = citations(&text);
    text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text = preamble(&text).to_string();
    let text = cut(&text, MAX_ANSWER);
    let text = text.trim();
    (text.chars().filter(|c| c.is_alphanumeric()).count() >= 2).then(|| text.to_string())
}

/// "[Text](https://…)" becomes "Text"; bare URLs go.
fn links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match (after.find(']'), after.find("](")) {
            (Some(close), Some(mid)) if close == mid => {
                let target = &after[close + 2..];
                if let Some(end) = target.find(')') {
                    out.push_str(&rest[..open]);
                    out.push_str(&after[..close]);
                    rest = &target[end + 1..];
                    continue;
                }
                out.push_str(&rest[..=open]);
                rest = after;
            }
            _ => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.split(' ').filter(|w| !(w.starts_with("http://") || w.starts_with("https://") || w.starts_with("(http"))).collect::<Vec<_>>().join(" ")
}

/// "[1]", "[12]" and "【3】" citation marks go.
fn citations(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let (open, close) = match chars[i] {
            '[' => ('[', ']'),
            '【' => ('【', '】'),
            _ => {
                out.push(chars[i]);
                i += 1;
                continue;
            }
        };
        let mut j = i + 1;
        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == ',' || chars[j] == ' ' || chars[j] == '†') {
            j += 1;
        }
        if j > i + 1 && j < chars.len() && chars[j] == close {
            // drop a space left in front of the mark
            if out.ends_with(' ') {
                out.pop();
            }
            i = j + 1;
        } else {
            out.push(open);
            i += 1;
        }
    }
    out
}

/// Drops an introduction such as "Hier ist die Antwort:" or "Answer:".
fn preamble(s: &str) -> &str {
    let lower = s.to_lowercase();
    const STARTS: [&str; 10] = [
        "hier ist die antwort", "hier ist eine kurze antwort", "hier ist eine zusammenfassung", "antwort:", "gerne",
        "here is the answer", "here's the answer", "here is a short answer", "answer:", "sure",
    ];
    if let Some(p) = STARTS.iter().find(|p| lower.starts_with(*p)) {
        // up to the end of that first clause, only if it is short (an introduction, not the answer)
        if let Some(end) = s.char_indices().find(|(_, c)| matches!(c, ':' | '!' | '.')).map(|(i, _)| i) {
            if end <= p.len() + 40 {
                let rest = s[end + 1..].trim_start();
                if !rest.is_empty() {
                    return rest;
                }
            }
        }
    }
    s
}

/// At most `max` bytes, ending at the last full sentence that fits.
fn cut(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let head = &s[..end];
    match head.rfind(['.', '!', '?']) {
        Some(i) if i > max / 3 => head[..=i].to_string(),
        _ => format!("{}…", head.trim_end()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_is_bare() {
        let p = prompt("de", "  Wer war der 17. Präsident der USA?  ");
        assert!(p.ends_with("Frage: Wer war der 17. Präsident der USA?"));
        assert!(!p.contains("Iris") && !p.contains("JSON"));
        assert!(prompt("en", "Who?").ends_with("Question: Who?"));
    }

    #[test]
    fn markdown_lists_and_links_read_as_prose() {
        let a = "Hier ist die Antwort:\n\n## Andrew Johnson\n- **Andrew Johnson** war der 17. Präsident [1]\n- Amtszeit 1865 bis 1869\n\nMehr bei [Wikipedia](https://de.wikipedia.org/wiki/Andrew_Johnson).";
        assert_eq!(
            clean(a).unwrap(),
            "Andrew Johnson. Andrew Johnson war der 17. Präsident. Amtszeit 1865 bis 1869. Mehr bei Wikipedia."
        );
    }

    #[test]
    fn long_answers_end_at_a_sentence() {
        let a = "Ein Satz. ".repeat(200);
        let c = clean(&a).unwrap();
        assert!(c.len() <= MAX_ANSWER && c.ends_with('.'));
    }

    #[test]
    fn chat_carries_memory_only_when_given() {
        assert!(chat("de", "Mir geht es heute mies.", "name: Alex").contains("name: Alex"));
        let plain = chat("en", "I feel bad today.", "");
        assert!(plain.ends_with("I feel bad today.") && !plain.contains("context"));
    }

    #[test]
    fn empty_answers_are_none() {
        assert_eq!(clean("```\n```\n---"), None);
        assert_eq!(clean("  "), None);
    }

    #[test]
    fn a_real_answer_starting_like_a_preamble_stays() {
        assert_eq!(clean("Sure enough, the sky is blue because of Rayleigh scattering.").unwrap(), "Sure enough, the sky is blue because of Rayleigh scattering.");
    }
}
