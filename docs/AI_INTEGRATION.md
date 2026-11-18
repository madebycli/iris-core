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
2. For a direct local command, use the existing skill: reachability is not routing, a plain command
   stays local at every level. For a planning turn, select the tools this request needs (all parts of
   a mixed order) and intersect them with the operations the host supports. Send the current
   utterance and the selected argument contract through the assistant provider. Credentials, HTTP and web sessions
   belong to the client. Research answers never enter the action dispatcher.
3. Validate the entire proposal before any side effect. A clock time, a duration and a timestamp
   are separate values. Resolve dates against a single turn clock and the host time zone.
   A clarification is a dialog turn. It cannot also create an entry.
4. Dispatch validated actions to their executor with the checked values (an action id and typed
   arguments, never a sentence to interpret again) and report their individual outcomes. An Android
   intent that hands off to another app may acknowledge the handoff only. Avoid replaying an action
   when its outcome is uncertain.

A future home client supplies its own clock, capabilities, provider transport and executors. It
can reuse the same Core policy and proposal validation. Introducing a capability requires a typed
contract, a validator, an executor and tests for unavailable and rejected operations.

### The contract, version 2 (`iris-core::actions`, `iris-core::plan`)

**Levels.** `plan::route(level, text, lang, open_question, direct)` returns `local`, `talk` or `plan`. Level 1 (sparing)
decides nothing here; the older routing (an unplaced sentence, knowledge questions, follow-ups) applies. Level 2
(balanced) hands over questions about earlier turns ("warum", "wann wollte ich") as `talk` and long or several-task
sentences as `plan`; level 3 (smart) also every sentence of nine words or more that is not a simple command; level 4
(AI first) everything except simple commands. A request put as a question ("kannst du eine Erinnerung erstellen?") is a
command, so it is `plan`, not `talk`. **Reachability is not routing:** an answer to an open question, a simple command
("Timer auf 5 Minuten"), a sentence of two words or fewer and one plain command the local network is sure of
(`Engine::direct`: its label is in `direct` in `core/actions/actions.toml`, e.g. `flashlight_on`, at most 7 words and the network at least 85 % sure: a long, vague sentence that the network happens to read as a command goes to the assistant) are `local` at every
level, and never cost a request. Inside a larger order ("Taschenlampe an und danach einen Timer") the same simple
words do not make it local: it is `plan`, and the assistant proposes `flashlight.set` as one step. The words are data
in `core/lang/plan/<code>.toml`.

**The tools of a request.** The registry (`core/actions/actions.toml`) holds every action the assistant may propose: id
(`timer.set`), executor, needed capabilities, typed arguments. `Engine::available_actions` intersects it with
`Host::has` (Android reports what the phone can really do: a clock app answers the alarm intent, the task app is set up,
the calendar permission is granted, the phone has a torch). `plan::select` then picks for **this request** the families
whose words (`[tools]` in the language file) appear anywhere in it, so a mixed order ("Termin in den Kalender und eine
Erinnerung") gets the tools of every part and nothing else; a request that names no family gets all available actions.
`plan::prompt` tells the assistant exactly these, with the turn's clock (weekday, date, time, zone).

**What is sent.** The current sentence, the prompt and nothing else: no earlier turns, entries, contacts, calendar or
memory. A follow-up ("nein, früher") is a question about earlier turns and goes as `talk`; the answer to a question the
planner asked is joined to the sentence that raised it. The research AI never enters the planner: it only delivers
content.

**Privacy: what the planner can be sent, and no setting changes it.** The planner gets the tools of the request, the turn's clock
and the current sentence, nothing else: no stored entries (reminders, lists, notes, timers), contacts, calendar, notifications,
memory, location or earlier turns, and never what a step returned (reading actions show their result on the phone only; the
assistant is asked once per order). There is no setting that adds to this: the AI-use slider only says how early the assistant is
asked. Inside the sentence the host replaces what the phone knows to be private by a neutral word before it leaves (Android:
names of contacts, saved places, the values in the memory, phone numbers, e-mail addresses, account numbers: `Kontakt1`,
`Ort1`, `Angabe1`, `Nummer1`, `Mail1`, `Konto1`) and puts the words back into the checked actions on the phone after the core
validated them, so the order still works and the AI never learns who "Kontakt1" is (`PlannerPrivacy`; tests
`theProviderGetsTheToolsTheClockAndTheMaskedSentenceOnly` and `whatAReadingStepReturnsNeverGoesBackToTheProvider`, which switch
every optional sharing setting on and check everything the provider received).

**One clock per turn.** The clock and the offered tools are fixed when the prompt is made (`plan_prompt` keeps them) and
the reply is checked against the same instant and the same offer (`plan_validate`; after ten minutes, or if the phone's
clock went backwards, the current time is used, the tools stay). A relative word turned into a fixed date is judged against
the clock the assistant was given.

**The reply.** One JSON object, in one of three forms:

```json
{"version":2,"actions":[ {"type":"reminder.create","title":"Schuhhaus Müller","date":"2026-11-19","time":"16:00"},
                         {"type":"alarm.set","time":"15:40","date":"2026-11-19"} ]}
{"version":2,"ask":{"slot":"time"}}
{"version":2,"not_action":true}
```

`date` is `YYYY-MM-DD`, `time` is `HH:MM`; a duration only ever appears as `seconds`. The arguments of each action are in
the table below.

**Validation (`plan::validate`, `Registry::check`).** Everything or nothing: one bad action rejects the whole proposal and
nothing is carried out before it is accepted. Rejected: another `version`, an unknown action or field, an action that was
not offered for this request or that the phone cannot carry out (`unsupported`), more than 8 actions, a missing or wrongly
typed argument, a number or enum value out of its range, a time or date that is not exactly `HH:MM` / `YYYY-MM-DD`, a date
in the past or more than a year ahead, a moment (date plus time) that is not more than a minute ahead of the turn's clock
in the host's zone, a duration outside its range, and text that is empty, too long, or holds control characters, sentence
punctuation or brackets. An `ask` takes one of the slots `time`, `date`, `duration`, `title`, `list`; its question is
the core's own sentence, not the assistant's text, and it never comes with actions. `not_action` goes back to the
assistant as a text question. Date and time are resolved once, in the core: the executor receives `at_ms`.

**Execution.** A checked action goes to its executor as an action id with its checked values, **never as a sentence to
read again**. `Engine::plan_run(actions)` runs the actions in order: a core action calls `Skill::act` of the skill the
registry names (same replies, undo, scheduling and confirmations as a spoken request); a step only the app can do comes back
in `host` (`{action, args}`), Android runs it with its own code (`hostAct`) and reports `plan_continue(ok)`. The run stops at
a question, at a step of the app and at the first action that fails, finds nothing, finds several entries
(`action_unclear`: nothing is changed) or has an unknown outcome. What was done is reported and the steps that did not run
are counted (`left`, "Noch offen: …"). Nothing is repeated.

**A plan that waits goes on exactly once.** The validated actions that have not run yet stay in the core while the plan waits.
(1) The core asked a confirmation ("Termin eintragen?"): the answer goes through `Engine::process`; only if the skill
completed the confirmed step (`Cx::proceed`) the rest runs, once. "Nein", "Lassen wir's", another request, a failure, an
expired question (`question_ttl`), a question dismissed with `cancel_question` and anything unclear drop the rest; a later
"ja" then finds nothing waiting. (2) The app asked its own question (which contact, which app): Android calls `plan_wait`,
handles the answer as always and reports a completed step with `plan_continue(ok)`; anything else, or a new request, calls
`plan_drop`. While a question of the core is open, no other plan action runs.

**Permissions and confirmations stay.** Android's runtime permissions are checked by the same device code as for a spoken
request (`Denied` opens the settings screen). A calendar entry is never written before the user said yes
(`calendar.event` asks, the reminder skill's `event.confirm` writes), an alarm is dismissed only after the alarm skill's
question, and the calendar is offered to the planner only while the permission is granted.

**A rejected plan or an unreachable provider changes nothing.** If the core rejects the proposal, or the provider times out or
refuses (a transport failure), nothing is carried out and the sentence is **not** handed to the local interpretation: it went
to the planner because the local reading is not trusted for it. Iris says so and asks to say it again. (A core that has no
prompt for the request, or the consent or the setting being off, leaves the sentence to the local path: nothing was
proposed and nothing was sent.)

**Dismissing a question.** `Engine::cancel_question` drops the open question and nothing else: running timers and
existing entries stay, and nothing is created from the half-finished request. Android ends an open planner question with
a dismissal of any length ("Es reicht, ich brauche keinen Timer mehr"): nothing goes to the assistant again.

#### The actions (50)

`exec` is the skill whose `act` carries it out; `host (Android)`: a step only the app can do, handed to Android with its checked
values. Needs: capabilities the host must report (`Phone.has_capability`). Every action has a test in
`crates/iris-cli/tests/plan.rs` (`every_action_of_the_registry_has_a_test_here` and `..._has_an_executor_that_knows_it` keep
the table and the tests honest) and a description and tool words in every language file (`plan::tests::the_language_data_is_valid_and_describes_every_action`).
Arguments marked `?` are optional.

| Action | Arguments | exec | Needs | Confirmation | Test |
|---|---|---|---|---|---|
| `timer.set` | seconds, label? | timer | schedule |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `timer.cancel` | label?, all? | timer | schedule |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `timer.pause` | label? | timer | schedule |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `timer.resume` | label? | timer | schedule |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `timer.extend` | seconds, label? | timer | schedule |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `timer.show` | label? | timer | – |  | `timers_are_set_shown_paused_resumed_extended_and_cancelled` |
| `alarm.set` | time, date?, repeat? | alarm | alarm |  | `an_alarm_is_set_with_a_date_or_for_the_next_time_the_clock_shows_it` |
| `alarm.next` | – | alarm | alarm |  | `the_next_alarm_is_asked_for_and_dismissing_asks_first` |
| `alarm.show` | – | alarm | alarm |  | `the_next_alarm_is_asked_for_and_dismissing_asks_first` |
| `alarm.dismiss` | time? | alarm | alarm | asks first | `the_next_alarm_is_asked_for_and_dismissing_asks_first` |
| `reminder.create` | title, date, time, repeat? | reminder | schedule |  | `reminders_are_created_shown_moved_and_deleted` |
| `reminder.show` | – | reminder | – |  | `reminders_are_created_shown_moved_and_deleted` |
| `reminder.delete` | title | reminder | schedule |  | `a_title_that_fits_two_reminders_changes_nothing` |
| `reminder.move` | title, date, time | reminder | schedule |  | `reminders_are_created_shown_moved_and_deleted` |
| `list.add` | list, items (up to 20) | lists | – |  | `list_entries_are_added_shown_ticked_off_and_removed` |
| `list.show` | list? | lists | – |  | `list_entries_are_added_shown_ticked_off_and_removed` |
| `list.check_all` | list | lists | – |  | `many_entries_for_two_lists_are_two_actions_and_a_cross_list_tick_is_decided_on_the_phone` |
| `list.check_common` | list, other | lists | – |  | same: ticks off on `list` what is also on `other`, decided on the phone, so the assistant never needs to see either list |
| `list.check` | item, list? | lists | – |  | `list_entries_are_added_shown_ticked_off_and_removed` |
| `list.remove` | item, list? | lists | – |  | `list_entries_are_added_shown_ticked_off_and_removed` |
| `note.add` | text | notes | – |  | `notes_are_added_shown_and_removed` |
| `note.show` | – | notes | – |  | `notes_are_added_shown_and_removed` |
| `note.remove` | text | notes | – |  | `notes_are_added_shown_and_removed` |
| `device.volume` | change, percent?, ring? | device | device |  | `the_phone_is_changed_with_the_checked_values` |
| `device.brightness` | change, percent? | device | device |  | `the_phone_is_changed_with_the_checked_values` |
| `device.sound_mode` | mode | device | device |  | `the_phone_is_changed_with_the_checked_values` |
| `device.dnd` | on | device | device |  | `the_phone_is_changed_with_the_checked_values` |
| `task.create` | title, date? | reminder | tasks |  | `a_task_goes_to_the_task_app` |
| `calendar.event` | title, date, time, minutes? | reminder | calendar | asks first | `after_a_yes_the_event_and_the_reminder_exist_exactly_once` |
| `flashlight.set` | on | host (Android) | flashlight |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `reminder.snooze` | seconds? | reminder | schedule |  | `a_reminder_that_just_rang_is_snoozed_by_the_given_time` |
| `clock.time` | – | clock | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `clock.date` | – | clock | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `stopwatch.start` | – | stopwatch | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `stopwatch.stop` | – | stopwatch | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `stopwatch.reset` | – | stopwatch | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `stopwatch.show` | – | stopwatch | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `convert.units` | value, from, to | convert | – |  | `the_clock_the_stopwatch_and_unit_conversion_answer` |
| `search.web` | query | device | device |  | `a_web_search_and_what_plays_and_notifications` |
| `music.playing` | – | device | media |  | `a_web_search_and_what_plays_and_notifications` |
| `notification.read` | from? | device | notifications |  | `a_web_search_and_what_plays_and_notifications` |
| `notification.reply` | message, from? | device | notifications | asks first | `a_reply_to_a_notification_always_asks_before_it_sends` |
| `music.control` | action, query? | host (Android) | media |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `app.open` | app | host (Android) | apps |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `navigation.start` | place | host (Android) | maps |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `call.start` | contact | host (Android) | contacts |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `weather.show` | date?, place? | host (Android) | weather |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `calendar.show` | date?, days? | host (Android) | calendar_read |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `day.overview` | tomorrow? | host (Android) | weather |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `battery.show` | – | host (Android) | – |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `calculator.compute` | expression | host (Android) | – |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |
| `joke.tell` | – | host (Android) | – |  | `a_step_only_the_app_can_do_is_handed_over_with_its_checked_values (core); AssistantPlanTest (app)` |

#### Inventory: everything Iris does today, and how the assistant reaches it

| Iris function (classifier label / skill) | Local | Planner action | Remark |
|---|---|---|---|
| timer set / query / cancel / pause / resume / extend | yes | `timer.*` | |
| alarm set / next / list / dismiss | yes | `alarm.*` | one-off and repeating (daily, weekdays, weekly) |
| reminder create / list / delete / move / snooze | yes | `reminder.*` | repeating via `repeat`; snooze acts on the reminder that rang last |
| lists: add / read / check / remove | yes | `list.*` | "empty a list" (asks first) stays spoken |
| notes: add / read / remove | yes | `note.*` | "delete all notes" (asks first) stays spoken |
| time, date | yes | `clock.time`, `clock.date` | |
| stopwatch | yes | `stopwatch.*` | |
| unit conversion | yes | `convert.units` | units as words of the user's language |
| volume, brightness, sound mode, do not disturb | yes | `device.*` | system permissions as for a spoken request |
| web search in the browser | yes | `search.web` | the online research AI is not an action: it only delivers content |
| what is playing | yes | `music.playing` | |
| notifications: read / reply | yes | `notification.read`, `notification.reply` | a reply always asks "send?" first |
| flashlight on / off | yes (every level, no request) | `flashlight.set` | |
| music: play / pause / next / previous | yes (every level) | `music.control` | |
| open an app | yes (every level) | `app.open` | several matches: the app asks which; the plan waits |
| navigation | yes (every level) | `navigation.start` | |
| call | yes (every level) | `call.start` | contacts permission; several numbers: the app asks which; the plan waits |
| weather (day, place) | yes (every level) | `weather.show` | |
| calendar: read out | yes (every level) | `calendar.show` | read permission |
| calendar: create | yes | `calendar.event` | asks "write it?" first |
| task app: create | yes | `task.create` | task app set up |
| day overview | yes (every level) | `day.overview` | |
| battery | yes (every level) | `battery.show` | |
| calculator | yes (every level) | `calculator.compute` | digits and operators only |
| joke | yes (every level) | `joke.tell` | |

**Gaps, stated on purpose.** These Iris functions are reached by the user's own words only; the assistant cannot start them as
a plan step yet:

- **Translation and lyrics**: they need the existing one-time "may I send this to a lookup site?" consent, which only the
  Kotlin assistant holds (`Pending.Online`); no typed contract yet.
- **School (Untis) queries and anime tracker queries and list changes**: many query shapes read from a sentence frame
  (`heads`, `slots`); the anime list changes ask for a yes in Kotlin. No typed contract yet.
- **Task app: read the tasks** (`tasks_list`) and **memory** (`memory_remember`, `memory_recall`): not in the registry yet
  (memory has its own review screen).
- **The phone's settings panels** (`system_settings`) and **song recognition** (`song_recognize`, opens another app).
- **Smalltalk** is answered locally or by the conversation AI, never as an action.
- **Alarm and reminder repeats on specific weekdays** ("montags und mittwochs"): only daily, weekdays and weekly.
- A calendar entry with a repeat, and an alarm with a date further than a year ahead.

Adding an action needs a registry entry, a description and tool words per language, the `act` of its executor (or a branch in
Android's `hostAct`), a test against the real engine and tests for rejected and unavailable operations.

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
