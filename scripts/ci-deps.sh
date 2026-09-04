#!/usr/bin/env bash

set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

BINARYEN_VERSION=130

# Determine whether sudo is required.
if [[ "$(id -u)" -eq 0 ]]; then
    SUDO=""
elif command -v sudo >/dev/null 2>&1; then
    SUDO="sudo"
else
    echo "ERROR: Root privileges are required, but sudo is not available"
    exit 1
fi

echo "Running as user: $(id -un)"
echo "UID: $(id -u)"

$SUDO apt-get update

$SUDO apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libclang-dev \
    clang \
    llvm-dev \
    libssl-dev \
    gnupg \
    curl \
    git \
    gpg

if ! command -v wasm-opt >/dev/null 2>&1; then
    echo "Installing Binaryen ${BINARYEN_VERSION}..."

    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    curl -fsSL \
        "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz" \
        | tar -xz -C "$tmp_dir"

    $SUDO cp \
        "$tmp_dir/binaryen-version_${BINARYEN_VERSION}/bin/"* \
        /usr/local/bin/

    echo "Installed wasm-opt: $(command -v wasm-opt)"
fi
