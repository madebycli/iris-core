# The online AI in Iris: what it gets, what it may change

The local network decides what a sentence is. The online AI (Lumo, OpenRouter, a custom server or Venice) is
asked only where it adds something, and it never changes the phone or the user's data on its own: the core
checks every proposal.

## What is worth a paid request (`iris-nlu`, `fallback::worth_cloud`, `fallback::conversation`)

- Offline commands (timers, alarms, weather, devices, music, lists, reminders) never go to the AI.
- A knowledge question goes to the AI.
- A sentence the network could not place and that is no real question is mostly noise from the speech
  recogniser: Iris asks back locally, which costs nothing.
- Set phrases ("hi", "danke", "wie geht's") keep Iris's own answers; real conversation of four words or more
  ("ich fühle mich heute mies, weil ...") goes to the AI with the memory (setting "Conversations").

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
