# menger-desktop's Linux build environment (DIST-4).
#
# Ubuntu 22.04 — the base tauri-action's `ubuntu-22.04` runner uses — with
# Tauri 2's documented Linux prerequisites, rustup stable and tauri-cli 2.x.
# The repository is bind-mounted at /src and the build runs as the host's
# uid (scripts/build-linux.sh), so nothing it writes is root-owned. Xvfb is
# here only so the app can be smoke-run headless in the container.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl wget file git ca-certificates pkg-config \
        libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
        libayatana-appindicator3-dev librsvg2-dev \
        xvfb xauth \
    && rm -rf /var/lib/apt/lists/*

# Toolchain in /opt, readable (and its cargo bin runnable) by any uid; each
# build's registry cache goes to a CARGO_HOME the script mounts.
ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable --component clippy \
    && cargo install tauri-cli --version "^2" --locked \
    && rm -rf /opt/cargo/registry /opt/cargo/git \
    && chmod -R a+rX /opt/rustup /opt/cargo

# linuxdeploy (the AppImage bundler) is itself an AppImage; there is no FUSE
# in a container, so it extracts and runs.
ENV APPIMAGE_EXTRACT_AND_RUN=1
WORKDIR /src/src-tauri
