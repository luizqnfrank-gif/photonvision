# NetworkTables v4, TimeSync & Binary Wire Contracts

**Feature**: `001-fast-photon-core`  
**Date**: 2026-11-23

---

## 1. Root NetworkTables Hierarchy (`/photonvision`)

All topics are published under the root table `/photonvision`.

### 1.1 Global Topics (`/photonvision/*`)
| Topic Path | NT4 Type | Direction | Description |
| :--- | :--- | :--- | :--- |
| `/photonvision/version` | `string` | Publish | PhotonVision semantic version string (`PhotonVersion`) |
| `/photonvision/ledMode` | `int` | Pub/Sub | Global Vision LED override (`-1` = Pipeline, `0` = Off, `1` = On, `2` = Blink) |
| `/photonvision/ledModeRequest` | `int` | Subscribe | Robot-requested LED mode |
| `/photonvision/metrics` | `protobuf:photonvision.proto.ProtobufDeviceMetrics` | Publish | Coprocessor hardware metrics (`DeviceMetricsProto`) |

### 1.2 TimeSync Subtable (`/photonvision/.timesync/<hostname>/*`)
Synchronizes coprocessor monotonic clock (`nt::Now()`) with the RoboRIO NT4 server via UDP port `5810`:
| Topic Path | NT4 Type | Direction | Description |
| :--- | :--- | :--- | :--- |
| `/photonvision/.timesync/<hostname>/offset_ns` | `int` | Publish | Clock offset to server in nanoseconds |
| `/photonvision/.timesync/<hostname>/rtt2_ns` | `int` | Publish | Half round-trip time in nanoseconds |
| `/photonvision/.timesync/<hostname>/ping_tx_count` | `int` | Publish | Total UDP TimeSync pings transmitted |
| `/photonvision/.timesync/<hostname>/pong_rx_count` | `int` | Publish | Total UDP TimeSync pongs received |
| `/photonvision/.timesync/<hostname>/pong_rx_time_ns` | `int` | Publish | Timestamp of most recent pong in nanoseconds |

### 1.3 Per-Camera Subtable (`/photonvision/<camera_nickname>/*`)
Each active `VisionModule` publishes and subscribes to the following topics under `/photonvision/<camera_nickname>` (per `NTTopicSet.java`):
| Topic Name | NT4 Type / Schema | Pub/Sub Options | Direction |
| :--- | :--- | :--- | :--- |
| `rawBytes` | `photonstruct:PhotonPipelineResult:fd353b11f3a818e9c5b457c7dc5a8f5c` | `periodic(0.01)`, `SEND_ALL`, `KEEP_DUPLICATES` | Publish |
| `result_proto` | `protobuf:photonvision.proto.ProtobufPhotonPipelineResult` | `periodic(0.01)`, `SEND_ALL` | Publish (when `shouldPublishProto` enabled) |
| `pipelineIndexState` | `int` | Default | Publish |
| `pipelineIndexRequest` | `int` | Default `0` | Subscribe |
| `driverMode` | `boolean` | Default | Publish |
| `driverModeRequest` | `boolean` | Default `false` | Subscribe |
| `fpsLimit` | `int` | Default | Publish |
| `fpsLimitRequest` | `int` | Default `-1` | Subscribe |
| `enabled` | `boolean` | Default | Publish |
| `enabledRequest` | `boolean` | Default `true` | Subscribe |
| `latencyMillis` | `double` | Default | Publish |
| `fps` | `double` | Default | Publish |
| `hasTarget` | `boolean` | Default | Publish |
| `targetPitch` | `double` | Default | Publish |
| `targetYaw` | `double` | Default | Publish |
| `targetArea` | `double` | Default | Publish |
| `targetSkew` | `double` | Default | Publish |
| `targetPose` | `struct:Transform3d` (56 bytes LE) | Default | Publish |
| `targetPixelsX` | `double` | Default | Publish |
| `targetPixelsY` | `double` | Default | Publish |
| `heartbeat` | `int` | Monotonic frame counter | Publish |
| `cameraIntrinsics` | `double[]` (9 elements row-major) | Default | Publish |
| `cameraDistortion` | `double[]` (5 or 8+ elements) | Default | Publish |

---

## 2. Photon Binary `Packet` Wire Format (`rawBytes`)

All multi-byte values are **Little-Endian (LE)**:
1. **`PhotonPipelineMetadata`** (`5231116d7f87ff560e6aa5263d0eee02`, 32 bytes):
   - `sequenceID: i64` (8 bytes)
   - `captureTimestampMicros: i64` (8 bytes)
   - `publishTimestampMicros: i64` (8 bytes)
   - `timeSinceLastPong: i64` (8 bytes)
2. **`targets: Vec<PhotonTrackedTarget>`** (`cc6dbb5c5c1e0fa808108019b20863f1`):
   - `len: u8` (1 byte, max 255 targets)
   - For each target:
     - `yaw: f64`, `pitch: f64`, `area: f64`, `skew: f64` (32 bytes)
     - `fiducialId: i32`, `objDetectId: i32`, `objDetectConf: f32` (12 bytes)
     - `bestCameraToTarget: Transform3d` (56 bytes: `x, y, z, w, qx, qy, qz` as `f64`)
     - `altCameraToTarget: Transform3d` (56 bytes: `x, y, z, w, qx, qy, qz` as `f64`)
     - `poseAmbiguity: f64` (8 bytes)
     - `minAreaRectCorners`: `u8` len + `len * (f64 x, f64 y)`
     - `detectedCorners`: `u8` len + `len * (f64 x, f64 y)`
3. **`multitagResult: Option<MultiTargetPNPResult>`** (`541096947e9f3ca2d3f425ff7b04aa7b`):
   - `present: u8` (`0` = None, `1` = Some)
   - If `present == 1`:
     - `estimatedPose: PnpResult` (`best: Transform3d` 56B, `bestReprojErr: f64` 8B, `alt: Transform3d` 56B, `altReprojErr: f64` 8B, `ambiguity: f64` 8B = 136 bytes)
     - `fiducialIDsUsed`: `u8` len + `len * i16`

---

## 3. WebSocket `/websocket_data` MessagePack Protocol

All WebSocket frames on `/websocket_data` are binary **MessagePack** maps:
- **Inbound (`WSMessage` from `photon-client`)**:
  - Optional `"cameraUniqueName": String`
  - Plus one or more command entries keyed by `DataSocketMessageType`:
    - `"driverMode": bool`
    - `"cameraNickname": String`
    - `"pipelineName": String`
    - `"newPipelineInfo": [String, i32]` (`[name, pipelineTypeOrdinalOffset]`)
    - `"changeBrightness": i32`
    - `"duplicatePipeline": i32`
    - `"deleteCurrPipeline": i32`
    - `"robotOffsetPoint": i32`
    - `"changeUICamera": i32`
    - `"changePipeline": i32`
    - `"startCalibration": Map<String, Value>`
    - `"saveInputSnapshot": i32`
    - `"saveOutputSnapshot": i32`
    - `"takeCalSnapshot": i32`
    - `"changePipelineSetting": Map<String, Value>` (includes `"cameraUniqueName"`)
    - `"changePipelineType": i32`
- **Outbound (`OutgoingUIEvent` to `photon-client`)**:
  - Map `{ "<propertyName>": <value> }` or camera-scoped `{ "<propertyName>": { "<cameraUniqueName>": <value> } }` (e.g., `"settings"`, `"uiResults"`, `"calibrationData"`, `"log"`, `"metrics"`).
