//! Notes: write one down, read them back.

use crate::{join, strip};
use iris_core::{Card, Cx, Entry, EntryKind, Reply, Skill};
use serde_json::json;

pub struct Notes;

impl Skill for Notes {
    fn manifest(&self) -> &'static str {
        include_str!("../data/notes/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/notes/de.toml")),
            "en" => Some(include_str!("../data/notes/en.toml")),
            _ => None,
        }
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        match intent {
            "add" => {
                let tokens: Vec<usize> = (0..cx.utt.len()).collect();
                let kept = strip(cx.utt, tokens, cx.list("drop"), cx.list("lead"), cx.list("trail"));
                let norm = kept.iter().map(|i| cx.utt.tokens[*i].norm.as_str()).collect::<Vec<_>>().join(" ");
                if kept.is_empty() || cx.list("placeholder").contains(&norm) {
                    cx.ask("add.text", json!({}));
                    return Some(Reply::question(cx.msg("ask_text", &[])));
                }
                Some(save(cx, join(cx.utt, &kept)))
            }
            "add.text" => {
                let all: Vec<usize> = (0..cx.utt.len()).collect();
                Some(save(cx, join(cx.utt, &all)))
            }
            "read" => {
                let items = cx.store.all(Some(EntryKind::Note));
                if items.is_empty() {
                    return Some(Reply::done(cx.msg("none", &[])));
                }
                let titles = items.iter().map(|e| e.title.clone()).collect::<Vec<_>>().join(", ");
                Some(Reply::done(cx.messages.plural("read", items.len() as i64, &[("items", &titles)])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items }))
            }
            "remove" | "remove.which" => Some(remove(cx, intent == "remove.which")),
            "clear.confirm" => Some(clear_confirmed(cx)),
            _ => None,
        }
    }
}

fn remove(cx: &mut Cx, answering: bool) -> Reply {
    let notes = cx.store.all(Some(EntryKind::Note));
    if notes.is_empty() {
        return Reply::done(cx.msg("none", &[]));
    }
    let said = cx.utt.norm.clone();
    if cx.utt.tokens.iter().any(|t| cx.list("all").contains(&t.norm)) {
        cx.ask("clear.confirm", json!({}));
        return Reply::question(cx.messages.plural("confirm_all", notes.len() as i64, &[]));
    }
    // the longest note whose words are in the sentence
    let hit = notes.iter().filter(|n| said.contains(&n.title.to_lowercase())).max_by_key(|n| n.title.len()).cloned();
    match hit {
        Some(note) => {
            cx.store.delete(&note.id);
            Reply::done(cx.msg("removed", &[("title", &note.title)]))
        }
        None if answering => Reply::failed(cx.msg("not_found", &[])),
        None => {
            cx.ask("remove.which", json!({}));
            Reply::question(cx.msg("ask_which", &[])).with_card(Card::Entries { title: cx.msg("card_title", &[]), items: notes })
        }
    }
}

fn clear_confirmed(cx: &mut Cx) -> Reply {
    if !cx.lang.dialog.is_yes(&cx.utt.norm) {
        return Reply::done(cx.msg("kept", &[]));
    }
    let notes = cx.store.all(Some(EntryKind::Note));
    for n in &notes {
        cx.store.delete(&n.id);
    }
    Reply::done(cx.messages.plural("deleted_all", notes.len() as i64, &[]))
}

fn save(cx: &mut Cx, text: String) -> Reply {
    let now = cx.now_ms();
    let text: String = text.chars().take(500).collect();
    let entry = Entry::new(cx.new_id(), EntryKind::Note, text.clone(), now, &cx.host.zone());
    cx.store.put(entry);
    Reply::done(cx.msg("added", &[("text", &text)]))
}
