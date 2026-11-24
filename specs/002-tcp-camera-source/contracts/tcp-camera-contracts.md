# TCP Camera Source Contracts (Wire, CLI, REST & SQLite)

**Feature**: `002-tcp-camera-source`  
**Date**: 2026-11-23

---

## 1. `rpicam-vid` TCP Stream Wire Contract

### 1.1 Sender Command (Raspberry Pi Camera Node)
```bash
rpicam-vid -t 0 --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:8888
```
- **Transport**: TCP over IPv4/IPv6 (default port `8888`).
- **Role**: `--listen` causes `rpicam-vid` to bind `0.0.0.0:8888` and wait for a TCP client connection from `fast-photon-core`. (If `--listen` is omitted on the sender, `fast-photon-core` can bind locally when configured with `tcp://0.0.0.0:8888?listen=true`).
- **Payload Formats Supported**:
  1. **H.264 Annex-B Elementary Stream (Default `rpicam-vid --inline`)**:
     - Start codes: `0x00 0x00 0x00 0x01` (4-byte) or `0x00 0x00 0x01` (3-byte).
     - `--inline` guarantees SPS (`NAL type 7`, byte `0x67` / `0x27`) and PPS (`NAL type 8`, byte `0x68` / `0x28`) headers precede every IDR keyframe (`NAL type 5`, byte `0x65` / `0x25`), allowing late-joining clients to synchronize at any keyframe boundary.
  2. **MJPEG Stream (`rpicam-vid --codec mjpeg`)**:
     - Concatenated JPEG frames delimited by SOI (`0xFF 0xD8`) and EOI (`0xFF 0xD9`).

---

## 2. SQLite & JSON Schema Contract (`PVCameraInfo::Tcp`)

Stored inside the `cameras.contents` JSON column in `photon.sqlite`:

```json
{
  "uniqueName": "rpicam_tcp_8888",
  "nickname": "RPiCam_TCP_8888",
  "deactivated": false,
  "FOV": 70.0,
  "currentPipelineIndex": 0,
  "streamIndex": 0,
  "matchedCameraInfo": {
    "type": "PVCameraInfo.PVTcpCameraInfo",
    "uri": "tcp://192.168.1.50:8888",
    "name": "RPiCam_TCP_8888",
    "width": 1280,
    "height": 720,
    "fps": 30,
    "uniquePath": "tcp://192.168.1.50:8888"
  },
  "pipelineSettings": [
    {
      "type": "AprilTagPipelineSettings",
      "pipelineIndex": 0,
      "pipelineNickname": "AprilTag_720p"
    }
  ]
}
```

---

## 3. HTTP REST API Contracts (`crates/photon-web`)

### 3.1 `POST /api/settings/camera/addTcpCamera`
Registers and activates a new TCP camera stream source, persisting it to `photon.sqlite`.

- **Request Body (`application/json`)**:
  ```json
  {
    "uri": "tcp://192.168.1.50:8888",
    "nickname": "RPiCam_Front",
    "width": 1280,
    "height": 720,
    "fps": 30,
    "fov": 70.0
  }
  ```
  *(Only `"uri"` is required; `"nickname"` defaults to `"rpicam-tcp"`, `"width"` to `1280`, `"height"` to `720`, `"fps"` to `30`, and `"fov"` to `70.0`.)*
- **Response (`200 OK`, `application/json`)**:
  ```json
  {
    "status": "ok",
    "uniqueName": "d4e5f6a7-...",
    "uri": "tcp://192.168.1.50:8888"
  }
  ```
- **Error Response (`400 Bad Request`)**: Returned if `"uri"` is invalid or missing a valid TCP `host:port`.

### 3.2 `POST /api/utils/assignUnmatchedCamera`
Existing endpoint enhanced to accept `PVCameraInfo::Tcp` (`"type": "PVCameraInfo.PVTcpCameraInfo"` or `"PVTcpCameraInfo"`) and instantiate a `TcpCameraSource` `VisionModule`.

---

## 4. Standalone CLI Contract (`fast-photon-core/src/main.rs`)

```text
fast-photon-core [OPTIONS]

Options:
      --config-dir <CONFIG_DIR>    Path to photon.sqlite configuration directory [default: photonvision_config]
      --port <PORT>                HTTP/WebSocket server port [default: 5800]
      --tcp-camera <TCP_CAMERA>    Register one or more TCP camera stream URIs on startup (e.g. tcp://192.168.1.50:8888)
      --test-mode                  Run a single startup validation pass and exit
```
