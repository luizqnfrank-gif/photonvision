# photon-core - Specification (Migrated)

> This specification was auto-generated from existing code in `photon-core/`.
> Review and refine before using for future development.

## Overview

`photon-core` is the central computer vision, camera management, configuration persistence, hardware control, and telemetry/dataflow engine of PhotonVision. It manages physical and simulated camera sources (USB V4L2/CSCore, CSI Libcamera GPU, and file replay sources), executes modular frame-processing pipelines (`CVPipeline` composed of `CVPipe` stages), performs 2D and 3D target localization (AprilTag, ArUco, reflective tape, colored shapes, and RKNN/TFLite neural network object detection), calibrates camera intrinsics via OpenCV and `mrcal`, and publishes results to FRC robot controllers over WPILib NetworkTables (NT4) and to the web dashboard via WebSockets and MJPEG streams.

## Functional Requirements

### 1. Camera Discovery, Lifecycle & Quirk Management
- **FR-CAM-01 (Discovery & Matching)**: Discover connected USB (`UsbCamera` via CSCore) and CSI (`LibCameraJNI`) cameras, blacklist non-capture devices (e.g., `bcm2835-isp`), and match devices by unique hardware paths and `PVCameraInfo`.
- **FR-CAM-02 (Source Lifecycle)**: Manage active and deactivated camera configurations via `VisionSourceManager` and `VisionModuleManager`, supporting dynamic assignment, deactivation (releasing hardware locks while retaining configuration), and reactivation.
- **FR-CAM-03 (Hardware Quirks)**: Detect and apply camera-specific quirks (`CameraQuirk`, `QuirkyCamera`) for specific USB sensors (ArduCam OV2311/OV9281/OV9782, InnoMaker OV9281, ThriftyCam OV9281, Microsoft LifeCam HD-3000, PS3 Eye, See3CAM_24CUG) across Linux and Windows platforms.
- **FR-CAM-04 (Camera Settables & Frame Providers)**: Expose camera controls (exposure, brightness, gain, white balance, video modes, FOV) via `VisionSourceSettables` and deliver frames via `FrameProvider` implementations (`USBFrameProvider`, `LibcameraGpuFrameProvider`, `FileFrameProvider`).

### 2. Vision Processing Pipelines & Modular Pipes
- **FR-PIPE-01 (Pipeline Execution Engine)**: Run per-camera asynchronous processing loops in `VisionRunner`, managed by `PipelineManager` and orchestrated by `VisionModule`.
- **FR-PIPE-02 (Supported Pipeline Types)**:
  - **AprilTag (`AprilTagPipeline`)**: Detect 16h5 and 36h11 AprilTags, estimate single-tag 3D pose (`AprilTagPoseEstimatorPipe`), and compute multi-tag robot/camera field pose (`MultiTargetPNPPipe`).
  - **ArUco (`ArucoPipeline`)**: Detect ArUco fiducial markers using `PhotonArucoDetector`, estimate 3D pose (`ArucoPoseEstimatorPipe`), and support multi-tag PnP.
  - **Reflective (`ReflectivePipeline`)**: HSV thresholding, contour detection, speckle rejection, contour grouping/filtering/sorting, and optional 3D `SolvePNPPipe`.
  - **Colored Shape (`ColoredShapePipeline`)**: HSV thresholding, contour detection, polygon/circle shape fitting (`FindPolygonPipe`, `FindCirclesPipe`), shape filtering, and 3D PnP.
  - **Object Detection (`ObjectDetectionPipeline`)**: Run hardware-accelerated object detection via Rockchip RKNN (`RknnObjectDetector` / `RknnModel`) or TensorFlow Lite (`TFLiteObjectDetector` / `TFLiteModel`) with image letterboxing (`Letterbox`) and confidence/area filtering (`FilterObjectDetectionsPipe`).
  - **3D Calibration (`Calibrate3dPipeline`)**: Detect chessboard/ChArUco board corners (`FindBoardCornersPipe`), collect board observations, and calibrate camera intrinsics/distortion using OpenCV or `mrcal` (`Calibrate3dPipe`) with pinhole (`CameraLensModel`) models.
  - **Driver Mode & Focus (`DriverModePipeline`, `FocusPipeline`)**: Provide low-latency unthresholded streaming with crosshairs and Laplacian variance focus scoring (`FocusPipe`).
- **FR-PIPE-03 (Native Memory Management)**: Enforce deterministic native memory cleanup for OpenCV `Mat` and JNI resources via the `Releasable` interface and `CVMat` reference tracking.
- **FR-PIPE-04 (Output Stream & Frame Saving)**: Render 2D/3D overlays (`OutputStreamPipeline`) onto processed frames, serve input/output MJPEG streams (`MJPGFrameConsumer`), and capture single-frame snapshots to disk (`FileSaveFrameConsumer`).

### 3. Configuration & SQLite Persistence
- **FR-CFG-01 (SQLite Storage & Migrations)**: Persist global settings (`NetworkConfig`, `HardwareConfig`, `HardwareSettings`, AprilTag `Field` layout, `NeuralNetworkModelsSettings`) and per-camera configurations (`CameraConfiguration`, pipeline JSONs, driver mode settings) in `photon.sqlite` via `ConfigProvider`, with schema evolution handled by `MigrationManager` and `DbMigration`.
- **FR-CFG-02 (Import / Export)**: Support full settings archive export and zip upload restoration (`ConfigProvider.saveUploadedSettingsZip`).
- **FR-CFG-03 (Neural Network Model Management)**: Discover, extract, upload, delete, and manage `.rknn` and `.tflite` weights and labels via `NeuralNetworkModelManager`.

### 4. Dataflow, Telemetry & Networking
- **FR-DATA-01 (Event Bus)**: Route internal state changes asynchronously through `DataChangeService` using typed `DataChangeEvent`s (`IncomingWebSocketEvent`, `OutgoingUIEvent`) filtered by `DataChangeSource` and `DataChangeDestination`.
- **FR-DATA-02 (NetworkTables NT4 Integration)**: Publish `PhotonPipelineResult` packets (serialized via Packet/Protobuf/Struct in `photon-targeting`), latency, FPS, heartbeats, and multi-tag results via `NTDataPublisher` and `NetworkTablesManager`; synchronize coprocessor-to-RIO clocks via `TimeSyncManager`; listen to robot control topics (pipeline index, driver mode, FPS limit, LED mode).
- **FR-DATA-03 (WebSocket UI Sync)**: Publish real-time pipeline results, calibration data, and configuration updates to the web frontend via `UIDataPublisher`.
- **FR-DATA-04 (OS Networking)**: Configure static/DHCP IP addresses, hostname, and NetworkManager (`nmcli`) device profiles on Linux coprocessors via `NetworkManager` and `NetworkingCommands`.

### 5. Hardware & Platform Abstraction
- **FR-HW-01 (GPIO & Lighting Control)**: Control vision illumination LEDs (`VisionLED`) and status LEDs (`StatusLED`, `RGBStatusLED`, `GreenYellowStatusLED`) via `HardwareManager` and `diozero` (`CustomAdapter`, `CustomDeviceFactory`).
- **FR-HW-02 (System Metrics Monitoring)**: Collect CPU usage, memory, disk usage, NPU usage, and thermal throttling metrics across Raspberry Pi, RK3588 (Orange Pi 5), QCS6490 (Rubik Pi 3), and Windows via `SystemMonitor` subclasses and publish them via Protobuf (`DeviceMetricsProto`).

## Key Entities

| Entity | Location | Description |
| :--- | :--- | :--- |
| [`PhotonConfiguration`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/common/configuration/PhotonConfiguration.java) | `common.configuration` | Root configuration aggregate containing `HardwareConfig`, `HardwareSettings`, `NetworkConfig`, `AprilTagFieldLayout`, `NeuralNetworkModelsSettings`, and `CameraConfiguration` map. |
| [`CameraConfiguration`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/common/configuration/CameraConfiguration.java) | `common.configuration` | Per-camera configuration holding `uniqueName`, `matchedCameraInfo`, `nickname`, `cameraQuirks`, `calibrations`, `pipelineSettings`, and `driveModeSettings`. |
| [`PVCameraInfo`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/camera/PVCameraInfo.java) | `vision.camera` | Hardware descriptor record/class for USB, CSI (`LibcameraGpuSource`), or file cameras (`CameraType`, path, name, vendor/product ID, other paths). |
| [`VisionModule`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/processes/VisionModule.java) | `vision.processes` | Runtime coordinator binding a `VisionSource`, `PipelineManager`, `VisionRunner`, MJPEG streamers, frame savers, and result consumers (`NTDataPublisher`, `UIDataPublisher`, `StatusLEDConsumer`). |
| [`CVPipeline`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/pipeline/CVPipeline.java) & [`CVPipelineSettings`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/pipeline/CVPipelineSettings.java) | `vision.pipeline` | Abstract base class for vision pipelines and polymorphic JSON-serializable settings (`AprilTagPipelineSettings`, `ReflectivePipelineSettings`, `ObjectDetectionPipelineSettings`, etc.). |
| [`CVPipe`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/pipe/CVPipe.java) | `vision.pipe` | Unit processing stage taking input `(I)`, parameters `(P)`, and producing output `(O)` with nanosecond profiling support. |
| [`Frame`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/frame/Frame.java) & [`CVMat`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/opencv/CVMat.java) | `vision.frame` / `vision.opencv` | Wrapper around raw color and processed/thresholded OpenCV `Mat` instances, capture timestamps, and `FrameStaticProperties`. |
| [`CVPipelineResult`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/pipeline/result/CVPipelineResult.java) | `vision.pipeline.result` | Output of a pipeline run containing `List<TrackedTarget>`, processing latency, FPS, multi-tag result, and output `Frame`. |
| [`TrackedTarget`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/target/TrackedTarget.java) | `vision.target` | Internal target representation holding 2D contour/corners, yaw/pitch/area/skew, 3D `Transform3d` poses (`bestCameraToTarget3d`, `altCameraToTarget3d`), fiducial ID, ambiguity, or neural network class ID and confidence. |
| [`CameraCalibrationCoefficients`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/vision/calibration/CameraCalibrationCoefficients.java) | `vision.calibration` | Intrinsics matrix, distortion coefficients, `BoardObservation` list, resolution, and `CameraLensModel` for a specific camera resolution. |
| [`DeviceMetrics`](file:///home/rose/Documents/workspace_local/photonvision/photon-core/src/main/java/org/photonvision/common/hardware/metrics/DeviceMetrics.java) | `common.hardware.metrics` | Snapshot of coprocessor hardware health (CPU temp/utilization, RAM, disk, NPU usage, throttling). |
