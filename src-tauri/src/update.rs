//! DIST-5: the desktop app updates itself from GitHub Releases.
//!
//! The updater plugin reads `latest.json` from the latest release
//! (`plugins.updater.endpoints` in tauri.conf.json), checks the bundle's
//! minisign signature against the public key there, and installs it. On
//! start the check runs in the background and stays silent unless there is
//! an update; *Check for updates…* in the menu reports every outcome. Every
//! failure (offline, no release yet, a bad signature) is logged and, when
//! the reader asked, shown — never fatal.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::oneshot;

/// One check at a time (start-up and the menu item can overlap).
static CHECKING: AtomicBool = AtomicBool::new(false);

/// Checks for an update in the background. `interactive`: the reader asked
/// (the menu), so "up to date" and failures are shown too.
pub fn spawn_check(app: &AppHandle, interactive: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if CHECKING.swap(true, Ordering::SeqCst) {
            tracing::info!("update check already running");
            return;
        }
        if let Err(e) = check(&app, interactive).await {
            tracing::warn!("update check failed: {e:#}");
            if interactive {
                let _ = ask(
                    &app,
                    "Update check failed",
                    &format!("{e:#}"),
                    MessageDialogKind::Warning,
                    None,
                )
                .await;
            }
        }
        CHECKING.store(false, Ordering::SeqCst);
    });
}

async fn check(app: &AppHandle, interactive: bool) -> anyhow::Result<()> {
    let current = app.package_info().version.to_string();
    tracing::info!("checking for updates (this is {current})");
    let Some(update) = app.updater()?.check().await? else {
        tracing::info!("menger-desktop {current} is up to date");
        if interactive {
            ask(
                app,
                "No update",
                &format!("menger-desktop {current} is the latest version."),
                MessageDialogKind::Info,
                None,
            )
            .await;
        }
        return Ok(());
    };
    tracing::info!(
        "update available: {} → {}",
        update.current_version,
        update.version
    );

    let mut text = format!(
        "menger-desktop {} is available — you have {}.\n\nInstall it now? The app restarts when it is done.",
        update.version, update.current_version
    );
    if let Some(notes) = update
        .body
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        let notes: String = notes.chars().take(1200).collect();
        text.push_str("\n\n");
        text.push_str(&notes);
    }
    let yes = ask(
        app,
        "Update available",
        &text,
        MessageDialogKind::Info,
        Some(("Install and restart", "Later")),
    )
    .await;
    if !yes {
        tracing::info!("update {} declined", update.version);
        return Ok(());
    }

    tracing::info!("downloading menger-desktop {}", update.version);
    update
        .download_and_install(
            |_, _| {},
            || tracing::info!("download finished; installing"),
        )
        .await?;
    tracing::info!("installed menger-desktop {}; restarting", update.version);
    // Exit (RunEvent::Exit) stops the local server before the new version starts.
    app.restart();
}

/// A message dialog, awaited without blocking the runtime. With `buttons`
/// it is a question: true when the first button is chosen.
async fn ask(
    app: &AppHandle,
    title: &str,
    text: &str,
    kind: MessageDialogKind,
    buttons: Option<(&str, &str)>,
) -> bool {
    let (tx, rx) = oneshot::channel();
    let mut dialog = app.dialog().message(text).title(title).kind(kind);
    if let Some((ok, cancel)) = buttons {
        dialog = dialog.buttons(MessageDialogButtons::OkCancelCustom(
            ok.into(),
            cancel.into(),
        ));
    }
    dialog.show(move |answer| {
        let _ = tx.send(answer);
    });
    rx.await.unwrap_or(false)
}
