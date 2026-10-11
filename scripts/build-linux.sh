#!/usr/bin/env bash
# Builds menger-desktop's Linux bundles (AppImage, .deb, .rpm) in Docker
# (DIST-4): docker/linux.Dockerfile is Ubuntu 22.04 with Tauri 2's
# prerequisites, so the host needs none of WebKitGTK, GTK or libsoup.
#
#   scripts/build-linux.sh                 # cargo tauri build
#   scripts/build-linux.sh -- <command…>   # any command in the build container
#
# The repository is bind-mounted at /src and the build runs as your uid, so
# src-tauri/target and the bundles are yours. The cargo registry is cached in
# ~/.cache/menger-desktop/cargo between runs. Updater signing (DIST-5): set
# TAURI_SIGNING_PRIVATE_KEY (the key, or its path) and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD, or have ~/.tauri/menger-desktop.key and
# ~/.tauri/menger-desktop.key.password — they are passed through, never
# written into the repository.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="${MENGER_DESKTOP_IMAGE:-menger-desktop-linux}"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/menger-desktop"
mkdir -p "$cache/cargo" "$cache/home"

if [[ "${MENGER_DESKTOP_REBUILD_IMAGE:-0}" == 1 ]] || ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build -t "$image" -f "$repo/docker/linux.Dockerfile" "$repo/docker"
fi

cmd=(cargo tauri build)
if [[ "${1:-}" == "--" ]]; then
  shift
  cmd=("$@")
fi

key_file="$HOME/.tauri/menger-desktop.key"
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" && -r "$key_file" ]]; then
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_file")"
  if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" && -r "$key_file.password" ]]; then
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$key_file.password")"
  fi
fi
env_args=()
for v in TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD RUST_LOG; do
  if [[ -n "${!v:-}" ]]; then
    export "$v"
    env_args+=(-e "$v")
  fi
done

tty=()
[[ -t 0 && -t 1 ]] && tty=(-it)

# SELinux is enforcing on the house machine: a bind mount is unreadable in a
# container without label=disable.
exec docker run --rm "${tty[@]}" \
  --security-opt label=disable \
  --user "$(id -u):$(id -g)" \
  -e HOME=/home/builder \
  -e CARGO_HOME=/cache/cargo \
  "${env_args[@]}" \
  -v "$repo:/src" \
  -v "$cache/cargo:/cache/cargo" \
  -v "$cache/home:/home/builder" \
  -w /src/src-tauri \
  "$image" "${cmd[@]}"
