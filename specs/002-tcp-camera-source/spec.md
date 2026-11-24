# Feature Specification: Local Network TCP Camera Stream Source (`rpicam-vid`)

**Feature Branch**: `002-tcp-camera-source`  
**Created**: 2026-11-23  
**Status**: Draft  
**Input**: User description: "add camera source : get tcp camera streaming via local network by command : rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888"

## Clarifications

### Session 2026-11-23

- Q: TCP Connection Mode (`rpicam-vid --listen` vs. Listener/Dual Mode) → A: Option A (TCP Client Default + Optional Listen Mode: connect as a TCP client to `tcp://<host>:<port>` matching `rpicam-vid --listen`, and also support `?listen=1` / `?listen=true` if `fast-photon-core` should bind and wait for an incoming push stream).
- Q: H.264 / MJPEG Stream Decoding Strategy in Rust → A: Option A (In-Process `openh264` Crate + MJPEG Auto-Detection: bundle the `openh264` crate for zero-dependency H.264 Annex-B decoding of `--inline` SPS/PPS/IDR/P-frames alongside built-in JPEG SOI/EOI decoding via the `image` crate).
- Q: Registration & Configuration Interface for TCP Camera Endpoints → A: Option A (CLI Flag + REST API + SQLite Persistence: support `--tcp-camera <tcp://host:port>` on the CLI, REST registration via `POST /api/utils/assignUnmatchedCamera` & `POST /api/settings/camera/addTcpCamera`, and automatic persistence in `photon.sqlite`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Connect to a Remote `rpicam-vid` TCP Camera Stream (Priority: P1)

An FRC robotics team or vision developer runs a Raspberry Pi camera node on the local network streaming video via `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888`. They configure a TCP camera source in `fast-photon-core` pointing to the remote host and port (e.g., `tcp://192.168.1.50:8888` or `tcp://127.0.0.1:8888`). `fast-photon-core` connects to the TCP listener, decodes incoming frames at 1280×720 @ 30 FPS in real time, and feeds them directly into any of the 9 vision pipelines (AprilTag, ArUco, Reflective, ColoredShape, ObjectDetection, Calibrate3d, DriverMode, Focus, OutputStream).

**Why this priority**: Ingesting and decoding live frames from a networked `rpicam-vid` TCP endpoint is the core capability requested by the user, allowing distributed camera nodes or remote desktop processing over LAN without direct local USB/CSI attachment.

**Independent Test**: Can be fully tested by launching a local TCP stream server (simulating `rpicam-vid --inline --listen -o tcp://0.0.0.0:8888`), connecting a `TcpCameraSource` / `TcpFrameProvider` to `tcp://127.0.0.1:8888`, and verifying that decoded `1280×720` `Frame` instances flow through a `VisionModule` and produce valid `CVPipelineResult` outputs.

**Acceptance Scenarios**:

1. **Given** a remote or local camera node listening on `tcp://<host>:8888` via `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888`, **When** a TCP camera source is registered with `fast-photon-core`, **Then** the frame provider establishes a TCP client connection, synchronizes on the inline stream header (`--inline` SPS/PPS or MJPEG frame boundaries), and delivers decoded `1280×720` RGB/grayscale `Frame`s to the active `VisionRunner`.
2. **Given** an active TCP camera `VisionModule`, **When** vision pipelines execute on incoming TCP frames, **Then** 2D/3D target localization results are published to NetworkTables v4, WebSocket UI subscribers, and output MJPEG streams (`1181+`) identically to physical USB or CSI cameras.

---

### User Story 2 - Persist, Configure & Manage TCP Camera Sources via CLI, REST & SQLite (Priority: P2)

A user adds, saves, deactivates, reactivates, or deletes a TCP camera source (`tcp://<host>:<port>`) through the `fast-photon-core` CLI, REST API (`/api/utils/assignUnmatchedCamera`), or `photon.sqlite` database. Across application restarts, `fast-photon-core` automatically reloads the saved TCP camera configuration from `photon.sqlite` (preserving its `uniqueName`, nickname, FOV, calibrations, and pipeline settings) and reconnects to the configured TCP stream URL.

**Why this priority**: A camera source must integrate cleanly with PhotonVision's existing configuration persistence (`photon.sqlite`) and lifecycle management (`VisionSourceManager`) so users do not have to re-enter stream URLs or recalibrate on every restart.

**Independent Test**: Can be tested by creating a `CameraConfiguration` with a `PVTcpCameraInfo` descriptor (`tcp://127.0.0.1:8888`), saving it to `photon.sqlite` via `ConfigProvider`, reloading the database, and verifying that `VisionSourceManager` instantiates a `TcpCameraSource` with all settings and calibrations intact.

**Acceptance Scenarios**:

1. **Given** a `PVCameraInfo::Tcp` camera descriptor containing a TCP endpoint URL (e.g., `tcp://192.168.1.100:8888`), resolution (`1280×720`), and framerate (`30`), **When** saved by `ConfigProvider` into `photon.sqlite` and reloaded, **Then** the JSON serialization and deserialization round-trip losslessly alongside existing USB, CSI, and File camera configurations.
2. **Given** an unassigned TCP camera URL submitted via CLI flag (`--tcp-camera tcp://127.0.0.1:8888`) or REST API (`/api/utils/assignUnmatchedCamera`), **When** processed by `VisionSourceManager`, **Then** a new active `VisionModule` is created, assigned MJPEG stream ports, and persisted to disk.
3. **Given** an active TCP camera module, **When** deactivated via `VisionSourceManager::deactivate_vision_source`, **Then** the underlying TCP socket connection is closed immediately and re-opened when reactivated.

---

### User Story 3 - Automatic Stream Reconnection & Drop Resilience (Priority: P2)

During robot operation or bench testing, the remote Raspberry Pi running `rpicam-vid` restarts, or the local network experiences a temporary packet drop/disconnect. `fast-photon-core` detects the broken TCP stream or read timeout without panicking or blocking other cameras, marks the camera source as disconnected while retaining the last valid frame or safe fallback frame, and automatically reconnects when the `tcp://<host>:<port>` listener becomes available again.

**Why this priority**: Networked TCP video streams over robot Ethernet/Wi-Fi are subject to power cycles and link interruptions; resilient non-blocking reconnection is essential for field reliability.

**Independent Test**: Can be tested by starting a mock TCP stream server on an ephemeral port, connecting `TcpFrameProvider`, dropping the server connection mid-stream, verifying `is_connected()` transitions cleanly without panicking, restarting the server, and confirming frame delivery resumes automatically.

**Acceptance Scenarios**:

1. **Given** an active TCP camera stream that drops or times out mid-frame, **When** `VisionRunner` polls `get_frame()`, **Then** `TcpFrameProvider` catches the socket error, updates connection status to disconnected without panicking, and attempts non-blocking background reconnection with bounded backoff.
2. **Given** a disconnected TCP camera source, **When** the remote `rpicam-vid --listen -o tcp://0.0.0.0:8888` process restarts and begins accepting connections again, **Then** `TcpFrameProvider` reconnects automatically, resynchronizes on the next `--inline` keyframe/frame boundary, and resumes pipeline processing.

---

### Edge Cases

- **Late-Joining Client on `--inline` Stream**: Because `rpicam-vid` is invoked with `--inline` (which repeats H.264 SPS/PPS headers before every IDR keyframe) and `--listen`, a client connecting or reconnecting mid-stream MUST discard partial leading bytes until the first valid NAL unit (`0x00 0x00 0x00 0x01` / `0x00 0x00 0x01` SPS/PPS/IDR) or JPEG SOI (`0xFF 0xD8`) marker is encountered.
- **Unreachable Host or Refused Port on Startup**: If `tcp://<host>:8888` is offline when `fast-photon-core` boots, startup MUST NOT hang; connect attempts MUST use a bounded connection timeout (e.g., `<= 500 ms`) and retry asynchronously.
- **Fragmented or Coalesced TCP Reads**: TCP is a byte-stream protocol without message boundaries; a single `read()` call may contain a fraction of a frame or multiple back-to-back frames. The stream parser MUST buffer and frame-delimit incoming chunks deterministically without unbounded buffer growth (capping buffer size to prevent OOM on malformed streams).
- **Resolution or Framerate Change**: If the incoming decoded frame dimensions differ from the initial `1280×720` default, `FrameStaticProperties` (width, height, center point, focal length, and matching calibration lookup) MUST update dynamically to match the actual decoded frame resolution.
- **Slow Pipeline / Backpressure**: If a vision pipeline runs slower than the incoming 30 FPS stream, the TCP reader MUST drop stale buffered frames (keeping only the latest decoded frame) so vision latency does not accumulate over time.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (TCP Camera Type & Descriptor)**: `fast-photon-core` MUST add a `TcpCamera` variant to `CameraType` and a `PVCameraInfo::Tcp` variant (serialized as `"PVCameraInfo.PVTcpCameraInfo"` / `"PVTcpCameraInfo"`) storing the TCP URI (e.g., `tcp://192.168.1.50:8888`), display name, configured width (default `1280`), height (default `720`), and framerate (default `30`).
- **FR-002 (TCP Stream Connection & Framing)**: `fast-photon-core` MUST implement `TcpFrameProvider` (and `TcpCameraSource` / `TcpCameraSettables`) capable of connecting as a TCP client to a remote or local `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888` endpoint (`tcp://<host>:<port>`), as well as supporting optional TCP listener mode (`tcp://0.0.0.0:<port>?listen=true`) for incoming push streams, framing continuous byte streams into discrete video frames.
- **FR-003 (Inline Stream Decoding)**: `TcpFrameProvider` MUST automatically detect and decode both inline H.264 Annex-B byte streams (with `--inline` SPS/PPS/IDR/P-frame NAL units produced by `rpicam-vid`, decoded in-process via the `openh264` crate) and raw MJPEG-over-TCP (`--codec mjpeg` SOI `0xFF 0xD8`..EOI `0xFF 0xD9`, decoded via the `image` crate) into `CVMat` / `Frame` instances.
- **FR-004 (Latest-Frame Buffering & Low Latency)**: `TcpFrameProvider` MUST decouple TCP socket reading/decoding from `VisionRunner::step_once` so that `get_frame()` always retrieves the most recently decoded frame without blocking the pipeline thread on network stalls or building up multi-frame latency queues.
- **FR-005 (Automatic Reconnection & Timeouts)**: `TcpFrameProvider` MUST enforce bounded connect and read timeouts, report connection health via `FrameProvider::is_connected()`, and automatically reconnect when a dropped `rpicam-vid --listen` stream becomes available again.
- **FR-006 (VisionSourceManager, CLI & REST Integration)**: `VisionSourceManager` MUST instantiate `TcpCameraSource` for `CameraType::TcpCamera` configurations, support dynamic assignment via `POST /api/utils/assignUnmatchedCamera` and `POST /api/settings/camera/addTcpCamera`, persist configured TCP cameras in `photon.sqlite`, and the `fast-photon-core` standalone binary MUST accept a repeatable `--tcp-camera <tcp://host:port>` CLI flag to register TCP stream sources directly from the command line.
- **FR-007 (Memory Safety & Zero Regressions)**: All TCP stream buffers and decoded frames MUST use RAII `CVMat` lifecycle management with bounded buffer capacities, zero `unsafe` blocks (`unsafe_code = "deny"`), and zero runtime panics on malformed network bytes or abrupt socket disconnects.

### Assumptions

- **Network Topology**: `rpicam-vid` runs with `--listen -o tcp://0.0.0.0:<port>` (acting as a TCP server waiting for a client connection), and `fast-photon-core` acts as the TCP client connecting to `tcp://<camera-ip>:<port>`.
- **Default Stream Parameters**: When not explicitly overridden in `CameraConfiguration` or the video mode settables, TCP camera sources default to the parameters in the user's command (`1280×720` resolution at `30` FPS).
- **Camera Controls on Networked Streams**: Because `rpicam-vid` controls sensor exposure/gain on the remote sender side via CLI arguments, `TcpCameraSettables` stores requested exposure/gain/video-mode properties locally in configuration and updates `FrameStaticProperties` (FOV, resolution) without requiring remote V4L2 `ioctl` calls.

### Key Entities

- **`PVCameraInfo::Tcp` (`PVTcpCameraInfo`)**: Persistent camera descriptor holding `uri` (`tcp://host:port`), `name`, `width` (`1280`), `height` (`720`), `fps` (`30`), and `unique_path`.
- **`TcpFrameProvider`**: `FrameProvider` implementation managing the TCP socket connection, stream framing/NAL/JPEG parser, latest-frame watch buffer, connection health state, and auto-reconnection loop.
- **`TcpCameraSource` & `TcpCameraSettables`**: `VisionSource` and `VisionSourceSettables` implementation binding a `CameraConfiguration` (`CameraType::TcpCamera`) to `TcpFrameProvider` for execution inside a `VisionModule`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (1280×720 @ 30 FPS Stream Ingestion)**: `fast-photon-core` sustains 30 FPS frame ingestion and decoding from a `1280×720` TCP stream (`tcp://<host>:8888`) with < 1 frame of queuing delay (latest-frame semantics).
- **SC-002 (Pipeline & Telemetry Compatibility)**: 100% of vision pipelines (including `AprilTagPipeline`, `ReflectivePipeline`, and `OutputStreamPipeline`) execute on frames from `TcpFrameProvider` and publish valid `PhotonPipelineResult` packets to NetworkTables v4 and WebSocket UI clients.
- **SC-003 (Fault Recovery & Reconnection)**: When a TCP stream server disconnects and restarts, `TcpFrameProvider` detects the disconnect within `1.0` second and automatically recovers frame delivery within `2.0` seconds of the server listening again, with zero process panics.
- **SC-004 (Configuration & Persistence Parity)**: `CameraConfiguration` entries with `PVCameraInfo::Tcp` round-trip losslessly through `photon.sqlite` and `.zip` settings archives, and all workspace quality gates (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`) pass with zero warnings.
