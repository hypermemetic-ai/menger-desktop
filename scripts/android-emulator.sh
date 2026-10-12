#!/usr/bin/env bash
# An Android emulator in Docker, for trying an APK on this machine (DIST-6).
# Needs /dev/kvm. Headless (-no-window); look at it with `adb exec-out
# screencap -p`.
#
#   scripts/android-emulator.sh start          # boot it (waits for the boot to finish)
#   scripts/android-emulator.sh adb <args…>    # adb against it, e.g. install, logcat, shell
#   scripts/android-emulator.sh stop
#
# The container uses the host's network, so `adb reverse tcp:P tcp:P` makes
# the emulator's 127.0.0.1:P the host's — a server on this machine is
# reachable from the app as http://127.0.0.1:P/. The repository is mounted
# read-only at /src (APKs under src-tauri/gen/android/app/build/outputs).
# The AVD lives in ~/.cache/menger-desktop/emulator-home.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image="${MENGER_DESKTOP_EMULATOR_IMAGE:-menger-desktop-android-emulator}"
name="menger-desktop-emulator"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/menger-desktop"
avd="menger-api35"

if ! docker image inspect "$image" >/dev/null 2>&1; then
  docker build --target emulator -t "$image" -f "$repo/docker/android.Dockerfile" "$repo/docker"
fi

case "${1:-}" in
  start)
    mkdir -p "$cache/emulator-home"
    docker rm -f "$name" >/dev/null 2>&1 || true
    docker run -d --name "$name" \
      --security-opt label=disable \
      --device /dev/kvm \
      --network host \
      --user "$(id -u):$(id -g)" \
      -e HOME=/home/builder \
      -e ANDROID_USER_HOME=/home/builder/.android \
      -e ANDROID_AVD_HOME=/home/builder/.android/avd \
      -v "$cache/emulator-home:/home/builder" \
      -v "$repo:/src:ro" \
      "$image" bash -c "
        set -e
        if [[ ! -d /home/builder/.android/avd/$avd.avd ]]; then
          echo no | avdmanager create avd -n $avd -k 'system-images;android-35;google_apis;x86_64' -d pixel_6
        fi
        find /home/builder/.android/avd -name '*.lock' -exec rm -rf {} + 2>/dev/null || true
        exec emulator -avd $avd -no-window -no-audio -no-boot-anim -no-snapshot \
          -gpu swangle_indirect -memory 3072 -accel on
      " >/dev/null
    echo "waiting for the emulator to boot…"
    sleep 5
    until [[ "$(docker exec "$name" adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; do
      if [[ "$(docker inspect -f '{{.State.Running}}' "$name")" != true ]]; then
        docker logs "$name" | tail -20 >&2
        echo "the emulator exited" >&2
        exit 1
      fi
      sleep 3
    done
    echo "booted: $(docker exec "$name" adb shell getprop ro.build.version.release | tr -d '\r') (API $(docker exec "$name" adb shell getprop ro.build.version.sdk | tr -d '\r'))"
    ;;
  adb)
    shift
    exec docker exec -i "$name" adb "$@"
    ;;
  stop)
    docker rm -f "$name" >/dev/null
    ;;
  *)
    echo "usage: $0 start | adb <args…> | stop" >&2
    exit 2
    ;;
esac
