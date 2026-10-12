//! menger-desktop (DIST-4): menger in a Tauri window, **local** or **remote**.
//!
//! The window opens on the launcher (`ui/`, bundled). **Local** starts
//! [`menger::local::LocalServer`] in-process — free loopback ports, its
//! database under the app's data directory — and points the window at its
//! shell; **remote** points the window at a menger the reader names. Either
//! way the page is menger's own Leptos app, served by the server it talks to.
//!
//! The choice and the last remote URL are remembered (`settings.json` in the
//! app's config directory); a remembered choice opens straight away.
//! "Switch server" returns to the launcher: the desktop menu's
//! *Server → Switch…*, and on every platform a small control injected into
//! every page (`switch.js`). The control needs no IPC — remote pages get
//! none — it navigates to a sentinel address that [`on_navigation`] cancels
//! and answers by opening the launcher.

mod settings;
#[cfg(desktop)]
mod update;

use menger::local::{LocalConfig, LocalServer, TAURI_ORIGINS};
use serde::Serialize;
use settings::{parse_remote_url, Choice, Settings};
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};
use url::Url;

/// The main window's label.
const MAIN: &str = "main";

/// The address the injected control navigates to; never loaded — the
/// navigation is cancelled and the launcher opened instead. `.invalid`
/// never resolves (RFC 2606), so a page that somehow reached it loads nothing.
const SWITCH_HOST: &str = "switch.menger-desktop.invalid";

/// The "Switch server" control, injected into every page's main frame.
const SWITCH_SCRIPT: &str = include_str!("switch.js");

/// menger's prod (4455–4458) and test (4450–4453) ports on its home machine:
/// the local server never holds one. Ports 0 make that so already; this is
/// the cheap assertion that it stays so.
const RESERVED_PORTS: RangeInclusive<u16> = 4450..=4458;

/// The prod database's path under the home directory — never the local one.
const PROD_DB: &str = ".local/share/menger/tickets.db";

struct AppState {
    settings_path: PathBuf,
    db_path: PathBuf,
    settings: Mutex<Settings>,
    /// The in-process menger, while local mode is (or was last) open.
    server: tokio::sync::Mutex<Option<LocalServer>>,
    /// The launcher's own address, as its page reported it.
    launcher_url: Mutex<Option<Url>>,
    /// True until the launcher has asked once: the first ask may open the
    /// remembered choice; a return through "Switch server" does not.
    auto: AtomicBool,
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn remember(&self, choice: Choice, remote_url: Option<String>) {
        let mut s = self.settings.lock().unwrap();
        s.choice = Some(choice);
        if remote_url.is_some() {
            s.remote_url = remote_url;
        }
        if let Err(e) = s.save(&self.settings_path) {
            tracing::warn!(
                "cannot save settings to {}: {e:#}",
                self.settings_path.display()
            );
        }
    }

    /// The local server's shell, started if it is not running.
    async fn ensure_local(&self) -> anyhow::Result<Url> {
        let mut server = self.server.lock().await;
        if let Some(s) = server.as_ref() {
            return Ok(s.shell_url().parse()?);
        }
        let s = self.start_server(0, 0).await?;
        let url = s.shell_url().parse()?;
        *server = Some(s);
        Ok(url)
    }

    /// Starts menger on the given loopback ports (0: free ones) over the
    /// local database.
    async fn start_server(&self, shell_port: u16, ws_port: u16) -> anyhow::Result<LocalServer> {
        if self.db_path.ends_with(PROD_DB) {
            anyhow::bail!(
                "refusing menger's production database {}",
                self.db_path.display()
            );
        }
        let mut cfg = LocalConfig::new(&self.db_path);
        cfg.shell_port = shell_port;
        cfg.ws_port = ws_port;
        cfg.extra_origins = TAURI_ORIGINS.iter().map(|o| o.to_string()).collect();
        let s = LocalServer::start(cfg).await?;
        if let Some(p) = [s.ws_port(), s.shell_port()]
            .into_iter()
            .find(|p| RESERVED_PORTS.contains(p))
        {
            s.shutdown().await;
            anyhow::bail!("the local server was given port {p}, which belongs to menger's prod/test environments");
        }
        tracing::info!(
            "local menger: shell {} · WebSocket {} · database {}",
            s.shell_url(),
            s.ws_url(),
            self.db_path.display()
        );
        Ok(s)
    }

    /// Debug builds only: a file `debug-restart-on-resume` in the app's data
    /// directory makes the next resume treat the local server as gone (and
    /// is removed) — the restart path, which Android rarely needs, tried on
    /// an emulator.
    #[cfg(mobile)]
    fn debug_pretend_dead(&self) -> bool {
        if !cfg!(debug_assertions) {
            return false;
        }
        let Some(flag) = self
            .db_path
            .parent()
            .map(|d| d.join("debug-restart-on-resume"))
        else {
            return false;
        };
        let hit = std::fs::remove_file(&flag).is_ok();
        if hit {
            tracing::warn!(
                "debug: {} — treating the local server as gone",
                flag.display()
            );
        }
        hit
    }

    /// DIST-6: back in the foreground (mobile). iOS suspends a backgrounded
    /// app and may take its sockets with it; Android may too. When the local
    /// server no longer answers on both ports it is stopped and started again
    /// over the same database — on the same ports when they are free, else
    /// on new ones — and this returns the old shell port and the new shell
    /// URL, so the window can be re-pointed. `None`: no local server, or it
    /// still answers (menger's app reconnects its own WebSocket).
    #[cfg(mobile)]
    async fn revive_local(&self) -> anyhow::Result<Option<(u16, Url)>> {
        let mut server = self.server.lock().await;
        let Some(s) = server.as_ref() else {
            return Ok(None);
        };
        let (shell, ws) = (s.shell_port(), s.ws_port());
        if !self.debug_pretend_dead() && answers_http(&s.shell_url()).await && answers_tcp(ws).await
        {
            tracing::info!(
                "resumed: the local server still answers (shell {shell}, WebSocket {ws})"
            );
            return Ok(None);
        }
        tracing::warn!("resumed: the local server no longer answers (shell {shell}, WebSocket {ws}); restarting it");
        let old = server.take().expect("checked above");
        if tokio::time::timeout(Duration::from_secs(3), old.shutdown())
            .await
            .is_err()
        {
            tracing::warn!("the old local server did not stop within 3 s; abandoning it");
        }
        let s = match self.start_server(shell, ws).await {
            Ok(s) => s,
            Err(e) => {
                tracing::info!("ports {shell}/{ws} not free again ({e:#}); taking new ones");
                self.start_server(0, 0).await?
            }
        };
        let url = s.shell_url().parse()?;
        *server = Some(s);
        Ok(Some((shell, url)))
    }

    /// Stops the local server, if one runs, and waits for its ports.
    async fn stop_local(&self) {
        if let Some(s) = self.server.lock().await.take() {
            let (shell, ws) = (s.shell_port(), s.ws_port());
            s.shutdown().await;
            tracing::info!("local menger stopped (ports {shell}, {ws} released)");
        }
    }
}

/// What the launcher shows.
#[derive(Serialize)]
struct LauncherState {
    choice: Option<Choice>,
    remote_url: Option<String>,
    /// Open the remembered choice now (the app's first launcher view only).
    auto: bool,
    /// The local server's shell, when one is running.
    local_url: Option<String>,
    /// Where local mode keeps its tickets.
    db_path: String,
    version: String,
    /// iOS / Android: no menu bar, and remote http:// only to this device.
    mobile: bool,
}

#[tauri::command]
async fn launcher_state(
    app: AppHandle,
    webview: tauri::Webview,
    state: State<'_, AppState>,
) -> Result<LauncherState, String> {
    if let Ok(mut url) = webview.url() {
        url.set_query(None);
        url.set_fragment(None);
        *state.launcher_url.lock().unwrap() = Some(url);
    }
    let s = state.settings();
    let local_url = state.server.lock().await.as_ref().map(|s| s.shell_url());
    Ok(LauncherState {
        choice: s.choice,
        remote_url: s.remote_url,
        auto: state.auto.swap(false, Ordering::SeqCst),
        local_url,
        db_path: state.db_path.display().to_string(),
        version: app.package_info().version.to_string(),
        mobile: cfg!(mobile),
    })
}

/// Local mode: the in-process server (started if need be), then its shell.
#[tauri::command]
async fn open_local(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let url = state.ensure_local().await.map_err(|e| {
        tracing::error!("local menger failed to start: {e:#}");
        format!("The local server did not start: {e:#}")
    })?;
    state.remember(Choice::Local, None);
    navigate(&app, url.clone())?;
    Ok(url.to_string())
}

/// Remote mode: the URL checked and reached, the local server stopped, then
/// the remote shell.
#[tauri::command]
async fn open_remote(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<String, String> {
    let url = parse_remote_url(&url)?;
    // DIST-6: the mobile web views load plain http only from this device
    // (Android's network security config, iOS's App Transport Security).
    #[cfg(mobile)]
    if url.scheme() == "http" && !settings::is_loopback(&url) {
        return Err(format!(
            "{url} is plain http:// to another host, which this device's web view refuses — only 127.0.0.1 and localhost may be http:// here. Use the server's https:// address."
        ));
    }
    probe(&url).await?;
    state.stop_local().await;
    state.remember(Choice::Remote, Some(url.to_string()));
    tracing::info!("remote menger: {url}");
    navigate(&app, url.clone())?;
    Ok(url.to_string())
}

/// Reaches `url` once, so a typo or a server that is down is a clear message
/// in the launcher rather than a blank window.
async fn probe(url: &Url) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| format!("cannot make an HTTP client: {e}"))?;
    match client.get(url.clone()).send().await {
        Ok(r) if r.status().is_success() => Ok(()),
        Ok(r) => Err(format!(
            "{url} answered {} — is that a menger server?",
            r.status()
        )),
        Err(e) => {
            let why = if e.is_timeout() {
                "it did not answer within 8 seconds".to_string()
            } else if e.is_connect() {
                "nothing is listening there, or the host is unreachable".to_string()
            } else {
                e.to_string()
            };
            Err(format!("Could not reach {url}: {why}."))
        }
    }
}

/// Whether an HTTP server answers at `url` within two seconds (any status).
#[cfg(mobile)]
async fn answers_http(url: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
    else {
        return false;
    };
    client.get(url).send().await.is_ok()
}

/// Whether something accepts a connection on 127.0.0.1:`port` within two
/// seconds.
#[cfg(mobile)]
async fn answers_tcp(port: u16) -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect(("127.0.0.1", port))
        )
        .await,
        Ok(Ok(_))
    )
}

/// DIST-6: the app is in the foreground again (mobile) — make local mode
/// usable: restart a local server that stopped answering, and point a
/// window that showed it at the new one (the same path, so the reader is
/// where they were; the page reloads either way).
#[cfg(mobile)]
fn on_resume(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        match state.revive_local().await {
            Ok(None) => {}
            Ok(Some((old_port, mut url))) => {
                let Some(win) = app.get_webview_window(MAIN) else {
                    return;
                };
                let Ok(current) = win.url() else { return };
                let on_local = current.scheme() == "http"
                    && current.host_str() == Some("127.0.0.1")
                    && current.port() == Some(old_port);
                if !on_local {
                    return; // the launcher: "Return to local" opens the new one
                }
                url.set_path(current.path());
                url.set_query(current.query());
                url.set_fragment(current.fragment());
                tracing::info!("re-pointing the window at the restarted local server: {url}");
                if let Err(e) = win.navigate(url) {
                    tracing::error!("cannot re-point the window: {e}");
                }
            }
            Err(e) => {
                tracing::error!("cannot restart the local server: {e:#}");
                switch_server(&app);
            }
        }
    });
}

fn navigate(app: &AppHandle, url: Url) -> Result<(), String> {
    let win = app
        .get_webview_window(MAIN)
        .ok_or("the main window is gone")?;
    win.navigate(url).map_err(|e| e.to_string())
}

/// The launcher's address: as its page reported it, else the bundled page's
/// address on this platform.
fn launcher_url(app: &AppHandle) -> Url {
    if let Some(url) = app.state::<AppState>().launcher_url.lock().unwrap().clone() {
        return url;
    }
    let base = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost/"
    } else {
        "tauri://localhost/"
    };
    Url::parse(base).expect("a constant URL")
}

/// Back to the launcher. A local server keeps running (choosing local again
/// is instant); choosing remote stops it.
fn switch_server(app: &AppHandle) {
    tracing::info!("switch server: back to the launcher");
    if let Err(e) = navigate(app, launcher_url(app)) {
        tracing::error!("cannot open the launcher: {e}");
    }
}

/// The main window's navigation guard: the switch control's sentinel opens
/// the launcher instead; everything else proceeds.
fn on_navigation(app: AppHandle) -> impl Fn(&Url) -> bool + Send + 'static {
    move |url| {
        tracing::debug!("navigation: {url}");
        if url.host_str() == Some(SWITCH_HOST) {
            let app = app.clone();
            // Not from inside the webview's own navigation callback.
            tauri::async_runtime::spawn(async move { switch_server(&app) });
            return false;
        }
        true
    }
}

/// The platform's default menu, plus *Server → Switch…* and *Check for
/// updates…* (in Help where the default menu has one).
#[cfg(desktop)]
fn menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, MenuItemKind, Submenu};
    let menu = Menu::default(app)?;
    let switch = MenuItem::with_id(app, "switch-server", "Switch…", true, None::<&str>)?;
    menu.append(&Submenu::with_items(app, "Server", true, &[&switch])?)?;
    let updates = MenuItem::with_id(
        app,
        "check-updates",
        "Check for updates…",
        true,
        None::<&str>,
    )?;
    let help = menu.items()?.into_iter().find_map(|item| match item {
        MenuItemKind::Submenu(s) if s.text().is_ok_and(|t| t == "Help") => Some(s),
        _ => None,
    });
    match help {
        Some(help) => help.append(&updates)?,
        None => menu.append(&Submenu::with_items(app, "Help", true, &[&updates])?)?,
    }
    Ok(menu)
}

#[cfg(desktop)]
fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "switch-server" => switch_server(app),
        "check-updates" => update::spawn_check(app, true),
        _ => {}
    }
}

fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        // Mobile: stderr goes to logcat / the device console, not a terminal.
        .with_ansi(cfg!(desktop))
        .try_init();
}

fn data_paths(app: &AppHandle) -> anyhow::Result<(PathBuf, PathBuf)> {
    let config = app.path().app_config_dir()?;
    let data = app.path().app_data_dir()?;
    Ok((Settings::file(&config), data.join("tickets.db")))
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let (settings_path, db_path) = data_paths(&handle)?;
    let settings = Settings::load(&settings_path);
    tracing::info!(
        "menger-desktop {} · settings {} · local database {}",
        handle.package_info().version,
        settings_path.display(),
        db_path.display()
    );
    app.manage(AppState {
        settings_path,
        db_path,
        settings: Mutex::new(settings),
        server: tokio::sync::Mutex::new(None),
        launcher_url: Mutex::new(None),
        auto: AtomicBool::new(true),
    });

    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .initialization_script(SWITCH_SCRIPT)
        .on_navigation(on_navigation(handle.clone()));
    #[cfg(desktop)]
    let builder = builder
        .title("menger")
        .inner_size(1280.0, 860.0)
        .min_inner_size(520.0, 400.0);
    builder.build()?;

    // DIST-5: a background check on start; silent unless there is an update.
    #[cfg(desktop)]
    update::spawn_check(&handle, false);
    Ok(())
}

/// The app: desktop's `main` and mobile's entry point (DIST-6).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window(MAIN) {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .menu(menu)
        .on_menu_event(on_menu_event);
    let app = builder
        .invoke_handler(tauri::generate_handler![
            launcher_state,
            open_local,
            open_remote
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while building menger-desktop");
    app.run(|app, event| match event {
        RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { .. },
            ..
        } => {
            tracing::info!("window {label}: close requested");
        }
        // DIST-6: Android's onResume, iOS's applicationWillEnterForeground.
        // (`RunEvent::Resumed` is the event loop's, not the app's.)
        #[cfg(mobile)]
        RunEvent::WindowEvent {
            event: tauri::WindowEvent::Resumed,
            ..
        } => {
            tracing::info!("app resumed");
            on_resume(app);
        }
        RunEvent::Exit => {
            // Quitting stops the local server and releases its ports.
            tracing::info!("quitting");
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(state.stop_local());
        }
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_prod_database_is_recognised() {
        assert!(Path::new("/home/x/.local/share/menger/tickets.db").ends_with(PROD_DB));
        assert!(
            !Path::new("/home/x/.local/share/ai.hypermemetic.menger/tickets.db").ends_with(PROD_DB)
        );
    }
}
