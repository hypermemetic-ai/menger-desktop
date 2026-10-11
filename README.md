# menger-desktop

[menger](https://github.com/hypermemetic-ai/menger) — *a plan is a program
whose leaves are prose* — as a desktop app. A [Tauri 2](https://tauri.app)
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

- *Server → Switch…* in the menu bar (desktop);
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
  src/main.rs       desktop entry point — calls run()
  src/settings.rs   the remembered choice
  src/switch.js     the injected Switch-server control
  tauri.conf.json   app config (version comes from Cargo.toml)
  capabilities/     IPC permissions — the bundled launcher only
  icons/            generated from app-icon.svg (`cargo tauri icon app-icon.svg -o src-tauri/icons`)
ui/                 the launcher — plain HTML/CSS/JS, no build step
docker/             Linux build image
scripts/            build-linux.sh
```

`run()` is `#[cfg_attr(mobile, tauri::mobile_entry_point)]`, so iOS and
Android (DIST-6) build from the same library.

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

**iOS / Android** — DIST-6.

**Running from source** — `cd src-tauri && cargo tauri dev`, on a host with
the prerequisites.

## License

MIT — see [LICENSE](LICENSE).
