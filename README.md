# menger-desktop

[menger](https://github.com/hypermemetic-ai/menger) — *a plan is a program
whose leaves are prose* — as a desktop and mobile app (Linux, macOS,
Windows, Android, iOS). A [Tauri 2](https://tauri.app)
shell that takes menger as a library from crates.io
(`menger = { version = "0.5", features = ["embedded-app"] }`) and opens
menger's own web app in its window.

## Two modes

The app opens on a small launcher with two choices:

- **Local** — *tickets on this device.* menger runs inside the app
  (`menger::local::LocalServer`): its Plexus WebSocket and its web shell on
  free loopback ports (`127.0.0.1`, ports chosen by the OS), its database in
  the app's data directory. The window opens the shell's `/`. Nothing listens
  beyond loopback, and nothing leaves the machine. Quitting the app stops the
  server and releases its ports.
- **Remote** — a menger server at a URL you give (`https://menger.example`,
  `http://127.0.0.1:4451/`, …). The launcher checks the address is http(s)
  and that something answers there, and tells you plainly if not; then the
  window opens that server's own app. Choosing remote stops a local server.

The choice and the last remote URL are remembered (`settings.json` in the
app's config directory); on the next start the app goes straight there.

**Switch server** returns to the launcher at any time:

- *Server → Switch…* in the menu bar (desktop; phones have no menu bar);
- the small ⇄ button in the bottom-left corner of every page (every
  platform) — faint until hovered. It is injected into each page's main
  frame, in a closed shadow root outside `<body>`, takes no focus and handles
  no keys, so menger's layout and keyboard are untouched. It needs no IPC
  (remote pages get none): it navigates to a sentinel address,
  `https://switch.menger-desktop.invalid/`, which the window's navigation
  guard cancels and answers by opening the launcher.

A local server keeps running while you are on the launcher, so *Return to
local* is instant.

## Where data lives

| | Linux | macOS | Windows |
|---|---|---|---|
| local tickets | `~/.local/share/ai.hypermemetic.menger/tickets.db` | `~/Library/Application Support/ai.hypermemetic.menger/tickets.db` | `%APPDATA%\ai.hypermemetic.menger\tickets.db` |
| settings | `~/.config/ai.hypermemetic.menger/settings.json` | `~/Library/Application Support/ai.hypermemetic.menger/settings.json` | `%APPDATA%\ai.hypermemetic.menger\settings.json` |

On Android both are in the app's private storage
(`/data/user/0/ai.hypermemetic.menger/`); on iOS, in the app's container
(`Library/Application Support/`). Uninstalling the app deletes them.

The launcher shows the database's path. The app never uses menger's
production database (`~/.local/share/menger/tickets.db`) and its local server
never holds ports 4450–4458 (menger's prod and test environments on its home
machine); both are asserted in code.

Logs go to stderr (`RUST_LOG` filters; `info` by default) — the local
server's shell URL is logged at start.

## Layout

```
src-tauri/          the app (Rust crate `menger-desktop`)
  src/lib.rs        run(): window, launcher commands, local server, switching
  src/update.rs     the self-update check (desktop)
  src/main.rs       desktop entry point — calls run()
  src/settings.rs   the remembered choice
  src/switch.js     the injected Switch-server control
  tauri.conf.json   app config (version comes from Cargo.toml)
  Info.ios.plist    merged into the iOS Info.plist (loopback http for local mode)
  capabilities/     IPC permissions — the bundled launcher only
  icons/            generated from app-icon.svg (`cargo tauri icon app-icon.svg -o src-tauri/icons`)
  gen/android/      the Android project (`cargo tauri android init`), edited: signing, cleartext, insets
  gen/apple/        the Xcode project (`cargo tauri ios init`)
ui/                 the launcher — plain HTML/CSS/JS, no build step
docker/             build images: linux.Dockerfile, android.Dockerfile (+ an emulator stage)
scripts/            build-linux.sh, build-android.sh, android-emulator.sh
.github/workflows/  release.yml (a v* tag → signed bundles + latest.json + mobile),
                    mobile.yml (Android + iOS; by hand on any branch, or from release.yml), ci.yml
```

`run()` is `#[cfg_attr(mobile, tauri::mobile_entry_point)]`, so iOS and
Android build from the same library.

## Building

Install the Tauri CLI once: `cargo install tauri-cli --version "^2" --locked`.

**Linux** — in Docker, so the host needs none of WebKitGTK, GTK or libsoup:

```sh
scripts/build-linux.sh                    # cargo tauri build in the container
scripts/build-linux.sh -- cargo clippy    # any command in the container
```

`docker/linux.Dockerfile` is Ubuntu 22.04 (the base of the `ubuntu-22.04`
runner the release CI uses) with Tauri 2's Linux prerequisites, rustup
stable and tauri-cli. The script builds the image the first time, bind-mounts
the repository, runs as your uid, caches the cargo registry in
`~/.cache/menger-desktop`, and passes `--security-opt label=disable` (SELinux
hosts). Output, in `src-tauri/target/release/bundle/`:

- `appimage/menger_<version>_amd64.AppImage` — bundles WebKit; runs on most
  distributions as is (`chmod +x` and run it);
- `deb/menger_<version>_amd64.deb`, `rpm/menger-<version>-1.x86_64.rpm`.

On a Linux host that has the
[prerequisites](https://v2.tauri.app/start/prerequisites/#linux) installed,
plain `cargo tauri build` in `src-tauri/` works too.

**macOS / Windows** — natively, on the platform:

```sh
cd src-tauri && cargo tauri build
```

(macOS: Xcode command-line tools; Windows: the MSVC build tools and WebView2,
which Windows 10/11 ship.) The bundles are not OS-code-signed — macOS
Gatekeeper and Windows SmartScreen will ask before the first run.

**Android / iOS** — see [Mobile](#mobile).

**Running from source** — `cd src-tauri && cargo tauri dev`, on a host with
the prerequisites.

## Releases and updates

The desktop builds update themselves (`tauri-plugin-updater`). On start the
app checks
`https://github.com/hypermemetic-ai/menger-desktop/releases/latest/download/latest.json`
in the background; if a newer version is there it asks — version and release
notes, *Install and restart* or *Later* — then downloads it, checks its
signature, installs it and relaunches. *Help → Check for updates…* asks on
demand. A failed check (offline, no release yet, a bad signature) is logged
and never stops the app. iOS and Android update through their stores
(DIST-7), not the updater.

Every bundle is signed for the updater with a minisign key
(`cargo tauri signer generate`): the public key is in `tauri.conf.json`
(`plugins.updater.pubkey`); the private key and its password are the
repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, kept by the owner outside the
repository and never committed. Losing the private key means installed apps
can no longer be updated — keep a copy.

**Cutting a release:**

1. bump `version` in `src-tauri/Cargo.toml` (`tauri.conf.json` has no
   version — it follows Cargo.toml), and commit;
2. `git tag vX.Y.Z` (the tag must be `v` + that version) and
   `git push origin vX.Y.Z`.

`.github/workflows/release.yml` then creates the tag's release, builds with
`tauri-action` on `ubuntu-22.04`, `macos-latest` (`aarch64-apple-darwin` and
`x86_64-apple-darwin`) and `windows-latest`, uploads every bundle with its
`.sig` and `latest.json` (one entry per platform), and checks that
`latest.json` lists all four. The release is published, not a draft, so
installed apps see it as soon as it lands. `ci.yml` runs fmt, clippy and the
unit tests on every push.

A signed local build: `scripts/build-linux.sh` passes
`TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` through
when they are set, or reads `~/.tauri/menger-desktop.key` and
`~/.tauri/menger-desktop.key.password` when present; the bundles then get
`.sig` files beside them. Without a key, `cargo tauri build` fails at the
signing step — set `TAURI_SIGNING_PRIVATE_KEY` or build with
`--config '{"bundle":{"createUpdaterArtifacts":false}}'`.

## Mobile

Android and iOS run the same app: the same launcher, **local** and
**remote**, the same ⇄ control to switch (there is no menu bar). Local mode
is menger in-process on loopback, exactly as on the desktop; the database is
in the app's private storage.

- **Plain http only to this device.** Local mode is `http://127.0.0.1:<port>`
  and `ws://127.0.0.1:<port>`, which the platforms allow only by exception:
  Android's network security config
  (`gen/android/app/src/main/res/xml/network_security_config.xml`) permits
  cleartext to `127.0.0.1` and `localhost` and nothing else; iOS's
  `NSAppTransportSecurity` → `NSAllowsLocalNetworking` (`src-tauri/Info.ios.plist`)
  does the same. A **remote** server must therefore be `https://` — the
  launcher refuses `http://` to any other host and says why.
- **Backgrounding.** iOS suspends a backgrounded app and may close its
  sockets; Android may too. When the app returns to the foreground
  (Android `onResume`, iOS `applicationWillEnterForeground`) it checks that
  the local server still answers on both its ports; if not, it stops it and
  starts it again over the same database — on the same ports when they are
  free, else on new ones — and reloads the page at the same path. A server
  that still answers is left alone: menger's app reconnects its own
  WebSocket. (In a debug build, a file `debug-restart-on-resume` in the app's
  data directory forces the restart on the next resume, to try it.)
- **Updates** come through the stores (DIST-7), not the desktop updater;
  mobile builds are not in `latest.json`.

### Android

Built in Docker, like Linux — the host needs no JDK, SDK or NDK:

```sh
scripts/build-android.sh            # signed release APK + AAB, all four ABIs
scripts/build-android.sh debug      # debug APK, x86_64 (the emulator's ABI)
scripts/build-android.sh -- <cmd>   # any command in the container
```

`docker/android.Dockerfile` is Ubuntu 24.04 with JDK 17, the Android SDK
(platform 37, build-tools 36), NDK 29.0.13846066 (tauri-cli 2.12's own),
the four Rust Android targets and tauri-cli. Output, in
`src-tauri/gen/android/app/build/outputs/`:

- `apk/universal/release/app-universal-release.apk` — install it directly
  (arm64-v8a, armeabi-v7a, x86, x86_64);
- `bundle/universalRelease/app-universal-release.aab` — for the Play Store;
- `apk/universal/debug/app-universal-debug.apk` (the debug build).

**Trying it on an emulator** (needs `/dev/kvm`): `scripts/android-emulator.sh
start` boots a headless Android 15 emulator in Docker (the image's
`emulator` stage), then `scripts/android-emulator.sh adb install -r
/src/src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`,
`… adb shell am start -n ai.hypermemetic.menger/.MainActivity`,
`… adb exec-out screencap -p > shot.png`, `… adb logcat -s RustStdoutStderr`
(the app's log). The container shares the host's network, so `… adb reverse
tcp:4451 tcp:4451` makes `http://127.0.0.1:4451/` in the app the host's.

### Android signing

The release key is a PKCS12 keystore kept **outside the repository**, on the
owner's machine at `~/.android-keys/menger-desktop-release.jks` (directory
700, file 600), with `~/.android-keys/menger-desktop-release.properties`
(600) beside it:

```properties
storeFile=menger-desktop-release.jks   # relative to this file, or absolute
storePassword=…
keyAlias=menger-desktop
keyPassword=…                          # PKCS12: the same as storePassword
```

`gen/android/app/build.gradle.kts` reads that file from
`$MENGER_ANDROID_KEYSTORE_PROPERTIES`, else from
`gen/android/keystore.properties` (gitignored); with neither, the release
build is unsigned and says so. `scripts/build-android.sh` mounts
`~/.android-keys` read-only and sets the variable. In CI the same key is
the repository secrets `ANDROID_KEYSTORE_BASE64` (the .jks, base64),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`.

**Back the keystore and its properties up.** An app on Google Play can only
ever be updated with the key that signed it (unless Play App Signing holds
the app key): lose it and the listing can never be updated again.

### iOS

iOS builds need macOS and Xcode, so they run in CI (`mobile.yml`,
`macos-latest`); on a Mac: `brew install xcodegen`, then in `src-tauri/`,
`cargo tauri ios build --target aarch64-sim --no-sign` (simulator) or
`cargo tauri ios dev` (a simulator or a connected device).

There is no Apple Developer team yet, so CI builds for the **simulator
only** (`aarch64-apple-ios-sim`, unsigned) — a compile check — and also
checks the library for the device target (`aarch64-apple-ios`). The release
carries it as `menger_<version>_ios-simulator-arm64.app.zip`: unzip it and
`xcrun simctl install booted menger.app` on a Mac. It is not an app for a
phone. A signed device build (`.ipa`) is wired in `mobile.yml` behind the
`APPLE_DEVELOPMENT_TEAM` secret (with `IOS_CERTIFICATE`,
`IOS_CERTIFICATE_PASSWORD`, `IOS_MOBILE_PROVISION`) and is untested until the
team exists — DIST-7, with TestFlight and Play internal testing.

### Mobile CI

`mobile.yml` builds Android (signed APK + AAB) and iOS (simulator) and
uploads them as workflow artifacts. Run it by hand on any branch —
`gh workflow run mobile.yml --ref <branch>` — to test without a tag;
`release.yml` runs it on a `v*` tag and attaches the files to the release
(`menger_<version>_android-universal.apk`, `…android-universal.aab`,
`…ios-simulator-arm64.app.zip`). The version (Android `versionName`, and
`versionCode` = major·1000000 + minor·1000 + patch) follows
`src-tauri/Cargo.toml`.

## License

MIT — see [LICENSE](LICENSE).
