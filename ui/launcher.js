// menger-desktop's launcher (DIST-4): Local or Remote. The app remembers the
// choice (in Rust, settings.json in the app's config directory); on the first
// view after start a remembered choice opens straight away.
"use strict";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

function setBusy(busy, message) {
  for (const b of document.querySelectorAll("button")) b.disabled = busy;
  $("remote-url").disabled = busy;
  const status = $("status");
  status.hidden = !message;
  status.textContent = message || "";
}

function showError(message) {
  const el = $("error");
  el.hidden = !message;
  el.textContent = message || "";
}

async function openLocal() {
  showError("");
  setBusy(true, "Starting menger on this device…");
  try {
    await invoke("open_local");
    setBusy(true, "Opening…");
  } catch (e) {
    setBusy(false);
    showError(String(e));
  }
}

async function openRemote(url) {
  showError("");
  const value = (url ?? $("remote-url").value).trim();
  if (!value) {
    showError("Enter the address of a menger server.");
    $("remote-url").focus();
    return;
  }
  setBusy(true, `Reaching ${value}…`);
  try {
    await invoke("open_remote", { url: value });
    setBusy(true, "Opening…");
  } catch (e) {
    setBusy(false);
    showError(String(e));
    $("remote-url").focus();
  }
}

async function start() {
  $("local").addEventListener("click", openLocal);
  $("remote-form").addEventListener("submit", (e) => {
    e.preventDefault();
    openRemote();
  });

  let state;
  try {
    state = await invoke("launcher_state");
  } catch (e) {
    showError(`The app did not answer: ${e}`);
    return;
  }
  $("version").textContent = `menger-desktop ${state.version}`;
  $("local-where").textContent = `Stored in ${state.db_path}`;
  if (state.mobile) {
    $("device-word").textContent = "device";
    $("menu-hint").hidden = true;
    $("remote-hint").hidden = false;
  }
  if (state.local_url) $("local").textContent = "Return to local";
  if (state.remote_url) $("remote-url").value = state.remote_url;

  if (state.auto && state.choice === "local") return openLocal();
  if (state.auto && state.choice === "remote" && state.remote_url) return openRemote(state.remote_url);
  (state.choice === "remote" ? $("remote-url") : $("local")).focus();
}

start();
