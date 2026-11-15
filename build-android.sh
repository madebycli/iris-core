#!/usr/bin/env bash
# Builds the core for an Android app.
#   build-android.sh <app-main-dir>      e.g. ../app/src/main
# Writes the native library to <dir>/jniLibs/arm64-v8a and the Kotlin bindings to <dir>/java.
# Needs: the rustup targets for the ABIs (aarch64-linux-android, armv7-linux-androideabi, i686-linux-android, x86_64-linux-android), cargo-ndk 4, ANDROID_NDK_HOME.
set -euo pipefail
cd "$(dirname "$0")"
app="${1:?usage: build-android.sh <app-main-dir>}"
mkdir -p "$app"; app="$(cd "$app" && pwd)"
: "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME to the NDK directory}"
# ABIs to build, space-separated; the default is every ABI the app ships.
abis="${IRIS_ABIS:-arm64-v8a armeabi-v7a x86 x86_64}"
targets=()
for abi in $abis; do targets+=(-t "$abi"); done
cargo ndk "${targets[@]}" -o "$app/jniLibs" build -p iris-ffi --release
cargo build -p iris-ffi
cargo run -q -p iris-ffi --features cli --bin uniffi-bindgen -- generate \
  --library target/debug/libiris_ffi.so --language kotlin --no-format --out-dir "$app/java"
# The trained network is part of this repository (models/brain); the app ships a copy in its assets.
mkdir -p "$app/assets/brain"
cp models/brain/brain.onnx models/brain/meta.txt models/brain/pieces.tsv "$app/assets/brain/"
echo "core built: $(wc -c < "$app/jniLibs/arm64-v8a/libiris_ffi.so") bytes"
