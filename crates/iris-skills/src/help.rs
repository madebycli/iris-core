//! "Befehle": a few examples of what Iris understands.

use iris_core::{Cx, Reply, Skill};

pub struct Help;

impl Skill for Help {
    fn manifest(&self) -> &'static str {
        include_str!("../data/help/skill.toml")
    }

    fn language(&self, code: &str) -> Option<&'static str> {
        match code {
            "de" => Some(include_str!("../data/help/de.toml")),
            "en" => Some(include_str!("../data/help/en.toml")),
            _ => None,
        }
    }

    fn handle(&self, intent: &str, cx: &mut Cx) -> Option<Reply> {
        if intent != "examples" {
            return None;
        }
        let examples = cx.list("examples");
        let shown = format!("{}\n{}\n{}", cx.msg("intro", &[]), examples.iter().map(|e| format!("• {e}")).collect::<Vec<_>>().join("\n"), cx.msg("outro", &[]));
        // read aloud as sentences, without the bullets
        let spoken = format!("{} {}. {}", cx.msg("intro", &[]), examples.join(". "), cx.msg("outro", &[]));
        let mut reply = Reply::done(shown);
        reply.speech = Some(spoken);
        Some(reply)
    }
}
