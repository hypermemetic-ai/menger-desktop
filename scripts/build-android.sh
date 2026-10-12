#!/usr/bin/env bash
# Builds menger-desktop for Android in Docker (DIST-6): docker/android.Dockerfile
# is Ubuntu 24.04 with JDK 17, the Android SDK and NDK, the Rust Android
# targets and tauri-cli, so the host needs none of them.
#
#   scripts/build-android.sh                  # signed release APK + AAB, every ABI
#   scripts/build-android.sh debug            # debug APK (x86_64 — the emulator's ABI)
#   scripts/build-android.sh -- <command…>    # any command in the build container
#
# Release signing reads a keystore.properties (storeFile, storePassword,
# keyAlias, keyPassword — see README "Android signing"):
# $MENGER_ANDROID_KEYSTORE_PROPERTIES if set, else
# ~/.android-keys/menger-desktop-release.properties when present. Its
# directory is mounted read-only into the container; nothing about the key is
# written into the repository. Without one the release build is unsigned.
#
# The repository is bind-mounted at /src and the build runs as your uid. The
# cargo registry is shared with scripts/build-linux.sh
# (~/.cache/menger-desktop/cargo); Gradle's caches live in
# ~/.cache/menger-desktop/android-home, and the Rust build in
# src-tauri/target/android (apart from the Linux build's, whose container has
# an older glibc).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="${MENGER_DESKTOP_ANDROID_IMAGE:-menger-desktop-android}"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/menger-desktop"
mkdir -p "$cache/cargo" "$cache/android-home"

if [[ "${MENGER_DESKTOP_REBUILD_IMAGE:-0}" == 1 ]] || ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build --target build -t "$image" -f "$repo/docker/android.Dockerfile" "$repo/docker"
fi

case "${1:-release}" in
  --)
    shift
    cmd=("$@")
    ;;
  release)
    cmd=(cargo tauri android build --apk --aab)
    ;;
  debug)
    cmd=(cargo tauri android build --debug --apk --target x86_64)
    ;;
  *)
    echo "usage: $0 [release|debug|-- <command…>]" >&2
    exit 2
    ;;
esac

mounts=()
env_args=(-e CARGO_TARGET_DIR=/src/src-tauri/target/android)
props="${MENGER_ANDROID_KEYSTORE_PROPERTIES:-$HOME/.android-keys/menger-desktop-release.properties}"
if [[ -r "$props" ]]; then
  keydir="$(cd "$(dirname "$props")" && pwd)"
  mounts+=(-v "$keydir:/keys:ro")
  env_args+=(-e "MENGER_ANDROID_KEYSTORE_PROPERTIES=/keys/$(basename "$props")")
elif [[ "${cmd[*]}" == "cargo tauri android build --apk --aab" ]]; then
  echo "note: no keystore.properties ($props) — the release build will be unsigned" >&2
fi
[[ -n "${RUST_LOG:-}" ]] && env_args+=(-e RUST_LOG)

tty=()
[[ -t 0 && -t 1 ]] && tty=(-it)

# SELinux is enforcing on the house machine: a bind mount is unreadable in a
# container without label=disable.
exec docker run --rm "${tty[@]}" \
  --security-opt label=disable \
  --user "$(id -u):$(id -g)" \
  -e HOME=/home/builder \
  -e CARGO_HOME=/cache/cargo \
  -e GRADLE_USER_HOME=/home/builder/.gradle \
  "${env_args[@]}" \
  -v "$repo:/src" \
  -v "$cache/cargo:/cache/cargo" \
  -v "$cache/android-home:/home/builder" \
  "${mounts[@]}" \
  -w /src/src-tauri \
  "$image" "${cmd[@]}"
