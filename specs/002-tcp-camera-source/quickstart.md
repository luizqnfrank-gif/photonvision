# Quickstart & Verification Guide: Local Network TCP Camera Stream Source (`rpicam-vid`)

**Feature**: `002-tcp-camera-source`  
**Date**: 2026-11-23

---

## 1. Starting the `rpicam-vid` TCP Stream on the Raspberry Pi

On the Raspberry Pi with a CSI camera attached, start the TCP listener stream:

```bash
rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888
```

---

## 2. Connecting `fast-photon-core` to the TCP Stream

### Option A: Via Command-Line Flag (`--tcp-camera`)
```bash
cd fast-photon-core
cargo run --release --bin fast-photon-core -- \
  --config-dir ./photonvision_config \
  --port 5800 \
  --tcp-camera tcp://192.168.1.50:8888
```
*(Use `tcp://127.0.0.1:8888` if running `rpicam-vid` on the same machine.)*

### Option B: Via HTTP REST API (`POST /api/settings/camera/addTcpCamera`)
```bash
curl -X POST http://localhost:5800/api/settings/camera/addTcpCamera \
  -H "Content-Type: application/json" \
  -d '{
    "uri": "tcp://192.168.1.50:8888",
    "nickname": "RPiCam_720p",
    "width": 1280,
    "height": 720,
    "fps": 30
  }'
```

---

## 3. Running Automated Tests & Quality Gates

```bash
cd fast-photon-core

# 1. Run the dedicated TCP camera streaming, H.264/MJPEG decoding, and auto-reconnection test suite
cargo test --test tcp_camera_stream_parity

# 2. Run the full workspace test suite
cargo test --workspace

# 3. Verify formatting and strict Clippy lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
