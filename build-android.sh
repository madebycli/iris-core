#!/usr/bin/env bash
# Builds the core for the Android app: the native library into app/src/main/jniLibs and the
# Kotlin bindings into app/src/main/java/uniffi. Both are generated and not committed.
# Needs: rustup target aarch64-linux-android, cargo-ndk 4, ANDROID_NDK_HOME.
set -euo pipefail
cd "$(dirname "$0")"
app=../app/src/main
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK directory}"
cargo ndk -t arm64-v8a -o "$app/jniLibs" build -p iris-ffi --release
cargo build -p iris-ffi
cargo run -q -p iris-ffi --features cli --bin uniffi-bindgen -- generate \
  --library target/debug/libiris_ffi.so --language kotlin --no-format --out-dir "$app/java"
echo "core built: $(ls -la "$app/jniLibs/arm64-v8a/libiris_ffi.so" | awk '{print $5}') bytes"
