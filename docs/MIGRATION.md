# Retiring the Kotlin assistant

The Android app used to understand and carry out everything in Kotlin (`LocalCommands`,
`LocalActions`, the dialogue memory in `Assistant`). Those rules are gone: the core answers
every sentence first, and there is no switch back. This page says what the core covers, how the
move was checked, and what stays on the Android side on purpose.

## What the core answers

| Area | In the core | Checked against |
|---|---|---|
| Timers | set, named, until a clock time, step by step with a name, ask, cancel, several | Kotlin storm test round 6, end-to-end test |
| Reminders | one sentence or guided (time first), title extraction, lists, next, today, move, snooze, delete, repetition, destination Iris / task list / calendar | rounds 3, 11 |
| Lists | named by content, several entries, "und noch …", guided with "Noch etwas?", read (one list, all lists), tick, remove, empty (asks), today's entries | rounds 1, 2, 4, 9, 10 |
| Notes | add (all phrasings), guided, read, delete | rounds 5, 12 |
| Alarms | set (bare hour as said), repeating (daily, weekdays, weekly), next, list, dismiss the next (asks first), honest about receipts | specs/alarm.spec, specs/device.spec |
| Time and date | spoken | specs/clock.spec |
| The phone | volume (media, ring), brightness, sound mode, Do Not Disturb, what is playing, a web search; each change can be undone | specs/device.spec |
| Notifications | read (all, or of one app), answer one (pick by number, confirm first) | specs/device.spec |
| Stopwatch | start, stop, reset, read; keeps running while the app is closed | specs/device.spec |
| Units | length, mass, volume, speed, time, temperature; numbers in words, decimal comma | specs/device.spec |
| Help | "Befehle": examples of what Iris understands | specs/device.spec |
| Dialogue | cancel in any words (also filler words), undo, say that again, do that again, a yes/no question that is answered with a new request, conversation context for the network | round 7, 12, specs |

The last group (the phone, notifications, stopwatch, units, help) was the old rules' fallback for
sentences the core did not claim. It moved into the skills `device`, `stopwatch`, `convert` and
`help` and the host calls `device`, `restore`, `now_playing`, `notifications`, `reply_to` and `keep`.

## What stays in Kotlin, and why

These are Android services, or they have to run where the network and the account are:

- The interface, microphone, speech recognition, speech output.
- How the host carries out what the core decided: `AndroidPhone` in the app turns the volume knob,
  opens the clock app, reads the notification listener, keeps the values the core asks it to keep.
- Weather, calendar queries, Untis / school, anime tracker, flashlight, music controls, navigation,
  calls, translation, lyrics, small talk personas, the online AI (Lumo, OpenRouter) and everything
  that needs one of them. Each is a candidate for a skill of the core with a `Host` call.
- The follow-up acts that talk about what the screen shows (`op_list`, `op_pick`, `op_revise` for the
  timetable and weather dialogues): they use the Kotlin dialogue memory, which the core does not feed.

## How the loss was measured

`app/src/test/java/app/iris/ui/EndToEndTest.kt` (the real network) and `FeatureStormTest.kt`
(641 checks) run through the core. The first run showed 90 differences in the storm test; each
became a line in `specs/` before it was fixed. Sentences the old fallback grammar knew (volume,
brightness, sound mode, stopwatch, units, notifications, web search, help, repeating and dismissed
alarms) became `specs/device.spec` before the Kotlin code was deleted.

## When a sentence goes wrong on a phone

Write it as a case in `specs/`, see it fail with `cargo test`, then fix the skill's patterns or
logic. Do not add rules to the app.
