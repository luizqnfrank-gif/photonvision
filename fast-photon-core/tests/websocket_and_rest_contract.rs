use axum::body::Body;
use axum::http::{Request, StatusCode};
use fast_photon_core::common::configuration::{CameraConfiguration, ConfigProvider, PVCameraInfo};
use fast_photon_core::common::dataflow::networktables::{
    NTDataPublisher, NtTopicValue, TimeSyncClient, TimeSyncServer,
};
use fast_photon_core::common::dataflow::{
    CVPipelineResultConsumer, ClosureSubscriber, DataChangeDestination, DataChangeService,
    DataChangeSource,
};
use fast_photon_core::hal::hardware_manager::HardwareManager;
use fast_photon_core::hal::networking::NetworkManager;
use fast_photon_core::targeting::{
    PhotonPipelineMetadata, PhotonPipelineResult, PhotonTrackedTarget,
};
use fast_photon_core::vision::processes::VisionSourceManager;
use fast_photon_core::web::handlers::AppState;
use fast_photon_core::web::mjpeg::format_mjpeg_frame;
use fast_photon_core::web::server::build_app_router;
use fast_photon_core::web::websocket::DataSocketHandler;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tower::ServiceExt;

#[tokio::test]
async fn test_nt4_publisher_and_udp_timesync_roundtrip() {
    // 1. Test UDP TimeSync server & client round-trip
    let server = TimeSyncServer::new(0);
    let (port, _handle) = server.spawn_udp_responder().expect("bind timesync server");
    let client = TimeSyncClient::new("127.0.0.1", port);
    let meta = client.send_ping_once().expect("send timesync ping");
    assert_eq!(meta.pings_sent, 1);
    assert_eq!(meta.pongs_received, 1);
    assert!(meta.last_pong_time_ns > 0);

    // 2. Test NTDataPublisher topic set and request callbacks
    let chosen_index = Arc::new(Mutex::new(0i32));
    let idx_clone = chosen_index.clone();
    let publisher = NTDataPublisher::new("FrontCam", true).with_callbacks(
        move |new_idx| {
            if let Ok(mut g) = idx_clone.lock() {
                *g = new_idx;
            }
        },
        |_| {},
        |_| {},
        |_| {},
    );

    publisher.handle_pipeline_index_request(2);
    assert_eq!(*chosen_index.lock().expect("lock"), 2);

    let res = PhotonPipelineResult {
        metadata: PhotonPipelineMetadata {
            sequence_id: 10,
            capture_timestamp_micros: 1000,
            publish_timestamp_micros: 4000,
            time_since_last_pong: 500,
        },
        targets: vec![PhotonTrackedTarget {
            yaw: 11.5,
            pitch: -4.2,
            area: 3.1,
            ..PhotonTrackedTarget::default()
        }],
        multitag_result: None,
    };
    publisher.accept(&res, 60.0);

    let ts = publisher.get_topic_set();
    assert_eq!(ts.subtable_path, "/photonvision/FrontCam");
    assert_eq!(
        ts.topics.get("hasTarget"),
        Some(&NtTopicValue::Boolean(true))
    );
    assert_eq!(
        ts.topics.get("targetYaw"),
        Some(&NtTopicValue::Double(11.5))
    );
    assert_eq!(ts.topics.get("heartbeat"), Some(&NtTopicValue::Integer(10)));
}

#[tokio::test]
async fn test_websocket_msgpack_and_rest_api_contracts() {
    let tmp = TempDir::new().expect("tempdir");
    let config_provider = ConfigProvider::new(tmp.path()).expect("init config");
    let dc_service = DataChangeService::new();
    let hw_manager = HardwareManager::default();
    let net_manager = NetworkManager::new();

    let mut vsm = VisionSourceManager::new(dc_service.clone(), tmp.path().join("snaps"));
    let mut cam = CameraConfiguration::new(PVCameraInfo::File {
        path: "test.png".to_string(),
        name: "TestFileCam".to_string(),
        unique_path: "test.png".to_string(),
    });
    cam.unique_name = "test_cam_1".to_string();
    vsm.register_loaded_configs(vec![cam]);

    let received_events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = received_events.clone();
    dc_service.add_subscriber(Arc::new(ClosureSubscriber::new(
        DataChangeSource::all(),
        DataChangeDestination::all(),
        move |ev| {
            if let Ok(mut g) = events_clone.lock() {
                g.push((ev.property_name.clone(), ev.data.clone()));
            }
        },
    )));

    // 1. Test MessagePack WebSocket binary command decoding & event dispatch
    let ws_cmd = json!({
        "cameraUniqueName": "test_cam_1",
        "driverMode": true,
        "changePipeline": 1,
        "cameraNickname": "RenamedCam",
        "changePipelineSetting": {
            "cameraUniqueName": "test_cam_1",
            "cameraExposureRaw": 12.5
        }
    });
    let msgpack_bytes = DataSocketHandler::encode_msgpack_value(&ws_cmd).expect("encode msgpack");
    let count = DataSocketHandler::handle_binary_message(
        &msgpack_bytes,
        &dc_service,
        Some(&hw_manager),
        Some(1),
    )
    .expect("handle msgpack");
    assert_eq!(count, 4);
    assert_eq!(received_events.lock().expect("lock").len(), 4);

    // 2. Test MJPEG frame multipart boundary formatting
    let mjpeg_chunk = format_mjpeg_frame(b"FAKE_JPEG_DATA");
    assert!(mjpeg_chunk.starts_with(b"--frame\r\nContent-Type: image/jpeg"));

    // 3. Test Axum REST endpoints
    let state = AppState {
        config_provider,
        data_change_service: dc_service,
        vision_source_manager: Arc::new(Mutex::new(vsm)),
        hardware_manager: hw_manager,
        network_manager: net_manager,
    };
    let app = build_app_router(state);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot /api/status");
    assert_eq!(resp.status(), StatusCode::OK);

    let resp_nick = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/camera/setNickname")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "cameraUniqueName": "test_cam_1",
                        "name": "TurretCam"
                    }))
                    .expect("json"),
                ))
                .expect("req"),
        )
        .await
        .expect("oneshot setNickname");
    assert_eq!(resp_nick.status(), StatusCode::OK);
}
