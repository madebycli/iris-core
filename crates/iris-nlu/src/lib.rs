//! What Iris reads from a sentence once the network has named the function: the day, the clock time,
//! a duration, a place, a name; what is said back when the network is not sure; and which readings are
//! offered to tap. Pure functions: no device, no clock of their own (`today` is passed in).
//!
//! Ported from the Android app, where these were Kotlin. The patterns keep their wording; they run with
//! Unicode semantics, as they did on the phone (ICU), not as on a desktop JVM.

pub mod anime;
pub mod clarify;
pub mod date;
pub mod duration;
pub mod fallback;
pub mod intent;
pub mod language;
pub mod normalize;
pub mod place;
pub mod re;
pub mod router;
pub mod slots;
pub mod time;

pub use normalize::normalize;
