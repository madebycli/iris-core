# Notices

## Ari (design reference)

The skill contract of this engine (one trait per skill, a manifest with patterns and declared
capabilities, replies as data the frontend renders, one language file per skill) follows the
design of [Ari](https://github.com/ari-digital-assistant) by Keith Vassallo (`ari-engine` and
`ari-android`: GPL-3.0; `ari-skills`: Apache-2.0, skills MIT). No Ari source code is included.

## Dependencies

Rust crates under their own licenses (serde, regex, jiff, toml, uniffi and their dependencies;
all MIT, Apache-2.0 or MPL-2.0), listed in `Cargo.lock`.

`iris-cloud` uses RustCrypto (AES-GCM, HKDF, SHA-2/SHA-3 and k256) and
Phala Network's `dcap-qvl` 0.7.1 (MIT). The public Intel quote test fixtures
are distributed with their MIT license under `crates/iris-cloud/tests/fixtures`.
The Venice wire protocol is documented in `docs/VENICE_E2EE.md`; no Venice SDK
source is embedded in the application.
