# 6. A narrow JSON boundary; on Android the host owns storage

**Decision.** `iris-ffi` exposes one object (`IrisCore`: `process`, `fired`, `awaits_answer`,
`languages`) and one callback interface (`Phone`). Structured values cross as JSON in the
shapes `iris-core` serialises. On Android the `Store` is the app's existing SQLite table,
reached through `Phone`; the trained network is reached the same way until it runs in Rust.

**Why.** Generated bindings with one object and strings do not change when a field is added.
The app's screens read the entries table directly and observe it; a second SQLite library
writing the same file from the same process is a documented way to corrupt it, so there is
exactly one writer library per platform. A desktop frontend supplies its own `Store`.

**Consequences.** Every store call from a skill crosses the boundary. Entry counts are in the
hundreds, so this is not a cost that matters; it was not measured on a phone.
