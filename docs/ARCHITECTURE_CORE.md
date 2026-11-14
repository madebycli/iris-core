# The Rust core

Everything Iris does that is not drawing a screen is moving into this Rust workspace. The Android app keeps the UI and the things only Android can do
(microphone, speech, notifications, alarms, permissions, other apps) and offers them to the
core as a *host*. The core runs unchanged on a desktop, which is how it is tested.

```
.
├── lang/<code>.toml              what the engine knows about a language
├── specs/*.spec                  conversations that must hold (cargo test)
├── specs/pending/*.spec          conversations wanted, not built yet (reported, not failing)
└── crates/
    ├── iris-core                 pipeline, language packs, time, Skill / Host / Store contracts
    ├── iris-skills               the built-in skills: Rust for logic, data/<skill>/ for words
    └── iris-cli                  `iris` in a terminal, the spec runner, a fake phone
```

## How an utterance is answered

`Engine::process(text, language)` returns one `Reply`.

1. **Open question.** A skill that asked something gets the next utterance, with the state it
   left as plain data. A cancel word of the language ends the question, whatever the skill.
   A question expires after two minutes.
2. **Grammar.** Each skill's patterns for the language are tried in order of skill priority.
   Named groups become slots. Exact, instant, no model.
3. **Classifier.** The trained network ("Iris Brain") names an act. If it is at least 0.60
   sure, the skill whose manifest declares that act runs.
4. **Unhandled.** The reply says so, and the app may hand the sentence to its language model.

A skill can decline at every step by returning `None`; the next candidate is tried.

The classifier stays the router for free wording because it was measured: on held-out
sentences the trained network finds the right act about 95 % of the time, example-phrase
matching with the untrained encoder 50 % at ten examples per act
(see [`adr/0003`](adr/0003-trained-classifier-stays-the-router.md)).

## A skill

A skill is a Rust type implementing `Skill` plus a directory of data:

```
data/reminder/
├── skill.toml        id, priority, capabilities, intents and the acts that mean them
├── de.toml           patterns, word lists, expressions, rewrites, messages, training templates
└── en.toml
```

The Rust code contains no word of any language. It asks its language file for word lists
(`cx.list("lead")`), expressions (`cx.expr("list_hint")`) and messages (`cx.msg("created", …)`).
**Adding a language to a skill is adding one file.** Adding a language to the engine is adding
`lang/<code>.toml` (numbers, time words, cancel/yes/no words, formats).

A skill reaches the world only through `Cx`: the utterance (normalised tokens that remember
their original spelling), the `Store`, the `Host`, the clock. It keeps no state of its own;
what it needs across turns it hands to `cx.ask(intent, state)` as JSON, so a question
survives the app being killed.

## Time

`time::scan` finds time words in tokens using the language pack and reports which tokens it
used; the rest is the title. `time::resolve` turns the result into an instant. The two are
separate so half a request ("at five" … "about what?") is plain data.

A bare hour ("um fünf", "at 8") means the next time a clock shows it; with a named day, hours
one to six mean the afternoon; a routine ("every day at 7") is taken as said. The reply always
names the full time, so a wrong guess is visible at once
(see [`adr/0004`](adr/0004-bare-hours.md)).

## Host and Store

`Host` is the phone: clock, zone, settings, scheduling an alert for an entry, the task app,
the calendar. `Store` is where entries live. Both are traits; Android implements them, the
CLI and the tests use `FakeHost` and `MemoryStore`. A skill declares the capabilities it
needs in its manifest.

`Entry` has the same fields as the Android app's `entries` table, so existing reminders,
timers, notes and lists carry over.

## Tests

`specs/*.spec` are conversations in plain text, run by `cargo test`:

```
## guided: time first, then the name
> Erstelle eine Erinnerung
< Uhr
> um 16:30 Uhr
> Hund rausbringen
= reminder "Hund rausbringen" 2026-11-10T16:30
```

The format is documented at the top of [`crates/iris-cli/src/lib.rs`](../crates/iris-cli/src/lib.rs).
A sentence that goes wrong on a phone becomes a spec line first, then a fix.

```bash
cd core
cargo test                       # every spec and unit test
cargo run -p iris-cli -- "Erinnere mich um fünf an den Müll"
cargo run -p iris-cli -- --lang en "set a pasta timer for 8 minutes"
cargo run -p iris-cli -- spec specs/pending/lists_de.spec
```

## What the core borrows

The shape of the skill contract — one trait, a manifest with patterns, capabilities checked
against what the host offers, replies as data the frontend renders, a language file per
skill — follows [Ari](https://github.com/ari-digital-assistant) (GPL-3.0 engine, see
`NOTICE.md`). No Ari code is included. Routing, the time parser, the dialogue state and the
skills are Iris' own.
