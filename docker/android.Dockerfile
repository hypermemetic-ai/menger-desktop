# menger-desktop's Android build environment (DIST-6).
#
# Ubuntu 24.04 with JDK 17, the Android SDK (cmdline-tools, platform-tools,
# the platform and build-tools tauri-cli's template compiles against), the
# NDK version tauri-cli 2.12 recommends, rustup stable with the four Android
# targets, and tauri-cli. The repository is bind-mounted at /src and the
# build runs as the host's uid (scripts/build-android.sh); the SDK is
# world-writable so Gradle (as that uid) may add a component it finds
# missing — the licenses are accepted here.
#
# Two stages: `build` (what scripts/build-android.sh uses) and `emulator`
# (build + the emulator and an x86_64 system image, for
# scripts/android-emulator.sh — needs /dev/kvm).
FROM ubuntu:24.04 AS build

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl wget unzip file git ca-certificates pkg-config \
        openjdk-17-jdk-headless \
    && rm -rf /var/lib/apt/lists/*

# Versions: tauri-cli 2.12's own (src/mobile/android/mod.rs: NDK_VERSION,
# SDK_VERSION, CMDLINE_TOOLS_URL).
ARG CMDLINE_TOOLS=13114758
ARG ANDROID_PLATFORM=37.0
ARG BUILD_TOOLS=36.0.0
ARG NDK_VERSION=29.0.13846066

ENV JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 \
    ANDROID_HOME=/opt/android-sdk \
    ANDROID_SDK_ROOT=/opt/android-sdk \
    NDK_HOME=/opt/android-sdk/ndk/${NDK_VERSION}
ENV PATH=/opt/android-sdk/cmdline-tools/latest/bin:/opt/android-sdk/platform-tools:$PATH

RUN mkdir -p /opt/android-sdk/cmdline-tools \
    && curl -fsSLo /tmp/cmdline-tools.zip \
        "https://dl.google.com/android/repository/commandlinetools-linux-${CMDLINE_TOOLS}_latest.zip" \
    && unzip -q /tmp/cmdline-tools.zip -d /opt/android-sdk/cmdline-tools \
    && mv /opt/android-sdk/cmdline-tools/cmdline-tools /opt/android-sdk/cmdline-tools/latest \
    && rm /tmp/cmdline-tools.zip \
    && yes | sdkmanager --licenses >/dev/null \
    && sdkmanager --install \
        "platform-tools" \
        "platforms;android-${ANDROID_PLATFORM}" \
        "build-tools;${BUILD_TOOLS}" \
        "ndk;${NDK_VERSION}" >/dev/null \
    && chmod -R a+rwX /opt/android-sdk

ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable --component clippy \
    && rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android \
    && cargo install tauri-cli --version "^2" --locked \
    && rm -rf /opt/cargo/registry /opt/cargo/git \
    && chmod -R a+rX /opt/rustup /opt/cargo

WORKDIR /src/src-tauri

# --- the emulator, for verifying an APK on this machine (needs /dev/kvm) ---
FROM build AS emulator
ARG SYSTEM_IMAGE="system-images;android-35;google_apis;x86_64"
RUN apt-get update && apt-get install -y --no-install-recommends \
        libpulse0 libxcomposite1 libxcursor1 libxdamage1 libxi6 libxtst6 libnss3 \
        libgl1 libegl1 libxkbfile1 libbsd0 iproute2 \
    && rm -rf /var/lib/apt/lists/* \
    && sdkmanager --install "emulator" "${SYSTEM_IMAGE}" >/dev/null \
    && chmod -R a+rwX /opt/android-sdk
ENV PATH=/opt/android-sdk/emulator:$PATH
