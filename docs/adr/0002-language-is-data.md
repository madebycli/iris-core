# 2. Language is data

**Decision.** No word of a spoken language appears in Rust code. The engine's knowledge of a
language is `lang/<code>.toml`; a skill's is `data/<skill>/<code>.toml` with patterns,
word lists, expressions, rewrites, messages and training templates. Files are compiled in
with `include_str!` and validated when a skill is registered; a broken file fails the tests,
not a user.

**Why.** A new language must not mean touching logic, and a wording fix must not need a
programmer.

**Consequences.** Language-specific behaviour must be expressible as data. Where it is not
yet (German "zu" inside separable verbs), the answer is a new data mechanism (`[[rewrite]]`),
never an `if lang == "de"`.
