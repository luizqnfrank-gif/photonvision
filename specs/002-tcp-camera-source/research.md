# Phase 0 Research: Local Network TCP Camera Stream Source (`rpicam-vid`)

**Feature**: `002-tcp-camera-source`  
**Date**: 2026-11-23  
**Status**: Resolved (0 NEEDS CLARIFICATION remaining)

---

## 1. `rpicam-vid` TCP Stream Wire Protocol & Connection Topology

### Decision
Implement `TcpStreamEndpoint` in `photon-vision` supporting two connection modes parsed from a `tcp://<host>:<port>[?listen=true]` URI:
1. **Default Outbound TCP Client (`tcp://<host>:8888`)**: Connects via `std::net::TcpStream::connect_timeout` (default `500 ms` connect timeout, `1000 ms` read timeout) to a remote or local Raspberry Pi running `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888`.
2. **Optional Inbound TCP Listener (`tcp://0.0.0.0:8888?listen=true`)**: Binds a local `TcpListener` and accepts incoming push connections when `rpicam-vid` is run without `--listen`.

### Rationale
- When `rpicam-vid` is invoked with `--listen -o tcp://0.0.0.0:8888`, it opens a listening TCP socket on port `8888` and begins streaming encoded frames as soon as a TCP client connects.
- Supporting outbound client mode by default (and `?listen=true` via query parameter) satisfies Clarification Q1 (Option A) and works seamlessly across localhost testing and multi-Pi LAN robot networks.

### Alternatives Considered
- **UDP / RTP only**: Rejected because the user's explicit command uses `-o tcp://0.0.0.0:8888` with `--listen`.

---

## 2. In-Process H.264 Annex-B (`--inline`) & MJPEG Frame Delimiting and Decoding

### Decision
Implement a unified, zero-`unsafe` stream parser (`TcpStreamDecoder`) in `photon-vision/src/camera/tcp.rs` that auto-detects and decodes both H.264 Annex-B (`--inline`) and MJPEG-over-TCP streams in-process:
1. **H.264 Annex-B Elementary Stream (`openh264` crate)**:
   - `rpicam-vid --inline` emits Annex-B start codes (`0x00 0x00 0x00 0x01` or `0x00 0x00 0x01`) with inline SPS (`NAL type 7`), PPS (`NAL type 8`), and IDR (`NAL type 5`) or non-IDR slice (`NAL type 1`) units.
   - `openh264::decoder::Decoder` (using `openh264::nal_units` packetization) decodes complete access units into YUV420p and converts them directly to `RgbImage` / `CVMat`.
   - For late-joining clients connecting mid-stream, the parser discards leading bytes until the first SPS (`0x67` / `nal_unit_type == 7`) start code is encountered.
2. **MJPEG / JPEG Stream (`image` crate)**:
   - Scans for JPEG SOI (`0xFF 0xD8`) and EOI (`0xFF 0xD9`) markers in the byte buffer and decodes complete JPEG frames via `image::load_from_memory_with_format(..., ImageFormat::Jpeg)`.
3. **Bounded Ring / Accumulator Buffer (`MAX_STREAM_BUFFER_BYTES = 8 MiB`)**:
   - Protects against unbounded memory growth on corrupted streams by truncating/resetting to the latest start code if buffer capacity exceeds `8 MiB`.

### Rationale
- Directly satisfies Clarification Q2 (Option A) and Constitution Principle II (`unsafe_code = "deny"`, deterministic memory bounds).
- Bundling `openh264` and `image` requires zero external runtime binaries (`ffmpeg`/`gstreamer` are not required on the target coprocessor) and enables deterministic unit and integration testing in `cargo test` using both synthetic H.264 Annex-B bitstreams (`openh264::encoder::Encoder` or pre-encoded NAL units) and MJPEG streams.

### Alternatives Considered
- **Spawning `ffmpeg` child process**: Rejected in Clarification Q2 because external process management adds fragile OS dependencies and extra pipe copy latency.

---

## 3. Decoupled Reader Thread, Latest-Frame Semantics & Auto-Reconnection

### Decision
Run a dedicated background reader thread inside `TcpFrameProvider` that shares the latest decoded `CVMat` with `VisionRunner` via a lock-free/latest-value `tokio::sync::watch::channel<Option<CVMat>>` (or `Arc<RwLock<Option<CVMat>>>` + atomic sequence counter and `AtomicBool` connection flag):
- **Non-Blocking Startup & Latest-Frame Handoff**: `TcpFrameProvider::new` spawns the reader thread immediately without blocking startup. When `VisionRunner` calls `get_frame()`, it clones the latest decoded `CVMat` in `O(1)` time without waiting on network I/O. If the pipeline runs slower than 30 FPS, intermediate frames are overwritten in the watch slot so queue latency is always `< 1` frame (`SC-001`).
- **Dynamic `FrameStaticProperties` Resolution Sync**: If a decoded frame's `(width, height)` differs from the initial `(1280, 720)` configuration, `TcpFrameProvider` rebuilds `Arc<FrameStaticProperties>` for the new dimensions and looks up any matching `CameraCalibrationCoefficients`.
- **Automatic Reconnection Loop**: On EOF, socket error, or read timeout (`> 1.0 s`), the reader thread sets `connected = false`, closes the socket, sleeps for a short backoff (`150 ms`), and reconnects to `tcp://<host>:<port>` (or re-accepts on the listener) until `stop()` / `Drop` is triggered (`SC-003`).

### Rationale
- Satisfies Constitution Principle IV (`High-Throughput Concurrency & Zero-Copy Hot Paths`) and `FR-004`/`FR-005` by isolating network jitter and reconnection stalls from the vision pipeline worker thread.

---

## 4. Schema, CLI & REST API Integration

### Decision
1. **`photon-common` Configuration Models**:
   - Add `CameraType::TcpCamera`.
   - Add `PVCameraInfo::Tcp` (tagged as `"PVCameraInfo.PVTcpCameraInfo"` with alias `"PVTcpCameraInfo"`) with fields `uri: String`, `name: String`, `width: i32` (default `1280`), `height: i32` (default `720`), `fps: i32` (default `30`), and `unique_path: String`.
2. **`photon-vision` Source & Manager**:
   - Implement `TcpCameraSource` and `TcpCameraSettables` in `crates/photon-vision/src/camera/tcp.rs`.
   - Update `VisionSourceManager::load_vision_source_from_cam_config` to map `CameraType::TcpCamera` → `Box::new(TcpCameraSource::new(config))`.
3. **`photon-web` & Standalone CLI**:
   - Add `POST /api/settings/camera/addTcpCamera` in `photon-web` (accepting `{ "uri": "tcp://...", "nickname": "...", "width": 1280, "height": 720, "fps": 30 }`) in addition to supporting `PVCameraInfo::Tcp` in `POST /api/utils/assignUnmatchedCamera`.
   - Add repeatable `--tcp-camera <URI>` CLI flag in `fast-photon-core/src/main.rs` so users can launch `fast-photon-core --tcp-camera tcp://192.168.1.50:8888` directly.

### Rationale
- Preserves backward compatibility with existing `photon.sqlite` databases (`user_version = 202704`) while giving users CLI, REST, and persistent SQLite control (Clarification Q3, Option A).
