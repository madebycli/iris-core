//! The Iris engine.
//!
//! One utterance goes in, one [`Reply`] comes out. Nothing here knows Android: time, storage,
//! alarms, task apps and calendars are reached through the [`Host`] and [`Store`] traits, so the
//! whole assistant runs and is tested on a desktop.
//!
//! No word of any spoken language is written in this crate's code. Language lives in
//! `core/lang/<code>.toml` ([`LangPack`]) and in each skill's own language files.

pub mod engine;
pub mod entry;
pub mod host;
pub mod lang;
pub mod reply;
pub mod skill;
pub mod store;
pub mod text;
pub mod time;

pub use engine::{Classifier, Engine, NoClassifier, Prediction};
pub use entry::{Entry, EntryKind};
pub use host::{AlarmRequest, Capability, ExternalEvent, ExternalList, ExternalTask, Host, HostError};
pub use lang::{LangPack, Messages};
pub use reply::{Card, Reply, ReplyKind};
pub use skill::{Cx, Skill, SkillError, Slots};
pub use store::{MemoryStore, Store};
pub use text::Utterance;
