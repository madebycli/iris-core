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
  move, snooze, repetition, destination Iris / task list / calendar / both), **lists** (named
  by content, several entries per sentence, "und noch …", guided with "Noch etwas?", tick,
  remove, empty, today's entries), **notes**, **alarm** (through the host's clock app, honest
  about receipts) and **clock** (time and date). 295 spec checks.
- `iris-brain`: the trained network in pure Rust (tract). Tokenizer, context text, inference,
  decoding; parity with the Kotlin runtime is tested against golden vectors. Implements
  `Classifier`, so the CLI and a station can use it without a native library.
- `iris-store`: a SQLite `Store` on the table of the Android app (desktop, station, watch).
- `iris-ffi`: UniFFI boundary (`IrisCore`, `Phone`); `build-android.sh <app-main-dir>`.
- `iris-cli`: terminal, fake phone, spec runner, `--brain`, `--db`.
- In the Iris app (repository `madebycli/iris`, branch `rust-core`): `CoreBridge.kt` implements
  `Phone` on the app's database, scheduler, clock app, CalDAV task list and device calendar;
  the destination of reminders is a setting; the core is behind **Settings › Services › New
  engine (experimental)**, off by default.
- Training: `nlu/brain/skill_templates.py` takes the `[training]` templates of the skills;
  `nlu/brain/gate.py` refuses a network that got worse than `nlu/data/baseline.json`.

Nothing here was run on a phone except what the first phone log showed (timers, reminders,
lists, notes). The Android side of alarms, CalDAV tasks and the calendar is only compiled and
tested through the fake host.

## Next, in this order

### 1. On a phone
Switch the core on, try every skill by voice. Every sentence that goes wrong becomes a line in
`specs/` before it is fixed. Check that alarms ring and that the release build's code shrinking
left the bridge intact (keep rules are in the app's `proguard-rules.pro`).

### 2. The core answers context follow-ups
The network needs the previous turn ("und morgen?", "die zweite"): pass `iris_brain::Context`
from the engine (what the last reply was about, what is shown, what is awaited) through
`Classifier::classify`. The Kotlin dialogue memory (`app/iris/dialog`, `BrainContext`) is the
reference. Until then the core's classifier tier only sees sentences without context.

### 3. Make the core the only path
Once task 1 holds on a phone: switch the core on by default, port `EndToEndTest` and
`FeatureStormTest` expectations that differ on purpose (bare hours, reply wording), then
delete `LocalCommand.kt`, `LocalActions.kt` and the own-feature branches of `Assistant.kt`.

### 4. The remaining skills, one at a time
Calendar queries, weather, Untis / school, anime tracker, device controls,
music, navigation, calls, memory, small talk, knowledge / Lumo hand-off. For each: specs from
the Kotlin tests first, then the skill, then delete the Kotlin.

### 5. Retraining from the skills
The generator and the gate exist. What is left: retrain with the skills' templates and see
whether the network passes `gate.py` (the round of 2026-11-12 needed five resumed trainings to
get from 34 regressions to 4: train longer from the start and balance examples per act), and add
`[training]` sections for notes, lists and reminders in new languages.

### 6. Reminders, the rest of the way
Listing and deleting reminders that live in the task app or calendar (needs `Host::tasks()` /
`Host::delete_task()`); an untimed "remind me to buy milk" offered as a list item; the default
time for date-only reminders as a setting.

### Later
Third-party skills as sandboxed WASM modules with signed manifests, on the same `Skill`
contract (Ari's `ari-skill-loader` is the reference).

## Known gaps in what exists

- The classifier tier passes no context yet (task 2).
- `Host::has` is not checked against a skill's declared capabilities at registration.
- Clock format is 24-hour in both languages; the app's 12/24 setting is not read.
- English contractions ("what's") are not expanded during normalisation.
- Timer names come from "what is left of the sentence"; unusual wordings can leave a stray word.
- The station and the watch do not exist yet; `iris-store` and `iris-brain` are what they need.
