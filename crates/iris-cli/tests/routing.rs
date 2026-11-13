//! The pipeline's tiers, with a stand-in for the trained network.

use iris_cli::Session;
use iris_core::{Classifier, EntryKind, Prediction, ReplyKind, Store};

struct Says(&'static str, f32);

impl Classifier for Says {
    fn classify(&self, _text: &str, _lang: &str) -> Vec<Prediction> {
        vec![Prediction { label: self.0.to_string(), p: self.1 }]
    }
}

const NOW: i64 = 1_794_322_800_000; // 2026-11-10 16:00 Europe/Berlin

#[test]
fn a_confident_act_runs_the_skill_that_declared_it() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    s.engine.set_classifier(Box::new(Says("timer_set", 0.93)));
    // no pattern of the timer skill matches this sentence
    let reply = s.say("Sag in 10 Minuten Bescheid wenn die Eier fertig sind, also so ein Küchenwecker");
    assert_eq!((reply.skill.as_str(), reply.intent.as_str()), ("timer", "set"), "{}", reply.text);
    assert_eq!(s.store.all(Some(EntryKind::Timer)).len(), 1);
}

#[test]
fn an_unsure_act_is_not_acted_on() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    s.engine.set_classifier(Box::new(Says("timer_set", 0.41)));
    assert_eq!(s.say("Mach mal irgendwas mit zehn Minuten").kind, ReplyKind::Unhandled);
    assert!(s.store.all(None).is_empty());
}

#[test]
fn grammar_wins_over_the_network() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    s.engine.set_classifier(Box::new(Says("timer_set", 0.99)));
    assert_eq!(s.say("Erinnere mich um 17 Uhr an Oma").skill, "reminder");
}

#[test]
fn a_question_expires() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    assert_eq!(s.say("Erstelle eine Erinnerung").kind, ReplyKind::Question);
    s.host.0.lock().unwrap().now_ms += 5 * 60_000;
    // five minutes later this is a new sentence, not the answer
    assert_eq!(s.say("um 16:30 Uhr").kind, ReplyKind::Unhandled);
    assert!(s.store.all(None).is_empty());
}

#[test]
fn a_repeating_reminder_is_planned_again_when_it_fires() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    s.say("Erinnere mich täglich um 7 Uhr an die Tabletten");
    let first = s.store.all(Some(EntryKind::Reminder)).remove(0);
    s.host.0.lock().unwrap().now_ms = first.due_at.unwrap() + 1000;
    s.engine.fired(&first.id);
    let again = s.store.get(&first.id).unwrap();
    assert!(again.pending);
    assert_eq!(again.due_at.unwrap() - first.due_at.unwrap(), 24 * 3_600_000);
}

#[test]
fn a_single_reminder_is_finished_when_it_fires() {
    let mut s = Session::new(NOW, "Europe/Berlin", "de");
    s.say("Erinnere mich um 17 Uhr an Oma");
    let entry = s.store.all(Some(EntryKind::Reminder)).remove(0);
    s.engine.fired(&entry.id);
    assert!(!s.store.get(&entry.id).unwrap().pending);
}
