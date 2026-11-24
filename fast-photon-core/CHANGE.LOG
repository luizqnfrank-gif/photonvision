# Changelog — `fast-photon-core` (`fast-phontonvision-core`)

All notable changes to the `fast-photon-core` pure-Rust PhotonVision migration workspace are documented in this file.

---

## [2027.0.0-rust.2] - 2026-11-23

### Added — NVIDIA Jetson Orin Nano Build & Platform Configuration
- **Binary Targets (`Cargo.toml`, `src/bin/`)**:
  - Added `fast-phontonvision-core` (and `fast-photonvision-core`) binary targets sharing the unified `fast_photon_core::run_cli()` entrypoint.
- **Jetson Orin Nano Feature & CPU Tuning (`.cargo/config.toml`, `Cross.toml`, `scripts/build-jetson-orin-nano.sh`)**:
  - Added `--features jetson-orin-nano` enabling `photon-hal/jetson-orin-nano`, `photon-vision/jetson-orin-nano`, `tflite`, and `gpio`.
  - Configured `[target.aarch64-unknown-linux-gnu]` with `linker = "aarch64-linux-gnu-gcc"` and `rustflags = ["-C", "target-cpu=cortex-a78ae"]` targeting the Jetson Orin Nano 6-core ARM Cortex-A78AE CPU.
  - Added Cargo aliases `cargo build-jetson-native` and `cargo build-jetson-orin-nano`, plus `scripts/build-jetson-orin-nano.sh` for one-command native or cross-compiled release builds.
- **Jetson Orin Nano Hardware Abstraction (`crates/photon-hal`)**:
  - Added `Platform::LinuxJetsonOrinNano` detection via `/proc/device-tree/model` (`jetson` / `orin`) and `cfg!(feature = "jetson-orin-nano")`.
  - Added NVIDIA Tegra Orin (`ga10b`) sysfs GPU utilization reader (`read_jetson_gpu_utilization()`) in `PlatformSystemMonitor` and enabled `NetworkManager` `nmcli` management on `Platform::LinuxJetsonOrinNano`.

### Added — Local Network TCP Camera Stream Source (`002-tcp-camera-source`)
- **`rpicam-vid` TCP Stream Ingestion (`crates/photon-vision/src/camera/tcp.rs`)**:
  - Added `TcpStreamEndpoint`, `TcpStreamDecoder`, `TcpFrameProvider`, `TcpCameraSettables`, and `TcpCameraSource` to ingest live `1280×720 @ 30 FPS` streams from `rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888`.
  - Supports outbound TCP client mode (`tcp://<host>:8888`) by default and optional inbound listener mode (`tcp://0.0.0.0:8888?listen=true`).
  - Added in-process H.264 Annex-B decoding (`openh264` crate for `--inline` SPS/PPS/IDR/slice NAL units with late-joining SPS synchronization) and automatic MJPEG SOI/EOI (`0xFF 0xD8`..`0xFF 0xD9`) frame decoding (`image` crate) with an `8 MiB` buffer cap (`MAX_STREAM_BUFFER_BYTES`).
  - Decoupled TCP socket reading and decoding onto a background thread writing the latest `CVMat` slot (`< 1` frame queuing delay) with bounded connect (`400 ms`) and read (`500 ms`) timeouts and automatic reconnection on server restarts.
- **SQLite, REST & CLI Registration (`crates/photon-common`, `crates/photon-web`, `src/lib.rs`)**:
  - Added `CameraType::TcpCamera` and `PVCameraInfo::Tcp` (`"PVCameraInfo.PVTcpCameraInfo"` / `"PVTcpCameraInfo"`) for lossless `photon.sqlite` persistence.
  - Added `POST /api/settings/camera/addTcpCamera` REST endpoint and repeatable `--tcp-camera <tcp://host:port>` CLI flag.
  - Added 4 end-to-end integration tests in `tests/tcp_camera_stream_parity.rs`.

---

## [2027.0.0-rust.1] - 2026-11-23

### Added — Core `photon-core` to Rust Migration (`001-fast-photon-core`)
- **Multi-Crate Cargo Workspace (`fast-photon-core/`)**:
  - Established 5 domain sub-crates (`photon-targeting`, `photon-common`, `photon-hal`, `photon-vision`, `photon-web`) plus the top-level library facade and standalone binary with workspace-wide `unsafe_code = "deny"` and `clippy::unwrap_used = "warn"`.
- **Wire & Protocol Parity (`crates/photon-targeting`)**:
  - Implemented little-endian `Packet` buffer and `PacketSerde` with exact MD5 interface UUIDs (`PhotonPipelineResult` = `fd353b11f3a818e9c5b457c7dc5a8f5c`, `PhotonTrackedTarget` = `cc6dbb5c5c1e0fa808108019b20863f1`, `PhotonPipelineMetadata` = `5231116d7f87ff560e6aa5263d0eee02`, `MultiTargetPNPResult` = `541096947e9f3ca2d3f425ff7b04aa7b`).
  - Implemented 3D geometry primitives (`Translation3d`, `Quaternion`, `Rotation3d`, `Transform3d` 56-byte WPILib `Struct`, `Pose3d`), `prost` Protobuf messages (`ProtobufPhotonPipelineResult`, `ProtobufDeviceMetrics`), and 3D `TargetModel` / `VisionEstimation` math.
- **SQLite Persistence, Migrations & Dataflow (`crates/photon-common`)**:
  - Implemented `ConfigProvider` and `MigrationManager` reproducing all SQLite `PRAGMA user_version` migrations (`2` → `202701` → `202702` → `202703` → `202704`) and `.zip` archive import/export.
  - Implemented `DataChangeService` pub/sub event bus, `NTTopicSet`, `NTDataPublisher`, `NetworkTablesManager`, UDP `TimeSyncClient`/`TimeSyncServer` (`5810`), and `UIDataPublisher`.
- **Hardware Abstraction (`crates/photon-hal`)**:
  - Implemented `Platform` detection, GPIO/PWM `HardwareManager` (`VisionLED`, `StatusLED`, `StatusLEDConsumer`), `PlatformSystemMonitor` (`DeviceMetrics`), and Linux `nmcli` `NetworkManager`.
- **Computer Vision Engine (`crates/photon-vision`)**:
  - Implemented RAII `CVMat` (with deterministic `Drop` tracking), `Contour`, `CVShape`, `Frame`, 35+ modular `CVPipe` stages with `PipelineProfiler`, and all 9 vision pipelines (`ReflectivePipeline`, `ColoredShapePipeline`, `AprilTagPipeline`, `ArucoPipeline`, `ObjectDetectionPipeline`, `Calibrate3dPipeline`, `DriverModePipeline`, `FocusPipeline`, `OutputStreamPipeline`).
  - Implemented `VisionSourceManager`, `VisionModuleManager`, `VisionModule`, `VisionRunner`, and `PipelineManager`.
- **Embedded HTTP REST, WebSocket & MJPEG Server (`crates/photon-web`)**:
  - Implemented Axum server on port `5800` serving binary MessagePack `/websocket_data` (all 16 `DataSocketMessageType` commands), all `/api/*` REST routes, and `multipart/x-mixed-replace` MJPEG streaming (`:1181+`).
