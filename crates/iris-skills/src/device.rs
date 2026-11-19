//! The phone itself: volume, brightness, sound mode, Do Not Disturb, what is playing, a web search,
//! and the notifications of other apps (read them, answer one).
//!
//! The skill decides what was meant and what to say; the host only turns the knob. A change the
//! host can take back is handed to the engine, so "rückgängig" restores the old setting.

use iris_core::{Cx, DeviceAction, HostError, LevelChange, Notice, Reply, Skill, SoundMode};
use serde_json::json;

pub struct Device;

impl Skill for Device {
    fn manifest(&self) -> &'static str {
        include_str!("../data/device/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/device/de.toml")),
            "en" => Some(include_str!("../data/device/en.toml")),
            _ => None,
        }
    }

    fn act(&self, action: &str, args: &iris_core::actions::Args, cx: &mut Cx) -> Option<Reply> {
        let change = |name: &str| match name {
            "up" => Some(LevelChange::Up),
            "down" => Some(LevelChange::Down),
            "set" => Some(LevelChange::Set),
            "auto" => Some(LevelChange::Auto),
            _ => None,
        };
        let percent = args.int("percent").map(|p| p as u8);
        Some(match action {
            "device.volume" | "device.brightness" => {
                let change = change(args.text("change")?)?;
                // "set" needs the level: without it nothing is changed
                if change == LevelChange::Set && percent.is_none() {
                    return Some(Reply::failed(cx.lang.messages.get("action_unclear", &[])));
                }
                if action == "device.volume" {
                    let ring = args.flag("ring").unwrap_or(false);
                    change_device(cx, DeviceAction::Volume { ring, change, percent }, if ring { "ring_volume" } else { "media_volume" })
                } else {
                    change_device(cx, DeviceAction::Brightness { change, percent }, "brightness")
                }
            }
            "device.sound_mode" => {
                let mode = match args.text("mode")? {
                    "silent" => SoundMode::Silent,
                    "vibrate" => SoundMode::Vibrate,
                    _ => SoundMode::Ring,
                };
                change_device(cx, DeviceAction::SoundMode { mode }, "sound_mode")
            }
            "device.dnd" => change_device(cx, DeviceAction::DoNotDisturb { on: args.flag("on")? }, "dnd"),
            "search.web" => {
                let query = args.text("query")?.to_string();
                match cx.host.device(&DeviceAction::WebSearch { query: query.clone() }) {
                    Ok(_) => Reply::done(cx.msg("searched", &[("query", &query)])),
                    Err(_) => Reply::failed(cx.msg("no_browser", &[])),
                }
            }
            "music.playing" => playing(cx),
            "notification.read" => read_from(cx, args.text("from").unwrap_or_default()),
            "notification.reply" => start_reply_to(cx, args.text("from").unwrap_or_default(), args.text("message")?),
            _ => return None,
        })
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        match intent {
            "volume" => {
                let (change, percent) = level(cx, "level", false)?;
                let ring = cx.slots.get("ring").is_some();
                Some(change_device(cx, DeviceAction::Volume { ring, change, percent }, if ring { "ring_volume" } else { "media_volume" }))
            }
            "brightness" => {
                let (change, percent) = level(cx, "level", true)?;
                Some(change_device(cx, DeviceAction::Brightness { change, percent }, "brightness"))
            }
            "sound_mode" => {
                let said = cx.utt.norm_of(cx.span("mode"));
                let mode = if cx.list("silent").contains(&said) {
                    SoundMode::Silent
                } else if cx.list("vibrate").contains(&said) {
                    SoundMode::Vibrate
                } else {
                    SoundMode::Ring
                };
                Some(change_device(cx, DeviceAction::SoundMode { mode }, "sound_mode"))
            }
            "dnd" => {
                let on = cx.slots.get("off").is_none();
                Some(change_device(cx, DeviceAction::DoNotDisturb { on }, "dnd"))
            }
            "web_search" => {
                // from the raw text: fillers ("mal", "doch") belong to what is searched for
                let query = raw_tail(cx, "query").unwrap_or_else(|| cx.utt.orig_where(cx.span("query"), |_| true));
                Some(match cx.host.device(&DeviceAction::WebSearch { query: query.clone() }) {
                    Ok(_) => Reply::done(cx.msg("searched", &[("query", &query)])),
                    Err(_) => Reply::failed(cx.msg("no_browser", &[])),
                })
            }
            "send_message" => Some(Reply::failed(cx.msg("cannot_send", &[]))),
            "playing" => Some(playing(cx)),
            "notifications" => Some(read_notifications(cx)),
            "reply" => Some(start_reply(cx)),
            "reply.pick" => Some(pick(cx)),
            "reply.confirm" => confirm(cx),
            _ => None,
        }
    }
}

/// The level in the slot: "lauter", "auf 40 Prozent", "maximal", "automatisch". `None` when it is not a level.
fn level(cx: &Cx, slot: &str, auto: bool) -> Option<(LevelChange, Option<u8>)> {
    let span = cx.slots.get(slot);
    let words: Vec<&str> = match &span {
        Some(r) => cx.utt.tokens[r.clone()].iter().map(|t| t.norm.as_str()).collect(),
        None => Vec::new(),
    };
    let said = words.join(" ");
    let dir = cx.utt.norm_of(cx.span("dir"));
    for (name, change) in [("up", LevelChange::Up), ("down", LevelChange::Down)] {
        if cx.list(name).contains(&said) || (span.is_none() && cx.list(name).contains(&dir)) {
            return Some((change, None));
        }
    }
    if cx.list("max").contains(&said) {
        return Some((LevelChange::Set, Some(100)));
    }
    if cx.list("min").contains(&said) {
        return Some((LevelChange::Set, Some(0)));
    }
    if auto && cx.list("auto").contains(&said) {
        return Some((LevelChange::Auto, None));
    }
    // a number, maybe with "%" or the word for percent after it
    let first = words.first()?.trim_end_matches('%');
    let (value, used) = if first.chars().all(|c| c.is_ascii_digit()) && !first.is_empty() {
        (first.parse::<i64>().ok()?, 1)
    } else {
        cx.lang.number_at(&words, 0)?
    };
    let rest = &words[used..];
    let fine = rest.is_empty() || (rest.len() == 1 && cx.list("percent").iter().any(|p| p == rest[0]));
    (fine && (0..=100).contains(&value)).then_some((LevelChange::Set, Some(value as u8)))
}

fn change_device(cx: &mut Cx, action: DeviceAction, what: &str) -> Reply {
    match cx.host.device(&action) {
        Ok(result) => {
            if let Some(token) = result.restore {
                cx.undoable(token);
            }
            let key = match &action {
                DeviceAction::Brightness { change: LevelChange::Auto, .. } => "brightness_auto".to_string(),
                DeviceAction::SoundMode { mode } => format!("sound_mode.{}", serde_json::to_value(mode).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()),
                DeviceAction::DoNotDisturb { on } => if *on { "dnd_on" } else { "dnd_off" }.to_string(),
                _ if result.percent.is_some() => format!("{what}.percent"),
                _ => format!("{what}.changed"),
            };
            Reply::done(cx.msg(&key, &[("percent", &result.percent.map(|p| p.to_string()).unwrap_or_default())]))
        }
        // the host opened the screen where the permission is granted
        Err(HostError::Denied) => Reply::failed(cx.msg(if what == "brightness" { "allow_settings" } else { "allow_dnd" }, &[])),
        Err(HostError::Unavailable) => Reply::failed(cx.msg("unavailable", &[])),
        Err(HostError::Failed(_)) => Reply::failed(cx.msg("refused", &[])),
    }
}

/// The group `m` of the expression `name` on the raw text, as the user wrote it.
fn raw_tail(cx: &Cx, name: &str) -> Option<String> {
    let caps = cx.expr(name)?.captures(&cx.utt.raw)?;
    let text = caps.name("m")?.as_str().trim().trim_end_matches(['.', '!', '?']).trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn from(cx: &Cx) -> String {
    cx.slots.get("app").map(|r| cx.utt.orig_where(r, |_| true)).unwrap_or_default()
}

fn notices(cx: &Cx, from: &str) -> Result<Vec<Notice>, Box<Reply>> {
    cx.host.notifications(from).map_err(|_| Box::new(Reply::failed(cx.msg("allow_notifications", &[]))))
}

fn playing(cx: &mut Cx) -> Reply {
    match cx.host.now_playing() {
        Ok(Some(p)) if p.artist.is_empty() => Reply::done(cx.msg("playing", &[("title", &p.title)])),
        Ok(Some(p)) => Reply::done(cx.msg("playing_by", &[("title", &p.title), ("artist", &p.artist)])),
        Ok(None) => Reply::failed(cx.msg("playing_untitled", &[])),
        Err(_) => Reply::failed(cx.msg("no_player", &[])),
    }
}

fn read_notifications(cx: &mut Cx) -> Reply {
    let from = from(cx);
    read_from(cx, &from)
}

fn read_from(cx: &mut Cx, from: &str) -> Reply {
    let list = match notices(cx, from) {
        Ok(list) => list,
        Err(reply) => return *reply,
    };
    if list.is_empty() {
        return Reply::done(cx.msg("no_notifications", &[]));
    }
    let lines: Vec<String> = list.iter().take(10).map(|n| [n.title.as_str(), n.text.as_str()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(": ").chars().take(500).collect()).collect();
    Reply::done(lines.join("\n"))
}

fn start_reply(cx: &mut Cx) -> Reply {
    let app = from(cx);
    let message = raw_tail(cx, "message").unwrap_or_else(|| cx.slots.get("message").map(|r| cx.utt.orig_where(r, |_| true)).unwrap_or_default());
    start_reply_to(cx, &app, &message)
}

/// Answers a notification (of the app or conversation named `app`, or any): asks which when several fit and always asks
/// for a yes before anything is sent.
fn start_reply_to(cx: &mut Cx, app: &str, message: &str) -> Reply {
    let message = message.to_string();
    let app = app.to_string();
    let candidates: Vec<Notice> = match notices(cx, &app) {
        Ok(list) => list.into_iter().filter(|n| n.can_reply).take(20).collect(),
        Err(reply) => return *reply,
    };
    match candidates.len() {
        0 => Reply::failed(cx.msg("no_reply_target", &[])),
        1 => ask_confirm(cx, &candidates[0], &message),
        _ => {
            let labels: Vec<String> = candidates.iter().enumerate().map(|(i, n)| format!("{}. {}", i + 1, if n.title.is_empty() { &n.app } else { &n.title })).collect();
            cx.ask("reply.pick", json!({ "message": message, "notices": candidates }));
            Reply::question(format!("{}\n{}", cx.msg("which_conversation", &[]), labels.join("\n")))
        }
    }
}

fn ask_confirm(cx: &mut Cx, notice: &Notice, message: &str) -> Reply {
    let to = if notice.title.is_empty() { notice.app.clone() } else { notice.title.clone() };
    cx.ask("reply.confirm", json!({ "message": message, "key": notice.key }));
    Reply::question(cx.msg("confirm_reply", &[("to", &to), ("message", message)]))
}

fn pick(cx: &mut Cx) -> Reply {
    let notices: Vec<Notice> = serde_json::from_value(cx.state["notices"].clone()).unwrap_or_default();
    let message = cx.state["message"].as_str().unwrap_or_default().to_string();
    let words = cx.utt.norms();
    let chosen = (0..words.len()).find_map(|i| cx.lang.number_at(&words, i).map(|(n, _)| n)).and_then(|n| usize::try_from(n - 1).ok()).and_then(|i| notices.get(i).cloned());
    match chosen {
        Some(n) => ask_confirm(cx, &n, &message),
        None => {
            let state = cx.state.clone();
            cx.ask("reply.pick", state);
            Reply::question(cx.msg("pick_number", &[]))
        }
    }
}

/// `None` when the answer is neither yes nor no: then it is a new request, and the question is dropped.
fn confirm(cx: &mut Cx) -> Option<Reply> {
    if cx.lang.dialog.is_no(&cx.utt.norm) {
        return Some(Reply::done(cx.lang.messages.get("cancelled", &[])));
    }
    if !cx.lang.dialog.is_yes(&cx.utt.norm) {
        return None;
    }
    let key = cx.state["key"].as_str().unwrap_or_default().to_string();
    let message = cx.state["message"].as_str().unwrap_or_default().to_string();
    Some(match cx.host.reply_to(&key, &message) {
        Ok(()) => {
            cx.proceed();
            Reply::done(cx.msg("replied", &[]))
        }
        Err(HostError::Unavailable) => Reply::failed(cx.msg("reply_gone", &[])),
        Err(_) => Reply::failed(cx.msg("reply_failed", &[])),
    })
}
