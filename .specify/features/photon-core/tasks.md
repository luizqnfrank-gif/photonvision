# photon-core - Tasks (Migrated)

All tasks marked `[x]` represent existing implemented functionality in `photon-core`.
Tasks marked `[ ]` are inferred gaps or explicit `TODO` items found in the codebase.

## Existing Implementation

### Configuration & Persistence
- [x] SQLite configuration store & JSON serialization (`ConfigProvider`, `PhotonConfiguration`, `CameraConfiguration`) - Implemented in `photon-core/src/main/java/org/photonvision/common/configuration/`
- [x] Database schema migration engine (`MigrationManager`, `DbMigration`, `SQLUtils`) - Implemented in `photon-core/src/main/java/org/photonvision/common/configuration/migrations/`
- [x] Neural network model discovery & management (`NeuralNetworkModelManager`, `NeuralNetworkModelsSettings`) - Implemented in `photon-core/src/main/java/org/photonvision/common/configuration/`

### Dataflow, NetworkTables & WebSocket Telemetry
- [x] Asynchronous pub/sub event bus (`DataChangeService`, `DataChangeEvent`, `DataChangeSubscriber`) - Implemented in `photon-core/src/main/java/org/photonvision/common/dataflow/`
- [x] NetworkTables v4 publisher, time sync, and listeners (`NetworkTablesManager`, `NTDataPublisher`, `TimeSyncManager`, `NTDriverStation`) - Implemented in `photon-core/src/main/java/org/photonvision/common/dataflow/networktables/`
- [x] WebSocket UI data publisher and DTOs (`UIDataPublisher`, `UIPhotonConfiguration`, `UICameraConfiguration`) - Implemented in `photon-core/src/main/java/org/photonvision/common/dataflow/websocket/`
- [x] Status LED pipeline consumer (`StatusLEDConsumer`) - Implemented in `photon-core/src/main/java/org/photonvision/common/dataflow/statusLEDs/`

### Hardware, Networking & Logging
- [x] Hardware & GPIO abstraction (`HardwareManager`, `VisionLED`, `StatusLED`, `CustomAdapter`) - Implemented in `photon-core/src/main/java/org/photonvision/common/hardware/`
- [x] Platform-specific system metrics monitors (`SystemMonitorRaspberryPi`, `SystemMonitorRK3588`, `SystemMonitorQCS6490`, `SystemMonitorWindows`, `DeviceMetricsProto`) - Implemented in `photon-core/src/main/java/org/photonvision/common/hardware/metrics/`
- [x] Linux NetworkManager integration (`NetworkManager`, `NetworkingCommands`, `NetworkUtils`) - Implemented in `photon-core/src/main/java/org/photonvision/common/networking/`
- [x] Logging subsystem with CSCore and kernel log integration (`Logger`, `KernelLogLogger`, `PvCSCoreLogger`) - Implemented in `photon-core/src/main/java/org/photonvision/common/logging/`

### Cameras, Frame Providers & Consumers
- [x] USB camera source & sensor-specific settables (OV2311, OV9281, OV9782, LifeCam, PS3 Eye, See3CAM, Generic) - Implemented in `photon-core/src/main/java/org/photonvision/vision/camera/USBCameras/`
- [x] CSI Libcamera GPU source & settables (`LibcameraGpuSource`, `LibcameraGpuSettables`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/camera/csi/`
- [x] USB, Libcamera GPU, and File frame providers (`USBFrameProvider`, `LibcameraGpuFrameProvider`, `FileFrameProvider`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/frame/provider/`
- [x] MJPEG streaming and frame snapshot saving consumers (`MJPGFrameConsumer`, `FileSaveFrameConsumer`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/frame/consumer/`

### Vision Pipelines, Pipes & Object Detection
- [x] Core `CVPipe` framework and 37 vision processing pipes - Implemented in `photon-core/src/main/java/org/photonvision/vision/pipe/`
- [x] Vision pipelines (`AprilTagPipeline`, `ArucoPipeline`, `ReflectivePipeline`, `ColoredShapePipeline`, `ObjectDetectionPipeline`, `Calibrate3dPipeline`, `DriverModePipeline`, `FocusPipeline`, `OutputStreamPipeline`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/pipeline/`
- [x] RKNN and TFLite object detection backends (`RknnObjectDetector`, `TFLiteObjectDetector`, `Letterbox`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/objects/`
- [x] 3D camera calibration & board observations (`CameraCalibrationCoefficients`, `BoardObservation`, `Calibrate3dPipe`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/calibration/`
- [x] Runtime orchestration (`VisionSourceManager`, `VisionModuleManager`, `VisionModule`, `VisionRunner`, `PipelineManager`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/processes/`
- [x] Target representation and geometry calculations (`TrackedTarget`, `PotentialTarget`, `TargetModel`, `TargetCalculations`) - Implemented in `photon-core/src/main/java/org/photonvision/vision/target/`

## Identified Gaps & Codebase TODOs

### Vision Pipelines & Pipes
- [ ] Refactor multi-tag global state and circular dependencies in `AprilTagPipeline` (`AprilTagPipeline.java:125, 152, 159, 183`) and `ArucoPipeline` (`ArucoPipeline.java:119, 152, 175`) so tag layout and single-tag estimation are cleanly decoupled inside pipes.
- [ ] Fix hardcoded `this.pipelineType = PipelineType.ObjectDetection` initialization in `ObjectDetectionPipelineSettings.java:31`.
- [ ] Review contour grouping logic note in `GroupContoursPipe.java:61` and "infinite" contour refactoring in `Contour.java:188`.
- [ ] Decide whether to implement target skew calculation or remove placeholder fields in `TrackedTarget.java:153, 214`, and document corner ordering invariants (`TrackedTarget.java:309, 342`).
- [ ] Evaluate refactoring `MathUtils.java:174` pose conversion logic into a dedicated pipe.

### Camera Sources & Frame Providers
- [ ] Avoid extra frame copy and consider adding a frame pool in `USBFrameProvider.java:73, 85`.
- [ ] Fix 1280x720 resolution handling in native Libcamera code and add video modes for newer CSI sensors in `LibcameraGpuSettables.java:82, 113`.
- [ ] Create a dedicated File/Replay camera info type instead of reusing USB camera info in `FileVisionSource.java:44, 59`.
- [ ] Clarify camera quirk detection ownership between `CameraConfiguration` and `USBCameraSource` (`USBCameraSource.java:64, 69, 70`, `CameraConfiguration.java:101`) and post-refactor behavior in `GenericUSBCameraSettables.java:67`.

### Process Management & Concurrency
- [ ] Improve thread safety and synchronization between web server threads and `VisionRunner` in `PipelineManager.java:200, 380`.
- [ ] Move camera LED control into a camera-level property controllable in driver mode (`VisionModule.java:623`) and extract helper methods (`VisionModule.java:699`).
- [ ] Evaluate replacing callback lambdas with a callback object in `VisionRunner.java:236`.

### Configuration, Dataflow & Infrastructure
- [ ] Verify SQLite connection closing behavior on save in `ConfigProvider.java:401` and `SQLConfigTest.java:313`.
- [ ] Verify duplicate pipeline nickname suffixing and index assignment logic in `CameraConfiguration.java:122, 128`.
- [ ] Add failure logging when setting pipeline index fails in `NTDataPublisher.java:103` and verify NT4 flushing requirement at `NTDataPublisher.java:264`.
- [ ] Ensure coprocessor subtable uniqueness (e.g., via MAC address) in `TimeSyncManager.java:47`.
- [ ] Add support for reporting additional GPU acceleration types in `UIPhotonConfiguration.java:61`.
- [ ] Refactor `Logger.java:32, 39, 178` to remove static initializer blocks, adopt a singleton pattern, and profile logging overhead.
- [ ] Add command execution timeout in `PlatformUtils.java:38`.

### Testing Gaps
- [ ] Re-enable and diagnose `CVMat.getMatCount()` leak assertion in `Calibrate3dPipeTest.java:352`.
- [ ] Replace hardcoded test expectations in `TrackedTargetTest.java:67` and introduce test fixtures in `VisionRunnerTest.java:62`.
- [ ] Add unit test coverage for `common/networking` (`NetworkManager`, `NetworkUtils`) and `vision/objects` (`Letterbox`, `ObjectDetector` filtering/post-processing).
