# Retiring the Kotlin assistant

The Android app used to understand and carry out everything in Kotlin (`LocalCommands`,
`LocalActions`, the dialogue memory in `Assistant`). The core now answers first; this page says
what that covers, how it was checked, and what has to happen before the old code is deleted.

## What the core answers (and the old rules no longer see)

| Area | In the core | Checked against |
|---|---|---|
| Timers | set, named, until a clock time, step by step with a name, ask, cancel, several | Kotlin storm test round 6, end-to-end test |
| Reminders | one sentence or guided (time first), title extraction, lists, next, today, move, snooze, delete, repetition, destination Iris / task list / calendar | rounds 3, 11 |
| Lists | named by content, several entries, "und noch …", guided with "Noch etwas?", read (one list, all lists), tick, remove, empty (asks), today's entries | rounds 1, 2, 4, 9, 10 |
| Notes | add (all phrasings), guided, read, delete | rounds 5, 12 |
| Alarms | set (bare hour as said), next, list, honest about receipts | specs/alarm.spec |
| Time and date | spoken | specs/clock.spec |
| Dialogue | cancel in any words (also filler words), undo, say that again, do that again, conversation context for the network | round 7, 12, specs |

## What stays in Kotlin, and why

These are not "old code that was not moved yet", they are Android services the core reaches
through `Phone` or that have to run where the network and the account are:

- Oberfläche, Mikrofon, Spracherkennung, Sprachausgabe.
- Weather, calendar queries, Untis / school, anime tracker, device controls (flashlight, volume),
  music, navigation, calls, translation, lyrics, small talk personas, the online AI (Lumo,
  OpenRouter) and everything that needs one of them. Each is a candidate for a skill of the core
  with a `Host` call; none is needed to retire the *own-feature* rules.
- The follow-up acts that talk about what the screen shows (`op_list`, `op_pick`, `op_revise` for the
  timetable and weather dialogues): they use the Kotlin dialogue memory, which the core does not feed.

## How the loss was measured

`app/src/test/java/app/iris/ui/EndToEndTest.kt` (143 sentences, the real network) and
`FeatureStormTest.kt` (641 checks) run through the core with `-Piris.core=true`. Anything the
core does not claim goes to the Kotlin rules, as in the app. The first run showed 90 differences
in the storm test; each became a line in `specs/` before it was fixed.

## Steps before the Kotlin own-feature code is deleted

1. The core has run on a phone for a while (sentences that go wrong become specs).
2. `-Piris.core=true` runs of both suites are green and stay green in CI.
3. Delete `LocalCommands.kt`, `LocalActions.kt`, the own-feature branches of `Assistant.kt`
   (`byAct`, `fallback`, `guessKind`), `ReminderTask.kt`, `ReminderScheduler`'s planning
   decisions (the scheduler itself stays: it is the host's `schedule`), and the Kotlin tests that
   only exercised them (`FeatureStormTest` becomes a run through the core).
4. Remove the setting "Rust core" (there is nothing to fall back to).
