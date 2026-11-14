# 5. Dialogue state is data

**Decision.** A skill that asks a question stores what it knows as JSON (`cx.ask(intent,
state)`); the engine routes the next utterance back with that state. Skills hold no memory.

**Why.** The Kotlin flows kept closures in memory; they could not be tested in isolation and
were lost when Android killed the process. Data can be persisted, inspected in a test and
moved across the FFI boundary.
