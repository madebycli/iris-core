# Rust core: where it stands and what comes next

Read [`ARCHITECTURE_CORE.md`](ARCHITECTURE_CORE.md) and the decisions in [`adr/`](adr) first.
The rules of the house: a behaviour is a spec line before it is code; no word of a language in
Rust; nothing is "done" without `cargo test` and `cargo clippy --all-targets` clean; the Kotlin
counterpart of a feature is deleted only after its sentences pass in the core.

## Done

- `iris-core`: pipeline with four tiers, language packs (de, en), time expressions, `Skill` /
  `Host` / `Store` contracts, replies with cards, dialogue state as data, small per-skill
  memory, repeating entries.
- `iris-skills`: **timer**, **reminder** (one sentence or guided, list, next, per day, delete,
  repetition, destination Iris / task app / calendar / both), **lists** (named by content,
  several entries per sentence, "und noch …", guided with "Noch etwas?", tick, remove, empty,
  today's entries) and **notes**. 241 spec checks and 6 pipeline tests pass.
- `iris-ffi`: UniFFI boundary (`IrisCore`, `Phone`). `build-android.sh` builds the
  arm64 library and the Kotlin bindings; Gradle runs it before every build (task `irisCore`),
  so building the app needs a Rust toolchain with `aarch64-linux-android` and `cargo-ndk`.
- In the app: `app/iris/core/CoreBridge.kt` implements `Phone` on the app's own database,
  scheduler and network. Behind the switch **Settings › Services › New engine (experimental)**,
  off by default, the core answers first and the Kotlin assistant takes what it leaves.
  `CoreBridgeTest` proves the round trip on the JVM. **Nothing was run on a phone.**

Decided differently than first planned: there is no SQLite store in Rust for Android; the
app's table is the store (see `adr/0006`).

## Next, in this order

Each task is self-contained. "Source" is the Kotlin that defines today's behaviour.

### 1. On a phone
Switch the core on, try timers, reminders, lists and notes by voice. Every sentence that goes
wrong becomes a line in `specs/` before it is fixed. Check that alarms ring (the core
plans them through `ReminderScheduler`) and that the release build's code shrinking left the
bridge intact (keep rules are in `app/proguard-rules.pro`).

### 2. Task app and calendar on Android
`AndroidPhone.taskLists / createTask / createEvent` return nothing yet. Connect them to
`TasksOrg.kt` / `CalDavTasks.kt` and `CalendarService.kt`, and add the setting
`reminder.destination` (local, tasks, calendar, both) to the settings screen. Until then a
reminder always stays in Iris.

### 3. The network in Rust (`iris-brain`)
Source: `app/src/main/java/app/iris/brain/*.kt` (tokenizer 52 lines, context features,
runtime), assets `brain/pieces.tsv`, `brain/meta.txt`; parity vectors
`app/src/test/resources/brain/golden.tsv`.
Port the tokenizer and the input features, run the ONNX file with the `ort` crate, implement
`Classifier`. The follow-up acts (`op_*`) need the previous turn: extend `Classifier::classify`
with a context argument and have the engine pass what the last reply was about.
Done when: the golden vectors match and `nlu/brain/eval.py`'s sets score as the Kotlin
runtime does. `brain.onnx` is never committed.

### 4. Make the core the only path
Once task 1 holds on a phone: switch the core on by default, port `EndToEndTest` and
`FeatureStormTest` expectations that differ on purpose (bare hours, reply wording), then
delete `LocalCommand.kt`, `LocalActions.kt` and the own-feature branches of `Assistant.kt`.

### 5. The remaining skills, one at a time
Alarms (`HalfDay::AsSaid`), calendar, weather, Untis / school, anime tracker, device controls,
music, navigation, calls, memory, small talk, knowledge / Lumo hand-off. For each: specs from
the Kotlin tests first, then the skill, then delete the Kotlin.

### 6. Training from the skills
`nlu/brain/build_data.py` reads each skill's `[training]` templates instead of `gen4.py`'s
hand-written ones; `eval.py` becomes a gate (no act may lose more than its noise margin
against the shipped network). The round of 2026-11-12 needed five resumed trainings to get
from 34 regressions to 4: train longer from the start and balance examples per act.

### 7. Reminders, the rest of the way
Snooze and "move it to six"; listing and deleting reminders that live in the task app or
calendar (needs `Host::tasks()` / `Host::delete_task()`); an untimed "remind me to buy milk"
offered as a list item; the default time for date-only reminders as a setting.

### Later
Third-party skills as sandboxed WASM modules with signed manifests, on the same `Skill`
contract (Ari's `ari-skill-loader` is the reference).

## Known gaps in what exists

- The classifier tier passes no context yet (task 3).
- `Host::has` is not checked against a skill's declared capabilities at registration.
- Clock format is 24-hour in both languages; the app's 12/24 setting is not read.
- English contractions ("what's") are not expanded during normalisation.
- Timer names come from "what is left of the sentence"; unusual wordings can leave a stray word.
