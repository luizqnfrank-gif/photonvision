# Feature Specification: Migrate `photon-core` to Rust (`fast-photon-core`)

**Feature Branch**: `001-fast-photon-core`  
**Created**: 2026-11-23  
**Status**: Draft  
**Input**: User description: "miragate the photon-core to rust which name of folder is fast-photon-core"

## Clarifications

### Session 2026-11-23

- Q: Integration Architecture with `photon-server` → A: Option B (Standalone Rust Engine: pure Rust library crate + standalone runtime binary communicating over NetworkTables/WebSockets/SQLite, without Java JNI bindings).
- Q: Vision & Robotics Stack Strategy → A: Option B (Hybrid Rust + OpenCV/Native FFI: system OpenCV/`opencv` crate and C/C++ FFI bindings for core vision pipes, calibration, and fiducial detection, combined with pure-Rust infrastructure for NT4, SQLite, and async runtime).
- Q: Cargo Workspace Layout inside `fast-photon-core/` → A: Option B (Multi-Crate Cargo Workspace under `fast-photon-core/crates/...` separating domain sub-crates such as `photon-targeting`, `photon-common`, `photon-vision`, and `photon-hal`, re-exported by a top-level `fast-photon-core` facade crate and binary).
- Q: Standalone Binary External Control & Web UI Contract → A: Option A (Direct WebSocket, REST & MJPEG Server in Binary: the standalone `fast-photon-core` binary hosts the MJPEG stream ports `1181+`, `/websocket_data`, and REST settings endpoints directly so `photon-client` and FRC dashboards connect out-of-the-box).
- Q: Camera Acquisition Backend Strategy (`USBFrameProvider` & `LibcameraGpuFrameProvider`) → A: Option A (Native Rust V4L2 + OpenCV/Nokhwa Fallback + Feature-Gated Libcamera: use direct Linux `v4l` with mmap buffers and V4L2 ioctl controls for camera quirks, OpenCV `VideoCapture`/`nokhwa` fallback on Windows, and feature-gated `libcamera` bindings for CSI cameras).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Drop-In Configuration & Data Compatibility (Priority: P1)

An FRC robotics team upgrades their coprocessor to a release powered by `fast-photon-core`. Their existing `photon.sqlite` database, camera calibrations, AprilTag field layouts, network configuration, hardware settings, and neural network model definitions load seamlessly without data loss, manual re-entry, or schema corruption, and can be exported/imported interchangeably with the legacy system.

**Why this priority**: Configuration persistence and data model parity are the foundational layer for all cameras, pipelines, and UI/NetworkTables communication. Without 100% database and JSON compatibility, users lose calibrated cameras and pipeline settings upon migration.

**Independent Test**: Can be fully tested by pointing `fast-photon-core` at existing `photon.sqlite` databases and configuration archives from `test-resources/`, loading all global and per-camera configurations, running schema migrations, saving modifications, and verifying byte- and field-level JSON/SQLite equivalence with the legacy system.

**Acceptance Scenarios**:

1. **Given** an existing `photon.sqlite` database containing global settings (`networkConfig`, `hardwareConfig`, `hardwareSettings`, `fieldLayout`, `neuralNetworkProperties`) and multiple camera rows (`config_json`, `drivermode_json`, `pipeline_jsons`), **When** `fast-photon-core` initializes its configuration provider, **Then** all configurations deserialize without error and match the original domain values.
2. **Given** an older database schema version (`user_version`), **When** `fast-photon-core` starts up, **Then** it executes all pending migrations sequentially and updates `user_version` identically to the reference migration manager.
3. **Given** a uploaded settings `.zip` archive exported from a legacy PhotonVision instance, **When** imported into `fast-photon-core`, **Then** the configuration directory is cleanly replaced and reloaded with all camera calibrations and pipelines intact.

---

### User Story 2 - Vision Pipeline & Target Localization Parity (Priority: P1)

A robot control system relies on PhotonVision to detect game elements and fiducial markers (AprilTags, ArUco markers, reflective tape, colored shapes, and neural network objects) and compute accurate 2D target metrics (yaw, pitch, area, skew, corners) and 3D poses (single-tag PnP, multi-tag field-to-camera pose, and geometric SolvePnP). When processed by `fast-photon-core`, identical input frames and camera calibrations produce identical target detections and 3D pose estimates within defined numerical tolerances, without memory leaks across continuous operation.

**Why this priority**: Accurate, deterministic vision processing and 3D pose estimation are the core value proposition of PhotonVision during FRC matches.

**Independent Test**: Can be tested headlessly using `FileFrameProvider` / static test images from `test-resources/` across all 8 pipeline types (`AprilTag`, `ArUco`, `Reflective`, `ColoredShape`, `ObjectDetection`, `Calibrate3d`, `DriverMode`, `Focus`) and comparing the resulting `CVPipelineResult` and `TrackedTarget` metrics against reference outputs.

**Acceptance Scenarios**:

1. **Given** calibrated camera intrinsics, an AprilTag field layout, and a test frame containing one or more 16h5/36h11 AprilTags or ArUco markers, **When** the `AprilTag` or `ArUco` pipeline executes, **Then** it outputs matching fiducial IDs, corner coordinates, yaw/pitch angles, single-tag `bestCameraToTarget3d`/`altCameraToTarget3d` transforms, pose ambiguity, and multi-tag pose estimates within `1e-4` meters / `1e-3` radians of the reference implementation.
2. **Given** a test frame and HSV/contour/shape settings for `Reflective` or `ColoredShape` pipelines, **When** the pipeline executes, **Then** it filters, groups, sorts contours/shapes, and computes 2D/3D target properties matching reference outputs.
3. **Given** an object detection model (`.rknn` or `.tflite` or mock/null detector) and an input frame, **When** the `ObjectDetection` pipeline runs letterboxing, inference, and confidence/area filtering, **Then** it returns `TrackedTarget` instances with accurate bounding corners, class IDs, confidence scores, and 2D yaw/pitch offsets.
4. **Given** a sequence of chessboard or ChArUco board frames, **When** the `Calibrate3d` pipeline collects observations and executes calibration (OpenCV or `mrcal` pinhole/fisheye models), **Then** it computes camera intrinsics, distortion coefficients, and reprojection errors within reference tolerances.
5. **Given** continuous frame processing over thousands of frames and frequent pipeline switches, **When** monitored for memory usage, **Then** native and heap memory remain bounded with zero leaks or use-after-free faults.

---

### User Story 3 - Camera Discovery, Quirks & Multi-Camera Lifecycle (Priority: P2)

A user connects multiple USB (V4L2/CSCore), CSI (Libcamera), or file-replay cameras to the coprocessor. `fast-photon-core` automatically discovers connected cameras, filters out blacklisted non-capture devices (e.g., `bcm2835-isp`), applies sensor-specific quirks (e.g., ArduCam OV9281/OV2311/OV9782, InnoMaker OV9281, LifeCam HD-3000, PS3 Eye, See3CAM_24CUG), runs each active camera in an isolated module, and allows the user to deactivate, reactivate, or reassign cameras without restarting the service.

**Why this priority**: Reliable hardware discovery and per-camera isolation ensure teams can run multi-camera setups on embedded coprocessors without one disconnected or misbehaving camera crashing the entire vision system.

**Independent Test**: Can be tested using mock/simulated camera sources and hardware descriptors (`PVCameraInfo`) to verify matching rules, quirk detection, camera control settables, deactivation/reactivation state transitions, and concurrent `VisionModule` execution.

**Acceptance Scenarios**:

1. **Given** a set of connected camera descriptors (`PVCameraInfo`) and saved `CameraConfiguration` entries, **When** the vision source manager initializes, **Then** active configurations are matched by unique path and started as isolated `VisionModule` workers while deactivated configurations remain stored without locking hardware ports.
2. **Given** a USB camera with known vendor/product IDs or V4L2 controls matching a `CameraQuirk` profile, **When** the camera source initializes, **Then** the corresponding exposure, gain, white balance, and resolution quirks are applied automatically.
3. **Given** a running `VisionModule`, **When** a user deactivates the camera and later reactivates it, **Then** the hardware device is cleanly released upon deactivation and re-acquired with its full pipeline state upon reactivation.

---

### User Story 4 - Real-Time NetworkTables (NT4), WebSocket & MJPEG Dataflow (Priority: P2)

During robot operation and dashboard tuning, `fast-photon-core` publishes pipeline results, target packets, latency, FPS, heartbeats, and synchronized timestamps to the robot over NetworkTables (NT4), broadcasts live UI updates and configuration mutations over the internal event bus (`DataChangeService`) to WebSocket subscribers, and streams raw input and overlay-annotated output frames over MJPEG ports.

**Why this priority**: Vision calculations are only useful if they reach the robot controller with minimal latency and can be monitored/tuned live by students and mentors via the dashboard.

**Independent Test**: Can be tested by connecting an NT4 client (`photon-lib` compatible) and a WebSocket subscriber to `fast-photon-core`, running a vision module on test frames, verifying published binary packets/topics and WebSocket events, and sending control mutations (pipeline index, driver mode, FPS limit) back from NT4/WebSocket to verify state updates.

**Acceptance Scenarios**:

1. **Given** an active `VisionModule` producing `CVPipelineResult` frames, **When** results are consumed by the NetworkTables publisher, **Then** the exact NT4 topics (`rawBytes` packet, protobuf/struct topics, `latencyMillis`, `hasTarget`, `targetPitch`, `targetYaw`, `targetArea`, `targetPose`, `heartbeat`) and time-sync subtables update immediately and are decodable by `photon-lib`.
2. **Given** an NT4 client or WebSocket event changing `pipelineIndex`, `driverMode`, or `fpsLimit`, **When** the event arrives, **Then** the target `VisionModule` switches pipelines or applies the setting within one frame cycle and confirms the updated state to all subscribers.
3. **Given** processed frames and 2D/3D overlay settings, **When** the output stream pipeline runs, **Then** it renders crosshairs, contours, fiducial axes, or calibration overlays on a decoupled streaming thread without delaying NetworkTables result publication.

---

### User Story 5 - Coprocessor Hardware, LEDs, Metrics & OS Networking (Priority: P3)

On supported coprocessor platforms (Raspberry Pi, Orange Pi 5 RK3588, Rubik Pi 3 QCS6490, and desktop Linux/Windows), `fast-photon-core` controls GPIO vision illumination LEDs and status LEDs, collects system health metrics (CPU temperature/utilization, RAM, disk, NPU usage, thermal throttling), and manages static/DHCP network settings via OS networking tools (`nmcli`), gracefully degrading on unsupported or simulated desktop environments.

**Why this priority**: Hardware telemetry, LED control, and IP configuration complete the coprocessor appliance experience for FRC teams in the pit and on the field.

**Independent Test**: Can be tested by running platform detection, metrics collection, LED state transitions, and command generation against mocked OS/GPIO interfaces and verifying generated `DeviceMetrics` payloads and `nmcli` command sequences.

**Acceptance Scenarios**:

1. **Given** a configured vision LED or status LED pin definition in `HardwareConfig`, **When** pipeline settings or target lock states change, **Then** the hardware manager updates PWM brightness or RGB/Green-Yellow status indicators accordingly (or logs a no-op in simulated environments).
2. **Given** a periodic metrics task on any supported OS/SoC, **When** the system monitor runs, **Then** it populates `DeviceMetrics` (and its Protobuf representation) with accurate or gracefully defaulted system statistics without panicking on missing sysfs files.
3. **Given** a network configuration change (DHCP vs. Static IP, hostname, team number), **When** applied by the network manager, **Then** the appropriate OS network commands are constructed and executed with bounded timeouts.

---

### Edge Cases

- **Camera Disconnection & Reconnection**: When a physical USB or CSI camera disconnects mid-frame, the frame provider and `VisionRunner` MUST catch the read error, avoid panicking, report the disconnected status to the UI/NT4, and automatically resume frame capture when the device reconnects.
- **Corrupted or Unreadable Configuration / Model Files**: If `photon.sqlite`, a JSON blob, or a neural network model file is corrupted or missing, `fast-photon-core` MUST log a structured error, back up the broken file, and fall back to safe default settings or `NullModel` without crashing the process.
- **Degenerate Geometry & High-Noise Frames**: Frames with zero contours, thousands of noise speckles, collinear PnP corners, or uncalibrated cameras attempting 3D pose estimation MUST return empty or 2D-only results cleanly without division-by-zero, matrix inversion panics, or unbounded processing latency.
- **Concurrent Settings Mutation**: Simultaneous updates to pipeline settings from WebSocket UI and pipeline index changes from NetworkTables MUST be serialized safely via channels/locks without data races or inconsistent intermediate states.
- **Unsupported Desktop Hardware**: Running `fast-photon-core` on a developer laptop without Rockchip NPU (`rknn`), `libcamera`, `nmcli`, or GPIO pins MUST compile and run cleanly using feature/platform fallbacks.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (Workspace & Structural Parity)**: The system MUST be implemented as a multi-crate Cargo workspace in `fast-photon-core/` (`fast-photon-core/crates/...` for domain sub-crates such as `photon-targeting`, `photon-common`, `photon-vision`, `photon-hal`, plus a top-level `fast-photon-core` facade library crate and standalone runtime binary, without Java JNI bindings), preserving the legacy `photon-core/` directory intact and mapping all core domains (`configuration`, `dataflow`, `hardware`, `logging`, `networking`, `util`, and `vision::{apriltag, aruco, calibration, camera, frame, objects, opencv, pipe, pipeline, processes, target}`).
- **FR-002 (SQLite & Schema Migration Parity)**: `fast-photon-core` MUST read, write, and migrate `photon.sqlite` databases using the exact table schemas (`global`, `cameras`), `user_version` pragma migrations, and JSON serialization format used by `photon-core`, as well as support zip archive export and upload import.
- **FR-003 (Camera Sources & Quirk Profiles)**: `fast-photon-core` MUST support USB cameras (using direct Linux `v4l` with mmap buffers and V4L2 ioctl controls on Linux, and OpenCV `VideoCapture`/`nokhwa` fallback on Windows), feature-gated CSI Libcamera GPU sources (`libcamera`), and file/test frame sources (`PVCameraInfo`), apply all hardware quirk profiles (`CameraQuirk`, `QuirkyCamera`) for supported camera sensors across Linux and Windows, and expose full camera settables (`VisionSourceSettables`).
- **FR-004 (Modular Vision Pipes & Profiling)**: `fast-photon-core` MUST implement a composable, typed pipe architecture (`CVPipe`) with per-stage nanosecond execution profiling (`PipelineProfiler`) covering all 37 image-processing stages (image rotation/resize/grayscale/HSV thresholding, contour finding/filtering/grouping/sorting/speckle-rejection, polygon/circle detection, corner detection, AprilTag detection & pose estimation, ArUco detection & pose estimation, single- and multi-target SolvePnP, neural network object detection & filtering, 3D board corner finding & calibration, focus calculation, FPS calculation, and 2D/3D overlay drawing).
- **FR-005 (Complete Pipeline Suite)**: `fast-photon-core` MUST implement all 9 vision pipelines (`AprilTagPipeline`, `ArucoPipeline`, `ReflectivePipeline`, `ColoredShapePipeline`, `ObjectDetectionPipeline`, `Calibrate3dPipeline`, `DriverModePipeline`, `FocusPipeline`, and `OutputStreamPipeline`) with polymorphic JSON settings serialization.
- **FR-006 (Neural Network Inference Backends)**: `fast-photon-core` MUST provide a pluggable `Model` / `ObjectDetector` trait architecture supporting Rockchip RKNN (`RknnModel`), TensorFlow Lite (`TFLiteModel`), and a safe fallback (`NullModel`), complete with image letterboxing (`Letterbox`) and model file management (`NeuralNetworkModelManager`).
- **FR-007 (3D Calibration Engine)**: `fast-photon-core` MUST support chessboard and ChArUco camera calibration workflows, storing `BoardObservation` datasets and computing `CameraCalibrationCoefficients` using OpenCV and `mrcal` lens models.
- **FR-008 (Multi-Camera Runtime & Thread Isolation)**: `fast-photon-core` MUST orchestrate multiple concurrent cameras via `VisionSourceManager`, `VisionModuleManager`, `VisionModule`, `VisionRunner`, and `PipelineManager`, isolating frame capture/pipeline execution from output stream rendering, MJPEG streaming (`MJPGFrameConsumer`), and snapshot saving (`FileSaveFrameConsumer`).
- **FR-009 (Event Bus, NetworkTables & Embedded Web/MJPEG Server)**: `fast-photon-core` MUST provide a thread-safe pub/sub event bus (`DataChangeService`), publish/subscribe to all NetworkTables v4 topics (`NTDataPublisher`, `TimeSyncManager`, `NTDriverStation`), and host the MJPEG stream ports (`1181+`), `/websocket_data` endpoint (`UIDataPublisher` / `IncomingWebSocketEvent`), and REST settings endpoints directly in the standalone binary with 100% wire and binary payload compatibility with `photon-targeting`, `photon-lib`, and `photon-client`.
- **FR-010 (Hardware, LEDs, System Metrics & OS Networking)**: `fast-photon-core` MUST implement hardware abstraction for GPIO vision/status LEDs (`HardwareManager`, `VisionLED`, `StatusLED`), platform-specific health telemetry (`SystemMonitor` for Raspberry Pi, RK3588, QCS6490, Windows, and generic Linux serialized via `DeviceMetricsProto`), and OS network configuration (`NetworkManager`).
- **FR-011 (Memory Safety & Panic-Free Runtime)**: All image matrices and native resources MUST be managed automatically via Rust RAII (`Drop`) and bounded buffer pools, with zero `unsafe` blocks in pipeline/application logic and zero `unwrap()`/`expect()` panics in runtime code paths.

### Assumptions

- **Integration Boundary**: `fast-photon-core` is built strictly as a multi-crate Rust library and standalone runtime binary (no Java JNI bridge) that directly serves `photon-client` and FRC dashboards via embedded HTTP REST, WebSocket (`/websocket_data`, MessagePack/JSON), MJPEG stream ports (`1181+`), NetworkTables (NT4), and SQLite (`photon.sqlite`).
- **Native C/C++ Dependencies & Hybrid Stack**: Core computer vision pipes, 3D calibration, and fiducial detection bind to OpenCV (`opencv` crate) and native C/C++ libraries (`apriltag`, `mrcal`, plus feature-gated `rknn`, `tflite`, `libcamera`), while infrastructure subsystems (NetworkTables v4, SQLite persistence, event bus, MJPEG streaming, and async runtime) use native Rust crates.
- **Reference Test Assets**: The repository's existing `test-resources/` directory serves as the canonical fixture dataset for validating SQLite migrations, camera calibration, AprilTag/ArUco detection, and pipeline parity.

### Key Entities

- **`PhotonConfiguration` & `CameraConfiguration`**: Root and per-camera configuration models persisted in `photon.sqlite`, containing hardware/network configs, field layouts, neural network settings, camera quirks, calibrations, and polymorphic `CVPipelineSettings`.
- **`PVCameraInfo` & `QuirkyCamera`**: Hardware identification and capability/quirk descriptors for USB, CSI, and file-based vision sources.
- **`Frame` & `CVMat`**: RAII-managed image matrix containers holding raw color frames, processed/thresholded mats, capture timestamps, and `FrameStaticProperties` (resolution, FOV, calibration).
- **`CVPipe` & `CVPipeline`**: Modular processing stages and composite vision pipelines transforming a `Frame` and `CVPipelineSettings` into a `CVPipelineResult`.
- **`CVPipelineResult` & `TrackedTarget`**: Output data structures encapsulating detected 2D/3D target metrics (contours, corners, yaw, pitch, area, skew, fiducial ID, class ID/confidence, `Transform3d` poses, ambiguity, multi-tag pose) and latency/FPS telemetry.
- **`VisionSourceManager`, `VisionModule`, `VisionRunner` & `PipelineManager`**: Runtime lifecycle controllers managing camera discovery, active/deactivated states, worker threads, and pipeline switching.
- **`DataChangeEvent` & `DeviceMetrics`**: Event bus messages and coprocessor health telemetry structures synchronized across NetworkTables and WebSocket UI clients.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001 (Functional & Numerical Parity)**: 100% of migrated unit and integration test scenarios from `photon-core/src/test` pass in `fast-photon-core`, with 2D target angles (yaw/pitch/area) matching within `0.05` degrees/percent and 3D pose translations/rotations matching within `1e-4` meters and `1e-3` radians on `test-resources/` fixtures.
- **SC-002 (Protocol & Database Interoperability)**: 100% of existing `photon.sqlite` test databases and settings `.zip` archives load, migrate, and round-trip without error, and `photon-lib` clients decode 100% of published NetworkTables v4 target packets and time-sync messages without modification.
- **SC-003 (Memory Stability & Leak Elimination)**: Continuous execution of any vision pipeline (including frequent pipeline switching and resolution changes) over 10,000+ frames exhibits zero memory leaks (steady-state RSS growth < 1%) and zero use-after-free crashes.
- **SC-004 (Processing Throughput & Latency)**: Average per-frame pipeline execution latency in `fast-photon-core` is equal to or faster than the Java `photon-core` baseline (targeting >= 15% reduction in p99 frame processing jitter due to elimination of JVM garbage collection pauses).
- **SC-005 (Code Quality & Safety Compliance)**: `fast-photon-core` passes formatting and strict linting gates with zero warnings, zero `unsafe` blocks outside designated FFI modules, and zero unrecovered panics under malformed input or hardware disconnect tests.
