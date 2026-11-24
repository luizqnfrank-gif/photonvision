use fast_photon_core::common::configuration::{
    CameraConfiguration, ConfigProvider, MigrationManager, PVCameraInfo, PipelineSettingsVariant,
    PipelineType, StatusLedConfig, LATEST_SCHEMA_VERSION, SCHEMA_02,
};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn test_schema_migrations_2_to_202704() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("photon.sqlite");

    // 1. Seed a legacy schema v2 database matching pre-2027 PhotonVision format
    {
        let conn = Connection::open(&db_path).expect("open db");
        conn.execute_batch(SCHEMA_02).expect("apply schema 02");
        MigrationManager::set_user_version(&conn, 2).expect("set version 2");

        // Legacy hardwareConfig with statusRGBPins
        conn.execute(
            "INSERT INTO global (filename, contents) VALUES ('hardwareConfig', ?1);",
            [r#"{"deviceName":"OrangePi5","statusRGBPins":[11,12,13],"statusRGBActiveHigh":true,"ledPins":[2,13]}"#],
        )
        .expect("insert hardwareConfig");

        // Legacy apriltagFieldLayout with "field" and "tags"
        conn.execute(
            "INSERT INTO global (filename, contents) VALUES ('apriltagFieldLayout', ?1);",
            [r#"{"field":{"length":16.54,"width":8.01},"tags":[{"ID":1,"pose":{"translation":{"x":1.0,"y":2.0,"z":0.5},"rotation":{"quaternion":{"w":1.0,"x":0.0,"y":0.0,"z":0.0}}}}]}"#],
        )
        .expect("insert apriltagFieldLayout");

        // Legacy camera row with wrapped PVCameraInfo, tuple drivermode_json, and string-encoded pipeline_jsons
        let legacy_cam_config = r#"{
            "uniqueName": "cam_ov9281",
            "nickname": "Front_OV9281",
            "FOV": 75.0,
            "deactivated": false,
            "currentPipelineIndex": 0,
            "matchedCameraInfo": {
                "PVUsbCameraInfo": {
                    "dev": 0,
                    "path": "/dev/video0",
                    "name": "Arducam OV9281",
                    "vendorId": 3141,
                    "productId": 25446,
                    "uniquePath": "/dev/v4l/by-id/usb-Arducam_OV9281"
                }
            }
        }"#;
        let legacy_drivermode = r#"["DriverModePipelineSettings",{"pipelineIndex":-2,"pipelineNickname":"Driver Mode","cameraExposureRaw":15.0}]"#;
        let legacy_pipelines = r#"["[\"AprilTagPipelineSettings\",{\"pipelineIndex\":0,\"pipelineNickname\":\"AprilTag_36h11\",\"outputShowMultipleTargets\":false,\"solvePNPEnabled\":true}]"]"#;
        let legacy_otherpaths = r#"["/dev/video0","/dev/video1"]"#;

        conn.execute(
            "INSERT INTO cameras (unique_name, config_json, drivermode_json, pipeline_jsons, otherpaths_json) VALUES (?1, ?2, ?3, ?4, ?5);",
            (
                "cam_ov9281",
                legacy_cam_config,
                legacy_drivermode,
                legacy_pipelines,
                legacy_otherpaths,
            ),
        )
        .expect("insert legacy camera");
    }

    // 2. Open with ConfigProvider and verify automatic migration to 202704
    let provider = ConfigProvider::new(tmp.path()).expect("init ConfigProvider");
    let conn = Connection::open(provider.db_path()).expect("open migrated db");
    let ver = MigrationManager::get_user_version(&conn).expect("get user_version");
    assert_eq!(ver, LATEST_SCHEMA_VERSION);

    let cfg = provider.get_config();
    assert_eq!(cfg.hardware_config.device_name, "OrangePi5");
    assert_eq!(
        cfg.hardware_config.status_led_config,
        StatusLedConfig::Rgb {
            active_high: true,
            red_pin: 11,
            green_pin: 12,
            blue_pin: 13
        }
    );
    assert_eq!(cfg.atfl.field_tags.len(), 1);
    assert_eq!(cfg.atfl.field_tags[0].id, 1);

    let cam = cfg
        .camera_configurations
        .get("cam_ov9281")
        .expect("migrated camera cam_ov9281");
    assert_eq!(cam.nickname, "Front_OV9281");
    assert_eq!(cam.pipeline_settings.len(), 1);
    assert_eq!(
        cam.pipeline_settings[0].pipeline_type(),
        PipelineType::AprilTag
    );
    // outputShowMultipleTargets: false -> outputMaximumTargets: 1
    assert_eq!(
        cam.pipeline_settings[0].settings().output_maximum_targets,
        1
    );
}

#[test]
fn test_config_provider_roundtrip_and_zip_export_import() {
    let tmp = TempDir::new().expect("tempdir");
    let provider = ConfigProvider::new(tmp.path()).expect("init ConfigProvider");

    let mut cam = CameraConfiguration::new(PVCameraInfo::Usb {
        dev: 1,
        path: "/dev/video2".to_string(),
        name: "LifeCam HD-3000".to_string(),
        other_paths: vec!["/dev/video2".to_string()],
        vendor_id: 0x045e,
        product_id: 0x0779,
        unique_path: "/dev/v4l/by-path/usb-lifecam".to_string(),
    });
    cam.unique_name = "lifecam_uuid".to_string();
    cam.nickname = "IntakeCam".to_string();
    cam.add_pipeline_setting(PipelineSettingsVariant::default());

    provider.update_config(|c| {
        c.network_config.nt_server_address = "3476".to_string();
        c.add_camera_config(cam.clone());
    });
    provider.save_to_disk().expect("save to disk");

    let zip_path = tmp.path().join("exported.zip");
    provider
        .export_settings_zip(&zip_path)
        .expect("export settings zip");
    assert!(zip_path.exists());

    // Import into a clean directory
    let tmp_restore = TempDir::new().expect("tempdir restore");
    let mut restored_provider =
        ConfigProvider::new(tmp_restore.path()).expect("init restored provider");
    restored_provider
        .save_uploaded_settings_zip(&zip_path)
        .expect("import settings zip");

    let restored_cfg = restored_provider.get_config();
    assert_eq!(restored_cfg.network_config.nt_server_address, "3476");
    let restored_cam = restored_cfg
        .camera_configurations
        .get("lifecam_uuid")
        .expect("restored camera");
    assert_eq!(restored_cam.nickname, "IntakeCam");
    assert_eq!(restored_cam.pipeline_settings.len(), 2);
}
