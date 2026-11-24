# Quickstart & Verification Guide: `fast-photon-core`

**Feature**: `001-fast-photon-core`  
**Date**: 2026-11-23

---

## 1. Prerequisites

- **Rust Toolchain**: Stable Rust (`rustc >= 1.80`, `cargo`, `rustfmt`, `clippy`)
- **System Libraries**:
  - OpenCV 4.x development headers (`libopencv-dev` on Ubuntu/Debian, `clang`/`libclang-dev` for `opencv` crate binding generation)
  - SQLite 3 (bundled automatically via `rusqlite/bundled`)
  - Linux V4L2 headers (`libv4l-dev` on Linux)
- **Optional Hardware Features**:
  - `--features rknn` (Rockchip RK3588 NPU inference)
  - `--features tflite` (TensorFlow Lite CPU/delegate inference)
  - `--features libcamera` (Raspberry Pi CSI Libcamera GL capture)
  - `--features gpio` (Linux GPIO/PWM LED hardware control)

---

## 2. Building the Workspace

All Rust migration code lives inside `fast-photon-core/`:

```bash
cd fast-photon-core

# Build all workspace crates (default desktop-compatible feature set)
cargo build --workspace --release

# Build with embedded coprocessor hardware features (Linux ARM64)
cargo build --workspace --release --features "rknn,tflite,libcamera,gpio"
```

---

## 3. Running Quality Gates & Parity Tests

```bash
cd fast-photon-core

# 1. Formatting check (Constitution Gate)
cargo fmt --all -- --check

# 2. Strict Clippy linting (Constitution Gate)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Fast Pure-Rust Unit & Wire-Protocol Tests (Targeting, SQLite Migrations, NT4, Web)
cargo test -p photon-targeting -p photon-common -p photon-hal -p photon-web

# 4. Full Vision Pipeline & Calibration Parity Suite against ../test-resources/
cargo test --workspace
```

---

## 4. Running the Standalone `fast-photon-core` Engine

```bash
cd fast-photon-core

# Run with an existing photon.sqlite config directory
cargo run --release --bin fast-photon-core -- \
  --config-dir ../test-resources/config \
  --port 5800
```

Once started:
- **Web UI & REST API**: `http://localhost:5800` (`/api/status`, `/websocket_data`)
- **MJPEG Camera Streams**: `http://localhost:1181/stream.mjpg` (Camera 0 Input), `http://localhost:1182/stream.mjpg` (Camera 0 Output)
- **NetworkTables v4**: Connects to RoboRIO NT4 server (or hosts NT4 server on port `5810` when `runNTServer: true`) and runs UDP TimeSync on port `5810`.
