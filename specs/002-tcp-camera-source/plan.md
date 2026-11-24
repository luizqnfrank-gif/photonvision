# Implementation Plan: Local Network TCP Camera Stream Source (`rpicam-vid`)

**Branch**: `002-tcp-camera-source` | **Date**: 2026-11-23 | **Spec**: [`specs/002-tcp-camera-source/spec.md`](file:///home/rose/Documents/workspace_local/photonvision/specs/002-tcp-camera-source/spec.md)
**Input**: Feature specification from `/specs/002-tcp-camera-source/spec.md`

## Summary

Add a first-class TCP network camera source (`CameraType::TcpCamera`, `PVCameraInfo::Tcp`, `TcpCameraSource`, and `TcpFrameProvider`) to `fast-photon-core` to ingest live video streams from `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888` over a local network. The implementation connects as a TCP client by default (with optional `?listen=true` server mode), decodes both inline H.264 Annex-B (`--inline` SPS/PPS/IDR/P-frames via the pure-Rust `openh264` crate) and MJPEG-over-TCP (`image` crate) in-process on a dedicated background reader thread, provides latest-frame (`< 1` frame delay) non-blocking handoff to `VisionRunner`, automatically reconnects on network drops, persists in `photon.sqlite`, and exposes CLI (`--tcp-camera`) and REST (`POST /api/settings/camera/addTcpCamera`) registration.

## Technical Context

**Language/Version**: Rust (Stable, Edition 2021 / `rustc >= 1.80`)  
**Primary Dependencies**:
- **H.264 & MJPEG Decoding**: `openh264` (`0.6`, in-process H.264 Annex-B decoder & test encoder), `image` (`0.25`, JPEG SOI/EOI decoding & RGB matrix conversion)
- **Existing Workspace Stack**: `photon-common` (`rusqlite`, `serde`, `serde_json`), `photon-vision` (`CVMat`, `Frame`, `VisionModule`, `VisionSourceManager`), `photon-web` (`axum`, `tokio`), `clap`
**Storage**: SQLite (`photon.sqlite`, `PVCameraInfo.PVTcpCameraInfo` JSON tagged union in `cameras` table)  
**Testing**: `cargo test --test tcp_camera_stream_parity` (end-to-end TCP server/client streaming for H.264 `--inline` Annex-B and MJPEG, vision pipeline execution, SQLite persistence, REST registration, and drop/auto-reconnection)  
**Target Platform**: `linux-arm64` (Raspberry Pi 4/5, Orange Pi 5 RK3588, Rubik Pi 3 QCS6490), `linux-x86_64`, `windows-x86_64`  
**Project Type**: Multi-crate Cargo workspace extension inside `fast-photon-core/`  
**Performance Goals**: Sustain 1280×720 @ 30 FPS stream ingestion with `< 1` frame queuing delay (latest-frame overwrite semantics) and automatic reconnection within `<= 2.0` seconds of server restart  
**Constraints**: Bounded stream buffer (`<= 8 MiB`), zero `unsafe` blocks (`unsafe_code = "deny"`), zero runtime `unwrap()`/`expect()` panics on network disconnects or malformed bytes  
**Scale/Scope**: Touches `photon-common` (camera configuration enum), `photon-vision` (new `camera/tcp.rs` module + `frame/provider.rs` + `VisionSourceManager`), `photon-web` (new REST handler & route), `fast-photon-core/src/main.rs` (`--tcp-camera` CLI flag), and adds `fast-photon-core/tests/tcp_camera_stream_parity.rs`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **I. Workspace Isolation & Parity**: All changes reside strictly inside `fast-photon-core/`; legacy Java `photon-core/` remains untouched.
- [x] **II. Memory Safety & Deterministic Resource Lifecycle**: Decoded frames are wrapped in RAII `CVMat` (with atomic `Drop` tracking); TCP accumulator buffer is strictly bounded (`8 MiB`); `unsafe_code = "deny"` is preserved across all workspace crates.
- [x] **III. Wire, Schema & Protocol Compatibility**: `PVCameraInfo::Tcp` extends the `#[serde(tag = "type")]` enum as `"PVCameraInfo.PVTcpCameraInfo"` without breaking existing USB, CSI, or File camera entries in `photon.sqlite`, NT4, or WebSocket UI DTOs.
- [x] **IV. High-Throughput Concurrency & Zero-Copy Hot Paths**: TCP socket reading and H.264/MJPEG decoding run on a dedicated background thread per TCP camera, passing only the latest decoded `CVMat` to `VisionRunner` via a shared lock-free/latest-value slot so network jitter never blocks pipeline execution or NT4 publishing.
- [x] **V. Test-Driven Parity & Benchmarking Discipline**: Full automated integration test suite (`tests/tcp_camera_stream_parity.rs`) verifies live TCP H.264 Annex-B (`--inline`) and MJPEG streaming, pipeline target detection, SQLite persistence, and disconnect/reconnect recovery.

## Project Structure

### Documentation (this feature)

```text
specs/002-tcp-camera-source/
├── plan.md                                      # This file (/04-speckit.plan output)
├── research.md                                  # Phase 0 protocol, H.264/MJPEG & concurrency decisions
├── data-model.md                                # Phase 1 entity schemas & state transitions
├── quickstart.md                                # Phase 1 rpicam-vid setup, CLI/REST & test guide
├── contracts/
│   └── tcp-camera-contracts.md                  # rpicam-vid TCP wire, SQLite JSON, REST & CLI contracts
└── tasks.md                                     # Phase 2 output (/05-speckit.tasks command)
```

### Source Code (repository root)

```text
fast-photon-core/
├── Cargo.toml                                   # Add openh264 workspace dependency
├── src/
│   └── main.rs                                  # Add --tcp-camera <URI> CLI argument
├── crates/
│   ├── photon-common/
│   │   └── src/
│   │       └── configuration/
│   │           └── camera.rs                    # Add CameraType::TcpCamera & PVCameraInfo::Tcp
│   ├── photon-vision/
│   │   ├── Cargo.toml                           # Add openh264 dependency
│   │   └── src/
│   │       ├── camera/
│   │       │   ├── mod.rs                       # Export tcp module
│   │       │   └── tcp.rs                       # NEW: TcpStreamEndpoint, TcpStreamDecoder, TcpFrameProvider, TcpCameraSource
│   │       ├── frame/
│   │       │   └── provider.rs                  # Re-export TcpFrameProvider
│   │       └── processes/
│   │           └── vision_source_manager.rs     # Instantiate TcpCameraSource for CameraType::TcpCamera
│   └── photon-web/
│       └── src/
│           ├── handlers.rs                      # Add on_add_tcp_camera_request handler
│           └── server.rs                        # Register POST /api/settings/camera/addTcpCamera
└── tests/
    └── tcp_camera_stream_parity.rs              # NEW: End-to-end H.264/MJPEG TCP streaming, reconnection & pipeline tests
```

**Structure Decision**: Extends the existing `fast-photon-core/` multi-crate workspace by adding `crates/photon-vision/src/camera/tcp.rs` alongside `usb.rs` and `csi.rs`, keeping camera transport and codec logic cleanly encapsulated inside `photon-vision`.

## Complexity Tracking

No Constitution Check violations exist. All 5 principles pass without exception.
