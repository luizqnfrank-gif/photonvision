# Phase 0 Research: `fast-photon-core` Rust Migration

**Feature**: `001-fast-photon-core`  
**Date**: 2026-11-23  
**Status**: Resolved (0 NEEDS CLARIFICATION remaining)

---

## 1. Multi-Crate Cargo Workspace Architecture

### Decision
Organize `fast-photon-core/` as a multi-crate Cargo workspace with 5 domain sub-crates under `fast-photon-core/crates/` and a top-level facade/binary crate:
- `crates/photon-targeting`: Pure-Rust targeting data structures (`PhotonPipelineResult`, `PhotonTrackedTarget`, `MultiTargetPNPResult`, `PnpResult`, `TargetCorner`, `PhotonPipelineMetadata`), Photon binary `Packet` serde (`PacketSerde`), WPILib `Struct` binary layout (`Transform3d`, `Pose3d`, `Translation3d`, `Rotation3d`, `Quaternion`), Protobuf schemas (`prost`), and 3D target estimation math (`nalgebra`).
- `crates/photon-common`: SQLite persistence (`rusqlite` + `DbMigration` steps `2`, `202701`–`202704`), JSON configuration models (`serde`, `serde_json`), `DataChangeService` pub/sub event bus (`tokio::sync::broadcast` / `crossbeam-channel`), NetworkTables v4 client/server & UDP TimeSync protocol (`5810`), structured logging (`tracing`), and zip archive utilities (`zip`).
- `crates/photon-hal`: Coprocessor platform detection (`Platform`, `PiVersion`), GPIO/PWM LED control (`VisionLED`, `StatusLED` with hardware feature flags and mock desktop fallback), OS networking (`nmcli` command builder via `tokio::process::Command` with timeouts), and `SystemMonitor` metrics (`sysinfo` + platform sysfs readers for Raspberry Pi, RK3588, QCS6490, Windows).
- `crates/photon-vision`: OpenCV (`opencv` crate) RAII matrix & contour wrappers (`CVMat`, `Contour`, `CVShape`), `Frame` & bounded frame buffer pools, camera discovery & settables (`v4l` on Linux, OpenCV/`nokhwa` on Windows, feature-gated `libcamera` for CSI, `FileVisionSource`), `CameraQuirk` profiles, all 37 `CVPipe` implementations, all 9 `CVPipeline` implementations, AprilTag/ArUco detectors, OpenCV/`mrcal` 3D calibration, feature-gated `rknn`/`tflite` object detectors (with `NullModel` fallback), and `VisionSourceManager` / `VisionModule` / `VisionRunner` worker threads.
- `crates/photon-web`: Embedded `axum` + `tokio` HTTP REST server (port `5800`), `/websocket_data` MessagePack (`rmp-serde`) WebSocket handler, and per-camera MJPEG HTTP streaming servers (ports `1181+`).
- Top-level `fast-photon-core`: Re-exports all sub-crates as a unified library (`rlib`) and provides the standalone `fast-photon-core` binary (`src/main.rs`).

### Rationale
- Separating pure-Rust crates (`photon-targeting`, `photon-common`, `photon-hal`, `photon-web`) from the C++/OpenCV-linked `photon-vision` crate keeps incremental compile and `cargo test` times under 2 seconds for data, config, SQLite, NT4, and WebSocket tests.
- Mirrors the logical separation in the existing repository (`photon-targeting`, `photon-core/common`, `photon-core/vision`, `photon-server`) while keeping everything isolated under `fast-photon-core/` per Constitution Principle I.

### Alternatives Considered
- **Single monolithic crate**: Rejected because every unit test change in SQLite or NT4 would trigger relinking against OpenCV and native vision libraries, and clarifies Q3 explicitly selected the multi-crate workspace.

---

## 2. Computer Vision, Fiducial Detection & 3D Calibration Stack

### Decision
Use the `opencv` Rust crate (binding to system OpenCV 4.x `core`, `imgproc`, `imgcodecs`, `calib3d`, `objdetect`, `videoio`) combined with `nalgebra` for 3D rigid-body transforms (`Transform3d`, `Rotation3d`, `Translation3d`, coordinate frame conversions between WPILib NWU, OpenCV EDN, and AprilTag frames) and feature-gated C FFI bindings for `apriltag` and `mrcal`.

### Rationale
- `photon-core` relies on exact OpenCV behaviors (`findContours`, `approxPolyDP`, `minAreaRect`, `fitEllipseDirect`, `inRange`, `solvePnPGeneric` / `SOLVEPNP_IPPE_SQUARE` / `SOLVEPNP_SQPNP`, `calibrateCamera`, `objdetect::ArucoDetector`). Using the `opencv` crate guarantees numerical parity within `1e-4` m / `1e-3` rad (SC-001) and automatically manages `cv::Mat` destruction via Rust's `Drop` trait (Constitution Principle II).
- Wrapping `cv::Mat` inside an arena/pool-backed `CVMat` struct in `photon-vision` eliminates manual `Releasable` tracking and prevents use-after-free bugs.

### Alternatives Considered
- **Pure-Rust `imageproc` / `cv-rs`**: Rejected during Clarification Q2 because pure-Rust replacements lack `SOLVEPNP_SQPNP`, ChArUco calibration, and exact OpenCV contour hierarchy parity.

---

## 3. Wire Protocol Compatibility (Packet, Struct, Protobuf, NT4 & TimeSync)

### Decision
Implement byte-exact serializers in `photon-targeting` and `photon-common`:
1. **Photon Binary `Packet`**: Dynamic byte buffer matching `org.photonvision.common.dataflow.structures.Packet` little-endian encoding (`u8`, `i16`, `i32`, `i64`, `f32`, `f64`, variable-length lists with `u8` length prefix, and `u8` boolean presence prefix for optionals) and exact MD5 interface UUIDs (`fd353b11f3a818e9c5b457c7dc5a8f5c` for `PhotonPipelineResult`, `cc6dbb5c5c1e0fa808108019b20863f1` for `PhotonTrackedTarget`).
2. **WPILib `Struct` & `Protobuf`**: Fixed-size 56-byte little-endian `Transform3d` struct (`Translation3d` 3×`f64` + `Rotation3d` Quaternion 4×`f64`) for the `targetPose` NT4 topic, plus `prost`-compiled `photon.proto` and `geometry3d.proto` for `result_proto` and `DeviceMetricsProto`.
3. **NetworkTables v4 & TimeSync (`5810/udp`)**: Pure-Rust async NT4 client/server over WebSockets (`ws://<server>:5810/nt/<client>`) supporting MsgPack value frames and JSON text announcements, paired with a native Rust UDP TimeSync client/server on port `5810` publishing `offset_ns`, `rtt2_ns`, `ping_tx_count`, `pong_rx_count`, and `pong_rx_time_ns` under `/photonvision/.timesync/<hostname>`.

### Rationale
- `photon-lib` (Java, C++, Python) decodes `rawBytes` (`photonstruct:PhotonPipelineResult:<uuid>`), `result_proto`, and scalar/struct topics directly from NT4 and computes capture timestamps using the UDP TimeSync offset. Pure-Rust implementations of these well-defined wire protocols remove JNI/C++ build complexity while ensuring 100% interoperability (SC-002).

### Alternatives Considered
- **Linking `libntcore.so` via C FFI**: Kept as an optional cargo feature if needed, but a native Rust async NT4 + UDP TimeSync implementation integrates cleanly with `tokio` without unmanaged background C++ threads.

---

## 4. Camera Capture Backends & Hardware Quirk Profiles

### Decision
Define a `FrameProvider` and `VisionSourceSettables` trait in `photon-vision` with three concrete providers:
1. **Linux USB (`v4l` crate)**: Zero-copy `mmap` V4L2 ring buffer acquisition and direct V4L2 `ioctl` extended controls (`V4L2_CID_EXPOSURE_AUTO`, `V4L2_CID_EXPOSURE_ABSOLUTE`, `V4L2_CID_GAIN`, `V4L2_CID_WHITE_BALANCE_TEMPERATURE`, ArduCam/OV9281/See3CAM vendor controls) driven by `QuirkyCamera` and `CameraQuirk` flags.
2. **Cross-Platform USB Fallback (`opencv::videoio::VideoCapture`)**: Used on Windows/macOS and for generic USB sources when V4L2 device nodes are not applicable.
3. **CSI Camera (`libcamera` feature-gated FFI)** & **File/Replay (`FileFrameProvider`)**: Loads static images or video sequences from `test-resources/` for deterministic headless testing and simulation.

### Rationale
- Direct V4L2 `mmap` on Linux avoids intermediate CSCore frame copies (addressing the `USBFrameProvider.java:73` TODO) and gives exact control over V4L2 quirk registers for ArduCam OV9281/OV2311/OV9782, InnoMaker, ThriftyCam, LifeCam, and PS3 Eye cameras.

### Alternatives Considered
- **OpenCV `VideoCapture` only**: Rejected in Clarification Q5 because OpenCV's `VideoCapture` strips raw V4L2 sysfs path metadata and vendor control quirks needed on FRC Linux coprocessors.

---

## 5. SQLite Configuration Store & Polymorphic JSON Compatibility

### Decision
Use `rusqlite` (with `bundled` SQLite 3) and `serde_json` to implement `ConfigProvider` and `MigrationManager`:
- Execute `PRAGMA user_version` checks on startup and run the exact migration chain (`2` → `202701` → `202702` → `202703` → `202704`) defined in `DbMigration.java`.
- Use `#[serde(tag = "type")]` (and legacy 2-element tuple array `[type_name, object]` fallback deserializers) for polymorphic `CVPipelineSettings` (`ReflectivePipelineSettings`, `AprilTagPipelineSettings`, `ArucoPipelineSettings`, `ColoredShapePipelineSettings`, `ObjectDetectionPipelineSettings`, `Calibration3dPipelineSettings`, `DriverModePipelineSettings`, `FocusPipelineSettings`) and `PVCameraInfo` (`PVUsbCameraInfo`, `PVCSICameraInfo`, `PVFileCameraInfo`).

### Rationale
- Ensures any `photon.sqlite` database or exported `.zip` from PhotonVision 2024–2027 loads and saves without losing a single field (SC-002).

### Alternatives Considered
- **SQLx / ORM (Diesel/SeaORM)**: Rejected because `photon.sqlite` only has two tables (`global` and `cameras`) storing JSON documents and uses custom imperative JSON-transforming migration steps (`202701`–`202704`).

---

## 6. Embedded Web Server, MessagePack WebSocket & MJPEG Streaming

### Decision
Implement `photon-web` using `axum` and `tokio`:
- **Main HTTP & WebSocket Server (port `5800`)**:
  - Serves `/websocket_data` accepting and broadcasting binary MessagePack (`rmp-serde`) maps matching `DataSocketHandler.java` (`WSMessage { cameraUniqueName, properties }` and `OutgoingUIEvent`).
  - Serves all 36 REST API routes under `/api/status`, `/api/settings/*`, `/api/utils/*`, `/api/calibration/*`, `/api/objectdetection/*`, and `/api/test/*`.
- **MJPEG Stream Servers (ports `1181`, `1182`, `1183`, `1184`, ...)**:
  - Each `VisionModule` spawns lightweight async HTTP `multipart/x-mixed-replace; boundary=frame` endpoints for its assigned `inputStreamPort` and `outputStreamPort`, fed via a latest-frame `tokio::sync::watch` channel from the decoupled `StreamRunnable` worker so slow HTTP clients never backpressure vision processing.

### Rationale
- Directly satisfies Clarification Q4 (Option A) and Constitution Principle IV (decoupling vision hot loops from MJPEG encoding and network I/O).
