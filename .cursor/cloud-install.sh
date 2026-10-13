#!/usr/bin/env bash
# Prepare compilers and dependencies for the native app and the web reference.
# Idempotent. Do not start servers here.
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential \
  pkg-config \
  cmake \
  clang \
  libclang-dev \
  libssl-dev \
  libxkbcommon-dev \
  libxkbcommon-x11-dev \
  libwayland-dev \
  libvulkan-dev \
  mesa-vulkan-drivers \
  vulkan-tools \
  libx11-dev \
  libx11-xcb-dev \
  libxcb1-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libxrandr-dev \
  libxi-dev \
  libxcursor-dev \
  libfontconfig1-dev \
  libfreetype6-dev \
  libasound2-dev \
  libdbus-1-dev \
  libsqlite3-dev \
  postgresql \
  postgresql-contrib \
  ca-certificates \
  curl \
  git

# Vendored GPUI uses Rust edition 2024. The default image toolchain is too old.
# Pin the compiler that this environment was verified with.
rustup toolchain install 1.99.0 --profile minimal -c rustfmt -c clippy
rustup default 1.99.0

# package-lock.json omits optional bufferutil, so npm ci rejects the tree.
npm install --package-lock=false --no-audit --no-fund

cargo fetch --locked --manifest-path desktop/Cargo.toml

rustc --version
cargo --version
node --version
