#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(CDPATH="" cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$WORKSPACE_DIR"

HOST_ARCH="$(uname -m)"
BIN_NAME="${BIN_NAME:-fast-phontonvision-core}"
FEATURES="${FEATURES:-jetson-orin-nano}"

echo "=== Building ${BIN_NAME} for NVIDIA Jetson Orin Nano (Cortex-A78AE / aarch64) ==="

if [[ "$HOST_ARCH" == "aarch64" || "$HOST_ARCH" == "arm64" ]]; then
    echo "Detected native aarch64 host (Jetson Orin Nano). Running native release build..."
    RUSTFLAGS="${RUSTFLAGS:--C target-cpu=cortex-a78ae}" \
        cargo build --release --bin "$BIN_NAME" --features "$FEATURES"
    echo "Built binary: $WORKSPACE_DIR/target/release/$BIN_NAME"
else
    echo "Detected $HOST_ARCH host. Cross-compiling for target aarch64-unknown-linux-gnu..."
    if command -v cross >/dev/null 2>&1 && ([[ "${USE_CROSS:-0}" == "1" ]] || ! command -v "${CC_aarch64_unknown_linux_gnu:-aarch64-linux-gnu-gcc}" >/dev/null 2>&1); then
        cross build --release --target aarch64-unknown-linux-gnu --bin "$BIN_NAME" --features "$FEATURES"
    else
        if ! rustup target list --installed | grep -q "^aarch64-unknown-linux-gnu$"; then
            echo "Installing Rust target aarch64-unknown-linux-gnu..."
            rustup target add aarch64-unknown-linux-gnu
        fi
        export CC_aarch64_unknown_linux_gnu="${CC_aarch64_unknown_linux_gnu:-aarch64-linux-gnu-gcc}"
        export CXX_aarch64_unknown_linux_gnu="${CXX_aarch64_unknown_linux_gnu:-aarch64-linux-gnu-g++}"
        export AR_aarch64_unknown_linux_gnu="${AR_aarch64_unknown_linux_gnu:-aarch64-linux-gnu-ar}"
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="${CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER:-aarch64-linux-gnu-gcc}"
        cargo build --release --target aarch64-unknown-linux-gnu --bin "$BIN_NAME" --features "$FEATURES"
    fi
    echo "Built binary: $WORKSPACE_DIR/target/aarch64-unknown-linux-gnu/release/$BIN_NAME"
fi
