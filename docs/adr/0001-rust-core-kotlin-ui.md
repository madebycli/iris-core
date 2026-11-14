# 1. A Rust core, a Kotlin UI

**Decision.** All logic lives in this Rust workspace. Kotlin keeps Compose screens and
the Android services, and implements the core's `Host` and `Store` traits. The two meet at a
UniFFI boundary (`iris-ffi`, not built yet): `process(text, lang) -> Reply`, `fired(id)`, and
a callback interface for the host.

**Why.** The Kotlin assistant grew into one 1 200-line class with language and features woven
together; 486 lines of Kotlin carried German literals. Tests needed Robolectric and minutes.
A core without Android compiles and tests in seconds on any machine, can serve a second
frontend, and is replaceable behind one interface.

**Consequences.** Two build systems. Android-only work (alarms, task apps) is split: the
decision in Rust, the act in Kotlin.
