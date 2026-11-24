# Implementation Plan: Migrate `photon-core` to Rust (`fast-photon-core`)

**Branch**: `001-fast-photon-core` | **Date**: 2026-11-23 | **Spec**: [`specs/001-fast-photon-core/spec.md`](file:///home/rose/Documents/workspace_local/photonvision/specs/001-fast-photon-core/spec.md)
**Input**: Feature specification from `/specs/001-fast-photon-core/spec.md`

## Summary

Migrate PhotonVision's `photon-core` (and its required `photon-targeting` wire types and `photon-server` embedded endpoints) into a self-contained, multi-crate Rust workspace at `fast-photon-core/`. The system is designed as a pure-Rust library facade and standalone runtime binary (with zero Java JNI coupling) that combines OpenCV (`opencv` crate) and feature-gated native C/C++ vision accelerators (`apriltag`, `mrcal`, `rknn`, `tflite`, `libcamera`, Linux `v4l`) with a pure-Rust infrastructure stack (`tokio`, `axum`, `rusqlite`, `rmp-serde`, `prost`, `nalgebra`, and native NT4 + UDP TimeSync). It preserves 100% external protocol compatibility with `photon.sqlite`, `photon-lib` (NetworkTables v4 packets/structs/protobufs), and `photon-client` (MessagePack `/websocket_data`, REST API, and MJPEG stream ports `1181+`).

## Technical Context

**Language/Version**: Rust (Stable, Edition 2021 / `rustc >= 1.80`)  
**Primary Dependencies**:
- **Vision & Math**: `opencv` (OpenCV 4.x bindings), `nalgebra` (3D poses/transforms/quaternions), `v4l` (Linux V4L2 mmap & ioctl camera controls), feature-gated FFI for `apriltag`, `mrcal`, `rknn`, `tflite`, `libcamera`
- **Serialization & Persistence**: `rusqlite` (bundled SQLite 3), `serde`, `serde_json`, `rmp-serde` (MessagePack for WebSocket & NT4), `prost` (Protobuf for `photon.proto` & `geometry3d.proto`), `zip`
- **Async Runtime, Networking & Web**: `tokio`, `axum`, `tower-http` (HTTP REST, `/websocket_data`, MJPEG multipart streaming), `tokio-tungstenite` (NT4 WebSocket client/server), `crossbeam-channel`
- **Hardware, Metrics & Logging**: `sysinfo` (CPU/RAM/disk metrics), `tracing` & `tracing-subscriber`
**Storage**: SQLite (`photon.sqlite` with `PRAGMA user_version` migrations `2` → `202701`..`202704`) + filesystem archives/models  
**Testing**: `cargo test` (unit, wire-contract, and integration parity suites against `../test-resources/`) + `criterion` benchmarks  
**Target Platform**: `linux-arm64` (Raspberry Pi 4/5, Orange Pi 5 RK3588, Rubik Pi 3 QCS6490), `linux-x86_64`, `windows-x86_64`  
**Project Type**: Multi-crate Cargo workspace (`rlib` sub-crates + top-level library facade and standalone binary)  
**Performance Goals**: Equal or faster average frame latency than Java `photon-core`; >= 15% reduction in p99 frame processing jitter; zero GC pause stalls  
**Constraints**: < 1% steady-state RSS memory drift over 10,000+ frames; numerical parity within `0.05` deg (2D) and `1e-4` m / `1e-3` rad (3D); zero `unsafe` in application/pipeline logic; zero runtime `unwrap()`/`expect()` panics  
**Scale/Scope**: ~225 Java classes migrated across 5 workspace sub-crates + 1 top-level crate (37 vision pipes, 9 pipelines, 36 REST endpoints, 16 WebSocket command types, 23 per-camera NT4 topics, 5 SQLite migration steps)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- [x] **I. Workspace Isolation & Parity**: All Rust code is isolated under `fast-photon-core/` with explicit module mapping to `photon-core/` Java classes; existing Java build remains untouched.
- [x] **II. Memory Safety & Deterministic Resource Lifecycle**: RAII (`Drop`) used for all matrices/frames (`CVMat`); zero `unsafe` in application/pipeline logic (FFI isolated in thin audited wrappers); bounded buffer pools on hot paths.
- [x] **III. Wire, Schema & Protocol Compatibility**: 100% compatibility preserved for `photon.sqlite` schema/JSON (`user_version` 2..202704), NT4 topics & packet/struct/protobuf payloads, and WebSocket MessagePack UI DTOs.
- [x] **IV. High-Throughput Concurrency & Zero-Copy Hot Paths**: Per-camera `VisionRunner` worker threads decoupled from MJPEG/WebSocket/disk I/O (`StreamRunnable` + `tokio::sync::watch`); bounded channels for `DataChangeService`; V4L2 `mmap` capture.
- [x] **V. Test-Driven Parity & Benchmarking Discipline**: Automated `cargo test` parity checks against Java baseline and `test-resources/` within floating-point tolerances; zero panics (`unwrap`/`expect`) in runtime paths.

## Project Structure

### Documentation (this feature)

```text
specs/001-fast-photon-core/
├── plan.md                                      # This file (/04-speckit.plan output)
├── research.md                                  # Phase 0 architectural & stack decisions
├── data-model.md                                # Phase 1 entity schemas & state transitions
├── quickstart.md                                # Phase 1 build, test & runtime verification guide
├── contracts/                                   # Phase 1 external wire & API contracts
│   ├── rest-and-websocket-api.yaml              # OpenAPI 3.0 spec for port 5800 REST & WebSocket
│   └── networktables-and-wire-protocol.md       # NT4 topics, TimeSync UDP, Packet & Struct layouts
└── tasks.md                                     # Phase 2 output (/05-speckit.tasks command)
```

### Source Code (repository root)

```text
fast-photon-core/
├── Cargo.toml                                   # Workspace manifest & shared dependency versions
├── src/
│   ├── lib.rs                                   # Top-level facade re-exporting all sub-crates
│   └── main.rs                                  # Standalone fast-photon-core CLI & daemon entrypoint
├── crates/
│   ├── photon-targeting/                        # Wire formats, geometry3d & target math (pure Rust)
│   │   ├── Cargo.toml
│   │   ├── build.rs                             # Prost protobuf compilation (photon.proto, geometry3d.proto)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── packet.rs                        # Little-endian Packet buffer & PacketSerde trait
│   │       ├── geometry.rs                      # Transform3d, Pose3d, Translation3d, Rotation3d, WPILib Struct
│   │       ├── targets.rs                       # PhotonTrackedTarget, TargetCorner, PnpResult, MultiTargetPNPResult
│   │       ├── result.rs                        # PhotonPipelineMetadata, PhotonPipelineResult
│   │       ├── estimation.rs                    # TargetModel, CameraTargetRelation, VisionEstimation
│   │       └── proto.rs                         # Generated Protobuf bindings & conversions
│   ├── photon-common/                           # Configuration, SQLite migrations, Event Bus & NT4
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── configuration/
│   │       │   ├── mod.rs                       # PhotonConfiguration, ConfigProvider, PathManager
│   │       │   ├── camera.rs                    # CameraConfiguration, PVCameraInfo, PipelineSettingsVariant
│   │       │   ├── hardware.rs                  # HardwareConfig, HardwareSettings, StatusLedConfig
│   │       │   ├── network.rs                   # NetworkConfig, NetworkMode
│   │       │   ├── neural_network.rs            # NeuralNetworkModelManager, NeuralNetworkModelsSettings
│   │       │   ├── sql.rs                       # SQLite read/write & zip archive import/export
│   │       │   └── migrations.rs                # MigrationManager & DbMigration (2 -> 202701..202704)
│   │       ├── dataflow/
│   │       │   ├── mod.rs                       # DataChangeService, DataChangeEvent, CVPipelineResultConsumer
│   │       │   ├── networktables/
│   │       │   │   ├── mod.rs                   # NetworkTablesManager, NTTopicSet, NTDataPublisher
│   │       │   │   ├── timesync.rs              # TimeSyncManager (UDP 5810 client/server)
│   │       │   │   └── driverstation.rs         # NTDriverStation
│   │       │   └── websocket.rs                 # UIDataPublisher, UIPhotonConfiguration, UICameraConfiguration
│   │       ├── logging.rs                       # Structured Logger, LogGroup, LogLevel, KernelLogLogger
│   │       └── util.rs                          # TimedTaskManager, MathUtils, ColorHelper, NumberCouple
│   ├── photon-hal/                              # Hardware abstraction, GPIO LEDs, Metrics & OS Networking
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── platform.rs                      # Platform, OSType, PiVersion, OsImageData
│   │       ├── hardware_manager.rs              # HardwareManager, VisionLED, StatusLED (RGB & GreenYellow)
│   │       ├── gpio.rs                          # CustomAdapter & GPIO/PWM traits + mock fallback
│   │       ├── metrics.rs                       # DeviceMetrics & SystemMonitor (Pi, RK3588, QCS6490, Windows)
│   │       └── networking.rs                    # NetworkManager, NetworkingCommands (nmcli), NetworkUtils
│   ├── photon-vision/                           # OpenCV pipes, 9 pipelines, cameras, calibration & processes
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── opencv_ext.rs                    # CVMat (RAII + pool), Contour, CVShape, ImageRotationMode, ImageFlipMode
│   │       ├── frame/
│   │       │   ├── mod.rs                       # Frame, FrameProvider, FrameStaticProperties, FrameDivisor
│   │       │   ├── provider.rs                  # USBFrameProvider (v4l/OpenCV), LibcameraGpuFrameProvider, FileFrameProvider
│   │       │   └── consumer.rs                  # MJPGFrameConsumer, FileSaveFrameConsumer
│   │       ├── camera/
│   │       │   ├── mod.rs                       # CameraType, CameraQuirk, QuirkyCamera, FileVisionSource, TestSource
│   │       │   ├── usb.rs                       # USBCameraSource & sensor-specific Settables (OV9281, OV2311, OV9782, LifeCam, etc.)
│   │       │   └── csi.rs                       # LibcameraGpuSource & LibcameraGpuSettables
│   │       ├── apriltag_aruco.rs                # AprilTagFamily, PhotonArucoDetector, ArucoDetectionResult
│   │       ├── calibration.rs                   # CameraCalibrationCoefficients, BoardObservation, CameraLensModel
│   │       ├── objects.rs                       # Model, ObjectDetector, Letterbox, RknnObjectDetector, TFLiteObjectDetector, NullModel
│   │       ├── target.rs                        # TrackedTarget, PotentialTarget, TargetCalculations, TargetModel
│   │       ├── pipe/
│   │       │   ├── mod.rs                       # CVPipe & MutatingPipe traits
│   │       │   └── impls/                       # All 37 image processing pipes (HSV, contours, PnP, AprilTag, ArUco, etc.)
│   │       ├── pipeline/
│   │       │   ├── mod.rs                       # CVPipeline trait, PipelineType, PipelineProfiler, CVPipelineResult
│   │       │   ├── apriltag.rs                  # AprilTagPipeline & AprilTagPipelineSettings
│   │       │   ├── aruco.rs                     # ArucoPipeline & ArucoPipelineSettings
│   │       │   ├── reflective.rs                # ReflectivePipeline & ReflectivePipelineSettings
│   │       │   ├── colored_shape.rs             # ColoredShapePipeline & ColoredShapePipelineSettings
│   │       │   ├── object_detection.rs          # ObjectDetectionPipeline & ObjectDetectionPipelineSettings
│   │       │   ├── calibrate3d.rs               # Calibrate3dPipeline & Calibration3dPipelineSettings
│   │       │   ├── driver_mode.rs               # DriverModePipeline & DriverModePipelineSettings
│   │       │   ├── focus.rs                     # FocusPipeline & FocusPipelineSettings
│   │       │   └── output_stream.rs             # OutputStreamPipeline
│   │       └── processes/
│   │           ├── mod.rs                       # VisionSource, VisionSourceSettables
│   │           ├── pipeline_manager.rs          # PipelineManager
│   │           ├── vision_runner.rs             # VisionRunner
│   │           ├── vision_module.rs             # VisionModule & VisionModuleChangeSubscriber
│   │           └── vision_source_manager.rs     # VisionSourceManager & VisionModuleManager
│   └── photon-web/                              # Embedded HTTP REST, MessagePack WebSocket & MJPEG Server
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── server.rs                        # Axum router (port 5800) & static web asset serving
│           ├── websocket.rs                     # /websocket_data MessagePack handler (DataSocketHandler parity)
│           ├── handlers.rs                      # All 36 /api/* REST handlers (RequestHandler & TestRequestHandler parity)
│           └── mjpeg.rs                         # Per-camera MJPEG multipart HTTP server (ports 1181+)
└── tests/
    ├── sqlite_migration_parity.rs               # Verifies DbMigration steps & round-trip against test-resources/
    ├── packet_and_proto_parity.rs               # Verifies byte-level Packet, Struct, and Protobuf compatibility
    ├── pipeline_numerical_parity.rs             # Runs all 9 pipelines on test-resources/ images & checks pose tolerances
    └── websocket_and_rest_contract.rs           # Verifies MessagePack WebSocket & REST endpoints
```

**Structure Decision**: Selected a 5-sub-crate Cargo workspace under `fast-photon-core/crates/` plus a top-level `fast-photon-core` library/binary crate (per Clarification Q1, Q3, Q4). This cleanly separates pure-Rust wire/config/web crates from OpenCV/hardware FFI crates while keeping 100% of new files inside `fast-photon-core/`.

## Complexity Tracking

No Constitution Check violations exist. All 5 principles pass without exception.
