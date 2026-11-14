# iris-core

The engine of [Iris](https://github.com/madebycli/iris), an offline-first voice assistant. One
utterance goes in, one reply comes out. No UI and no platform code: the device is reached
through the `Host` and `Store` traits, so the whole assistant runs and is tested on a desktop.

- **Skills today:** timers, reminders, lists, notes, alarms, time and date, the phone (volume, brightness, sound mode, Do Not Disturb, what is playing, web search, notifications), stopwatch, unit conversion, help. German and English.
- **The network:** `crates/iris-brain` runs the trained MiniLM in pure Rust (tract), no native library. Model files are not in this repository.
- **Language is data:** every word the engine knows lives in `lang/<code>.toml` and
  `crates/iris-skills/data/<skill>/<code>.toml`, never in Rust code.
- **Status:** the Android app uses it behind an experimental switch. Nothing else is moved yet;
  see [docs/CORE_ROADMAP.md](docs/CORE_ROADMAP.md).

```
crates/iris-core     pipeline, language packs, time expressions, Skill / Host / Store contracts
crates/iris-skills   the built-in skills (Rust for logic, TOML for words)
crates/iris-brain    the trained network in pure Rust (tract)
crates/iris-store    SQLite store (desktop, station, watch)
crates/iris-ffi      UniFFI boundary for frontends (Kotlin bindings)
crates/iris-cli      `iris` in a terminal, a fake phone, the spec runner
lang/                language packs
specs/               conversations that must hold (`cargo test`)
docs/                architecture, roadmap, decisions (adr/)
```

## Try it

```bash
cargo test                                   # every spec and unit test
cargo run -p iris-cli -- "Erinnere mich um fünf an den Müll"
cargo run -p iris-cli -- --lang en "set a pasta timer for 8 minutes"
```

A sentence that goes wrong becomes a line in `specs/` first, then a fix. The spec format is
described at the top of `crates/iris-cli/src/lib.rs`.

## Use it from an app

An app brings the device: it implements `Phone` (clock, settings, scheduling, storage, task app,
calendar) and links `iris-ffi`. `build-android.sh <app-main-dir>` builds the native library and
the Kotlin bindings (needs `cargo-ndk` and an NDK). Read
[docs/ARCHITECTURE_CORE.md](docs/ARCHITECTURE_CORE.md) for what is inside and what is not.

## License

GPL-3.0-or-later, see [LICENSE](LICENSE). The skill contract follows the design of
[Ari](https://github.com/ari-digital-assistant) by Keith Vassallo; no Ari source code is
included. See [NOTICE.md](NOTICE.md).
