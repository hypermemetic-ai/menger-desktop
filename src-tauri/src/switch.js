// menger-desktop's "Switch server" control (DIST-4), injected into the main
// frame of every page the window loads. It is a faint 22px button in the
// bottom-left corner, solid on hover or focus; inside a closed shadow root,
// so no page style reaches it and none of its styles leak; outside <body>,
// so the app re-rendering its body never removes it. It takes no focus, adds
// no key handlers and stops only its own clicks — the page's layout and
// keyboard are untouched.
//
// It needs no IPC (remote pages get none): it navigates to a sentinel
// address the app cancels in its navigation guard and answers by opening the
// launcher.
(function () {
  if (window.top !== window) return;
  var proto = location.protocol;
  if (proto !== "http:" && proto !== "https:") return; // the launcher (tauri://) and blank pages
  if (location.hostname === "tauri.localhost") return; // the launcher on Windows / Android
  var SWITCH = "https://switch.menger-desktop.invalid/";

  function mount() {
    if (document.querySelector("menger-desktop-switch")) return;
    var host = document.createElement("menger-desktop-switch");
    // CSSOM, not style attributes or <style>: a page's CSP cannot refuse it.
    var hs = host.style;
    hs.all = "initial";
    hs.position = "fixed";
    hs.left = "6px";
    hs.bottom = "6px";
    hs.zIndex = "2147483647";
    var root = host.attachShadow({ mode: "closed" });
    var css =
      "button{all:initial;box-sizing:border-box;width:22px;height:22px;border-radius:6px;display:grid;place-items:center;" +
      "cursor:pointer;background:rgba(127,127,127,.18);color:#1a5fb4;opacity:.35;transition:opacity .15s,background .15s;" +
      "-webkit-tap-highlight-color:transparent;}" +
      "button:hover,button:focus-visible{opacity:1;background:rgba(127,127,127,.3);}" +
      "@media (prefers-color-scheme:dark){button{color:#8ab4f8;}}" +
      "svg{width:14px;height:14px;fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;}";
    try {
      var sheet = new CSSStyleSheet();
      sheet.replaceSync(css);
      root.adoptedStyleSheets = [sheet];
    } catch (_) {
      var style = document.createElement("style");
      style.textContent = css;
      root.appendChild(style);
    }
    var button = document.createElement("button");
    button.type = "button";
    button.tabIndex = -1;
    button.title = "Switch server";
    button.setAttribute("aria-label", "Switch server");
    var NS = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    var path = document.createElementNS(NS, "path");
    path.setAttribute("d", "M7 4 3 8l4 4M3 8h14M17 20l4-4-4-4M21 16H7");
    svg.appendChild(path);
    button.appendChild(svg);
    root.appendChild(button);
    button.addEventListener("mousedown", function (e) {
      e.preventDefault(); // keep focus where the reader had it
    });
    button.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      location.href = SWITCH;
    });
    document.documentElement.appendChild(host);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", mount, { once: true });
  } else {
    mount();
  }
})();
