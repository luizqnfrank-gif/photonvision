use axum::body::Body;
use axum::http::{Request, StatusCode};
use fast_photon_core::common::configuration::{
    CVPipelineSettings, CameraConfiguration, CameraType, ConfigProvider, PVCameraInfo,
    PipelineSettingsVariant,
};
use fast_photon_core::common::dataflow::DataChangeService;
use fast_photon_core::common::util::{DoubleCouple, IntegerCouple};
use fast_photon_core::hal::hardware_manager::HardwareManager;
use fast_photon_core::hal::networking::NetworkManager;
use fast_photon_core::vision::camera::{TcpCameraSource, TcpStreamDecoder, TcpStreamEndpoint};
use fast_photon_core::vision::opencv_ext::CVMat;
use fast_photon_core::vision::processes::{VisionModule, VisionSource, VisionSourceManager};
use fast_photon_core::web::handlers::AppState;
use fast_photon_core::web::server::build_app_router;
use image::{DynamicImage, Rgb, RgbImage};
use openh264::encoder::Encoder;
use openh264::formats::{RgbSliceU8, YUVBuffer};
use serde_json::{json, Value};
use std::io::Write;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tower::ServiceExt;

fn make_green_target_mat_720p() -> CVMat {
    let mut img = RgbImage::from_pixel(1280, 720, Rgb([10u8, 10u8, 10u8]));
    // Draw bright green target rectangle centered at (640, 360)
    for y in 310..=410 {
        for x in 590..=690 {
            img.put_pixel(x, y, Rgb([0u8, 255u8, 0u8]));
        }
    }
    CVMat::from_dynamic(DynamicImage::ImageRgb8(img))
}

fn encode_h264_annex_b_inline_frames(width: usize, height: usize, count: usize) -> Vec<u8> {
    let mut encoder = Encoder::new().expect("create openh264 encoder");
    let mut rgb = vec![16u8; width * height * 3];
    // Draw a bright green square in the middle of the frame
    let cx = width / 2;
    let cy = height / 2;
    for y in cy.saturating_sub(24)..=(cy + 24).min(height - 1) {
        for x in cx.saturating_sub(24)..=(cx + 24).min(width - 1) {
            let idx = (y * width + x) * 3;
            rgb[idx] = 0;
            rgb[idx + 1] = 255;
            rgb[idx + 2] = 0;
        }
    }
    let yuv = YUVBuffer::from_rgb8_source(RgbSliceU8::new(&rgb, (width, height)));
    let mut stream = Vec::new();
    for _ in 0..count {
        let bitstream = encoder.encode(&yuv).expect("encode h264 frame");
        bitstream.write_vec(&mut stream);
    }
    stream
}

#[test]
fn test_tcp_stream_endpoint_parsing() {
    let ep1 = TcpStreamEndpoint::parse("tcp://0.0.0.0:8888").expect("parse tcp uri");
    assert_eq!(ep1.host, "0.0.0.0");
    assert_eq!(ep1.port, 8888);
    assert!(!ep1.listen);

    let ep2 =
        TcpStreamEndpoint::parse("tcp://192.168.1.50:8888?listen=true").expect("parse listen");
    assert_eq!(ep2.host, "192.168.1.50");
    assert_eq!(ep2.port, 8888);
    assert!(ep2.listen);

    assert!(TcpStreamEndpoint::parse("").is_err());
    assert!(TcpStreamEndpoint::parse("tcp://192.168.1.50").is_err());
}

#[test]
fn test_h264_annex_b_inline_and_late_join_decoding() {
    let h264_bytes = encode_h264_annex_b_inline_frames(320, 240, 3);
    assert!(!h264_bytes.is_empty());

    // Prepend arbitrary garbage bytes to simulate a late-joining client connecting mid-stream
    // before an `--inline` SPS/PPS/IDR header
    let mut noisy_stream = vec![0xAA, 0xBB, 0xCC, 0x12, 0x34];
    noisy_stream.extend_from_slice(&h264_bytes);

    let mut decoder = TcpStreamDecoder::new();
    let mut frames = decoder.push_bytes(&noisy_stream);
    frames.extend(decoder.flush_remaining_h264());

    assert!(
        !frames.is_empty(),
        "expected at least 1 decoded H.264 frame from Annex-B inline stream"
    );
    assert_eq!(frames[0].width(), 320);
    assert_eq!(frames[0].height(), 240);
}

#[test]
fn test_live_tcp_streaming_1280x720_through_vision_module_and_auto_reconnect() {
    // Bind a mock `rpicam-vid --width 1280 --height 720 --framerate 30 --inline --listen -o tcp://0.0.0.0:<port>` server
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral tcp server");
    let port = listener.local_addr().expect("local_addr").port();
    let uri = format!("tcp://127.0.0.1:{port}");

    let jpeg_frame = make_green_target_mat_720p().encode_jpeg();

    // Spawn server thread that accepts Connection #1, sends 3 frames, drops the socket (simulating rpicam-vid restart),
    // then accepts Connection #2 and streams 3 more frames.
    let server_handle = thread::spawn(move || {
        // Connection #1
        if let Ok((mut sock1, _)) = listener.accept() {
            for _ in 0..3 {
                let _ = sock1.write_all(&jpeg_frame);
                let _ = sock1.flush();
                thread::sleep(Duration::from_millis(15));
            }
            // Drop sock1 to simulate network drop / rpicam-vid restart
        }

        thread::sleep(Duration::from_millis(120));

        // Connection #2 (after restart)
        if let Ok((mut sock2, _)) = listener.accept() {
            for _ in 0..5 {
                let _ = sock2.write_all(&jpeg_frame);
                let _ = sock2.flush();
                thread::sleep(Duration::from_millis(30));
            }
            thread::sleep(Duration::from_millis(250));
        }
    });

    // Configure TcpCameraSource with ReflectivePipeline tuned for green target
    let mut cam_cfg =
        CameraConfiguration::new(PVCameraInfo::tcp(uri.clone(), "RPiCam_720p", 1280, 720, 30));
    let reflective_settings = CVPipelineSettings {
        hsv_hue: IntegerCouple::new(40, 80),
        hsv_saturation: IntegerCouple::new(100, 255),
        hsv_value: IntegerCouple::new(100, 255),
        contour_area: DoubleCouple::new(0.05, 50.0),
        ..CVPipelineSettings::default()
    };
    cam_cfg.pipeline_settings = vec![PipelineSettingsVariant::ReflectivePipelineSettings(
        reflective_settings,
    )];

    let mut source = TcpCameraSource::new(cam_cfg.clone());

    // Wait up to 1.5s for Connection #1 to deliver a 1280x720 frame
    let start = Instant::now();
    while !source.tcp_frame_provider().has_frame() && start.elapsed() < Duration::from_millis(1500)
    {
        thread::sleep(Duration::from_millis(15));
    }
    assert!(
        source.tcp_frame_provider().has_frame(),
        "TcpFrameProvider did not receive initial 1280x720 frame"
    );

    let frame = source.frame_provider().get_frame();
    assert_eq!(frame.color_image.width(), 1280);
    assert_eq!(frame.color_image.height(), 720);
    assert_eq!(frame.static_properties.image_width, 1280);
    assert_eq!(frame.static_properties.image_height, 720);

    // Wait for Connection #2 auto-reconnection and run full VisionModule tick
    let reconnect_deadline = Instant::now() + Duration::from_secs(2);
    while !source.frame_provider().is_connected() && Instant::now() < reconnect_deadline {
        thread::sleep(Duration::from_millis(20));
    }

    let dcs = DataChangeService::new();
    let tmp = TempDir::new().expect("tempdir");
    let mut module = VisionModule::new(Box::new(source), dcs, tmp.path().join("snaps"), Vec::new());

    // Ensure module is connected on Connection #2 and runs pipeline
    let tick_deadline = Instant::now() + Duration::from_secs(2);
    let mut pipeline_res = None;
    while Instant::now() < tick_deadline {
        if let Some(res) = module.tick_once() {
            if res.has_targets() {
                pipeline_res = Some(res);
                break;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }

    let res =
        pipeline_res.expect("VisionModule should detect green target from TCP 1280x720 stream");
    assert!(res.has_targets());
    assert!(
        res.targets[0].yaw.abs() < 0.5,
        "expected target near center yaw, got {}",
        res.targets[0].yaw
    );

    let _ = server_handle.join();
}

#[tokio::test]
async fn test_tcp_camera_sqlite_persistence_and_rest_api() {
    let tmp = TempDir::new().expect("tempdir");
    let config_provider = ConfigProvider::new(tmp.path()).expect("init config");
    let dc_service = DataChangeService::new();
    let hw_manager = HardwareManager::default();
    let net_manager = NetworkManager::new();
    let vsm = VisionSourceManager::new(dc_service.clone(), tmp.path().join("snaps"));

    let state = AppState {
        config_provider: config_provider.clone(),
        data_change_service: dc_service,
        vision_source_manager: Arc::new(Mutex::new(vsm)),
        hardware_manager: hw_manager,
        network_manager: net_manager,
    };
    let app = build_app_router(state);

    // 1. Register TCP camera via POST /api/settings/camera/addTcpCamera
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/camera/addTcpCamera")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "uri": "tcp://127.0.0.1:8888",
                        "nickname": "RPiCam_LAN",
                        "width": 1280,
                        "height": 720,
                        "fps": 30
                    }))
                    .expect("json"),
                ))
                .expect("req"),
        )
        .await
        .expect("oneshot addTcpCamera");
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), 16 * 1024)
        .await
        .expect("read body");
    let body_json: Value = serde_json::from_slice(&body_bytes).expect("parse json");
    assert_eq!(body_json["status"], "ok");
    let unique_name = body_json["uniqueName"]
        .as_str()
        .expect("uniqueName string")
        .to_string();

    // 2. Reload ConfigProvider from photon.sqlite on disk and verify PVCameraInfo::Tcp round-trip
    let reloaded = ConfigProvider::new(tmp.path()).expect("reload ConfigProvider");
    let cfg = reloaded.get_config();
    let saved_cam = cfg
        .camera_configurations
        .get(&unique_name)
        .expect("saved TCP camera in photon.sqlite");

    assert_eq!(saved_cam.nickname, "RPiCam_LAN");
    assert_eq!(
        saved_cam.matched_camera_info.camera_type(),
        CameraType::TcpCamera
    );
    assert_eq!(
        saved_cam.matched_camera_info.unique_path(),
        "tcp://127.0.0.1:8888"
    );

    // 3. Invalid URI returns 400 Bad Request
    let bad_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/camera/addTcpCamera")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "uri": "invalid_uri_without_port"
                    }))
                    .expect("json"),
                ))
                .expect("req"),
        )
        .await
        .expect("oneshot invalid addTcpCamera");
    assert_eq!(bad_resp.status(), StatusCode::BAD_REQUEST);
}
