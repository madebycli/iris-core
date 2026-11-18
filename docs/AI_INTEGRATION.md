# The online AI in Iris: what it gets, what it may change

The local network decides what a sentence is. The online AI (Lumo, OpenRouter, a custom server or Venice) is
asked only where it adds something, and it never changes the phone or the user's data on its own: the core
checks every proposal.

## What is worth a paid request (`iris-nlu`, `fallback::worth_cloud`, `fallback::conversation`)

- Clear local commands take the local path. Complex instructions can use the action planner when allowed by the routing level and the online consent settings.
- A knowledge question goes to the AI.
- A sentence the network could not place and that is no real question is mostly noise from the speech
  recogniser: Iris asks back locally, which costs nothing.
- Set phrases ("hi", "danke", "wie geht's") keep Iris's own answers; real conversation of four words or more
  ("ich fühle mich heute mies, weil ...") goes to the AI with the memory (setting "Conversations").

## Action planning across the Core and Android

The four routing levels express how readily Iris asks for interpretation. Android stores the choice
and presents the slider. The Core owns the routing decision, the vocabulary of actions and the
validation of the reply. Changing levels grants no additional device or network permission.

1. Resolve the current dialog first. When the user dismisses an unfinished request, discard its
   outstanding question. A dismissal of a timer setup must leave existing timers alone.
2. For a direct local command, use the existing skill. For a planning turn, intersect the relevant
   action families with the operations the host supports. Send the current utterance and the
   selected argument contract through the assistant provider. Credentials, HTTP and web sessions
   belong to the client. Research answers never enter the action dispatcher.
3. Validate the entire proposal before any side effect. A clock time, a duration and a timestamp
   are separate values. Resolve dates against a single turn clock and the host time zone.
   A clarification is a dialog turn. It cannot also create an entry.
4. Dispatch validated actions to the host and report their individual outcomes. An Android intent
   that hands off to another app may acknowledge the handoff only. Avoid replaying an action when
   its outcome is uncertain.

A future home client supplies its own clock, capabilities, provider transport and executors. It
can reuse the same Core policy and proposal validation. Introducing a capability requires a typed
contract, a validator, an executor and tests for unavailable and rejected operations.

### The contract, version 1 (`iris-core::plan`)

**Levels.** `plan::route(level, text, lang, open_question)` returns `local`, `talk` or `plan`. Level 1 (sparing) decides
nothing here; the older routing (an unplaced sentence, knowledge questions, follow-ups) applies. Level 2 (balanced) hands
over questions about earlier turns ("warum", "wann wollte ich") as `talk` and long or several-task sentences as `plan`;
level 3 (smart) also every sentence of nine words or more that is not a simple command; level 4 (AI first) everything
except simple commands. A request put as a question ("kannst du eine Erinnerung erstellen?") is a command, so it is
`plan`, not `talk`. An answer to an open question, a simple command ("Timer auf 5 Minuten", "Wie spät ist es") and a
sentence of two words or fewer are always `local`. The words are data in `core/lang/plan/<code>.toml`.

**The prompt.** `plan::prompt(lang, now_ms, zone, families)` gives the assistant the contract for the action families
the host can carry out now (`Engine::plan_families`: reminders and timers need scheduling, alarms need the clock app,
lists and notes live in the store) and the turn's clock (weekday, date, time, zone). The host sends this text and the
current sentence and nothing else: no earlier turns, entries, contacts, calendar or memory. A follow-up
("nein, früher") is a question about earlier turns and is routed as `talk`, not planned. The host reports what it can
do through `Phone::has_capability` (Android: the clock app answers the alarm intent, a task app is set up); a family it
cannot carry out is left out of the prompt and rejected in a reply. If the host can carry
out no action, or the language has no planner words, there is no prompt and the sentence stays local.

**One clock per turn.** The clock is fixed when the prompt is made (`plan_prompt` keeps it) and the answer is checked
against that same instant (`plan_validate` takes it; after ten minutes, or if the phone's clock went backwards, the
current time is used). A relative word the assistant turned into a fixed date is therefore judged against the clock it
was given.

**The reply.** One JSON object, in one of three forms:

```json
{"version":1,"actions":[ {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
                         {"type":"alarm.set","time":"15:40","date":"2026-11-19"} ]}
{"version":1,"ask":{"slot":"time"}}
{"version":1,"not_action":true}
```

Actions: `reminder.create {title,date,time}`, `alarm.set {time,date?}`, `timer.set {seconds,label?}`,
`list.add {list,item}`, `note.add {text}`. `date` is `YYYY-MM-DD`, `time` is `HH:MM`; a duration only ever appears as
`timer.set.seconds`.

**Validation (`plan::validate`).** Everything or nothing: one bad action rejects the whole proposal, and nothing is
carried out before it is accepted. Rejected: another `version`, an unknown action type or unknown field, more than
5 actions, a time or date that is not exactly `HH:MM` / `YYYY-MM-DD`, a date in the past or more than a year ahead, a
reminder or dated alarm whose moment is not more than a minute ahead of the turn's clock in the host's zone (it would
be past, or be read as the next day's, by the time it is carried out), an action of a family the host cannot carry out
(`unsupported`), a timer outside 1 s to
24 h, and text that is empty, longer than 80 characters (note: 200), or holds control characters, sentence punctuation
or brackets, so that a title cannot smuggle a second command into a sentence. An `ask` takes one of the slots `time`,
`date`, `duration`, `title`, `list`; its question is the core's own sentence, not the assistant's text, and it never
comes with actions. `not_action` goes back to the assistant as a text question.

**Execution.** Each valid action becomes a sentence in the engine's own grammar ("Erinnere mich morgen um 16:00 Uhr an
Schuhhaus Müller"), in the user's language. The host says the sentences to `process` one after the other, so the
existing skills create the entries and report their own outcomes (an alarm handed to the clock app is acknowledged as
handed over, not as set). If a sentence opens a question, the host stops and lists the rest as still open. The host
never resends the assistant's request after an uncertain outcome.

**Dismissing a question.** `Engine::cancel_question` drops the open question and nothing else: running timers and
existing entries stay, and nothing is created from the half-finished request.

**Calendar and task apps.** A reminder reaches the user's task list or calendar through the reminder skill's own
destination setting (`Capability::Tasks` / `Calendar`), so `reminder.create` already covers them; the contract has no
separate calendar or task action. A calendar appointment with a duration needs its own typed action, which is not
part of version 1.

**Adding a capability** needs a typed action, a validator branch, a sentence template per language, an integration test
against the real engine (`crates/iris-cli/tests/plan.rs`) and tests for rejected and unavailable operations.

## Tidying up entries (`iris-cloud::organize`)

After the core created a reminder, task, note or list item, the AI may propose a clean title ("ich muss noch um
16:30 mit dem Hund raus" becomes "Mit dem Hund rausgehen") and, for list items, a list. It gets the sentence, the
new entries and all list names as JSON, plus the user's own rules. "Ordne meine Erinnerungen / Aufgaben / Listen"
sends all open entries of that kind. The answer must be one JSON object; the core keeps only:

- entries of this request, and only title and list (dates, times, repetitions, deletions are not in the template);
- titles without clock times or dates, at most 80 characters;
- existing lists, or new ones only when allowed; a list the user named in the sentence stays.

By default Iris reads the proposal and changes nothing before a yes; without confirmation it applies it and says
what it saved ("undo" still works). If the AI fails, the entry stays as it was said.

## Memory (`iris-cloud::memory`)

Modelled on LibreChat's key/value memory and Letta's bounded blocks: at most 40 named entries ("name: Ada") of
140 characters, visible and deletable on the Memory screen, a switch that stops sending without deleting.
Earlier "- fact" lists are taken over as fact_1, fact_2, ...

- Sent once per conversation session (a session ends after 30 minutes without a question to the AI, on a
  provider change or when the memory settings change). Lumo's own chat gets it in its setup message; stateless
  APIs get it as one pinned context message at the start of the session's history, never inside each request text.
- With "Learn from conversations" (on by default) the AI may add `"memory": "set key: value; forget key"` to a
  reply. At most three changes per reply are applied, after checks; Iris says what it noted.
- "Weißt du noch, wie ich heiße?" is answered from the memory on the phone, without a request.

## Research AI (`iris-cloud::research`)

An optional second AI only for knowledge questions ("Wer war der 17. Präsident der USA?"), typically a stronger
model than the everyday assistant (for example Lumo Max for research, a small Venice model for conversation and tidying up).

- The research model gets `research::prompt`: a short instruction (three or four sentences of plain prose, no
  introduction, lists, links or citations, say so when unsure) and the bare question. No app rules, no memory, no history.
- Its answer goes through `research::clean` (Markdown, lists, links, citation marks and "Hier ist die Antwort:" removed,
  cut at a sentence end) and is shown and read aloud instead of an answer from the assistant AI.
- The assistant AI gets only a note in its local history that the user looked the question up; no extra request is made.
- A follow-up ("und der 18.?") goes to the research AI again with the earlier question attached.
- When the research AI fails, Iris says so; it does not quietly ask the weaker assistant model instead.

## Lumo is a web page, not an API

Lumo is driven through its web chat. Since late 2026 Lumo refuses a chat message that installs another assistant's
protocol or persona ("Ich bin Lumo und bleibe Lumo"), so Iris no longer sends a rules message or a JSON protocol to Lumo:

- Conversation turns are plain requests in the user's voice (`research::chat`); the memory goes along once, in a chat's
  first message, as background the user gave. Lumo cannot propose memory entries or phone actions; the local assistant
  handles those.
- Knowledge questions use the research prompt above; Lumo's web search is switched on for them where offered.
- Tidying up entries asks for one JSON code block; the answer is checked by `organize::apply` as for every provider.
  Lumo thinks before answering (about 18 s measured), so this waits up to 40 s instead of 12 s.
- Proton's modals are `<dialog>` elements without a role; the page script recognises them, and uses the visible
  "New chat" button rather than the copy in the collapsed sidebar.
