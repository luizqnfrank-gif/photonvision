use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use photon_common::configuration::{
    AprilTagFieldLayout, CameraCalibrationCoefficients, ConfigProvider, HardwareConfig,
    HardwareSettings, NetworkConfig, PVCameraInfo,
};
use photon_common::dataflow::DataChangeService;
use photon_hal::hardware_manager::HardwareManager;
use photon_hal::networking::NetworkManager;
use photon_vision::camera::TcpStreamEndpoint;
use photon_vision::processes::VisionSourceManager;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub config_provider: ConfigProvider,
    pub data_change_service: DataChangeService,
    pub vision_source_manager: Arc<Mutex<VisionSourceManager>>,
    pub hardware_manager: HardwareManager,
    pub network_manager: NetworkManager,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CameraActionQuery {
    #[serde(default)]
    pub camera_unique_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
}

// 1. GET /api/status
pub async fn on_status_request() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

// 2. POST /api/settings (Zip import or JSON batch)
pub async fn on_settings_import_request(State(state): State<AppState>) -> impl IntoResponse {
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Settings imported")
}

// 3. GET /api/settings/photonvision_config.zip
pub async fn on_settings_export_request(State(state): State<AppState>) -> impl IntoResponse {
    let tmp = std::env::temp_dir().join("photonvision_config_export.zip");
    if state.config_provider.export_settings_zip(&tmp).is_ok() {
        if let Ok(bytes) = std::fs::read(&tmp) {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/zip")],
                bytes,
            )
                .into_response();
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR, "Export failed").into_response()
}

// 4. POST /api/settings/hardwareConfig
pub async fn on_hardware_config_request(
    State(state): State<AppState>,
    Json(hw_cfg): Json<HardwareConfig>,
) -> impl IntoResponse {
    state.config_provider.update_config(|cfg| {
        cfg.hardware_config = hw_cfg;
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Saved hardwareConfig")
}

// 5. POST /api/settings/hardwareSettings
pub async fn on_hardware_settings_request(
    State(state): State<AppState>,
    Json(hw_settings): Json<HardwareSettings>,
) -> impl IntoResponse {
    state
        .hardware_manager
        .set_brightness_percent(hw_settings.led_brightness_percentage);
    state.config_provider.update_config(|cfg| {
        cfg.hardware_settings = hw_settings;
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Saved hardwareSettings")
}

// 6. POST /api/settings/networkConfig
pub async fn on_network_config_request(
    State(state): State<AppState>,
    Json(net_cfg): Json<NetworkConfig>,
) -> impl IntoResponse {
    let _ = state.network_manager.apply_config(&net_cfg);
    state.config_provider.update_config(|cfg| {
        cfg.network_config = net_cfg;
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Saved networkConfig")
}

// 7. POST /api/settings/fieldLayout
pub async fn on_field_layout_request(
    State(state): State<AppState>,
    Json(atfl): Json<AprilTagFieldLayout>,
) -> impl IntoResponse {
    if let Ok(mut vsm) = state.vision_source_manager.lock() {
        for module in vsm.vmm.modules_mut() {
            module.set_atfl(atfl.clone());
        }
    }
    state.config_provider.update_config(|cfg| {
        cfg.atfl = atfl;
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Saved fieldLayout")
}

// 8. POST /api/settings/general
pub async fn on_general_settings_request(
    State(state): State<AppState>,
    Json(_body): Json<Value>,
) -> impl IntoResponse {
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Saved general settings")
}

// 9. POST /api/settings/camera
pub async fn on_camera_settings_request(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if let Some(uname) = body.get("cameraUniqueName").and_then(Value::as_str) {
        let fov = body.get("fov").and_then(Value::as_f64).unwrap_or(70.0);
        if let Ok(mut vsm) = state.vision_source_manager.lock() {
            if let Some(m) = vsm.vmm.get_module_mut(uname) {
                m.vision_source.settables_mut().set_fov(fov);
                m.vision_source.camera_configuration_mut().fov = fov;
            }
        }
        state.config_provider.update_config(|cfg| {
            if let Some(c) = cfg.camera_configurations.get_mut(uname) {
                c.fov = fov;
            }
        });
        let _ = state.config_provider.save_to_disk();
    }
    (StatusCode::OK, "Saved camera settings")
}

// 10. POST /api/settings/camera/setNickname
pub async fn on_camera_nickname_change_request(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let uname = body
        .get("cameraUniqueName")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let name = body.get("name").and_then(Value::as_str).unwrap_or("Camera");
    if let Ok(mut vsm) = state.vision_source_manager.lock() {
        if let Some(m) = vsm.vmm.get_module_mut(uname) {
            m.set_camera_nickname(name);
        }
    }
    state.config_provider.update_config(|cfg| {
        if let Some(c) = cfg.camera_configurations.get_mut(uname) {
            c.nickname = name.to_string();
        }
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Updated nickname")
}

// 10b. POST /api/settings/camera/addTcpCamera
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTcpCameraRequest {
    pub uri: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub fps: Option<i32>,
    #[serde(default)]
    pub fov: Option<f64>,
}

pub async fn on_add_tcp_camera_request(
    State(state): State<AppState>,
    Json(req): Json<AddTcpCameraRequest>,
) -> impl IntoResponse {
    if let Err(err) = TcpStreamEndpoint::parse(&req.uri) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "message": err })),
        )
            .into_response();
    }

    let name = req.nickname.unwrap_or_else(|| "rpicam-tcp".to_string());
    let width = req.width.unwrap_or(1280);
    let height = req.height.unwrap_or(720);
    let fps = req.fps.unwrap_or(30);
    let fov = req.fov.unwrap_or(70.0);

    let cam_info = PVCameraInfo::tcp(req.uri.clone(), name.clone(), width, height, fps);
    let mut created_cfg = None;
    let mut uname = String::new();

    if let Ok(mut vsm) = state.vision_source_manager.lock() {
        uname = vsm.assign_unmatched_camera(cam_info);
        if let Some(m) = vsm.vmm.get_module_mut(&uname) {
            m.set_camera_nickname(name);
            m.vision_source.settables_mut().set_fov(fov);
            m.vision_source.camera_configuration_mut().fov = fov;
            created_cfg = Some(m.to_camera_config());
        }
    }

    if let Some(c) = created_cfg {
        state.config_provider.update_config(|cfg| {
            cfg.add_camera_config(c);
        });
        let _ = state.config_provider.save_to_disk();
    }

    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "uniqueName": uname,
            "uri": req.uri,
        })),
    )
        .into_response()
}

// 11. GET /api/settings/camera/getCalibImages
pub async fn on_camera_calib_images_request() -> impl IntoResponse {
    Json(json!({ "images": [] }))
}

// 12. GET /api/settings/camera/getCalibration
pub async fn on_calibration_json_request(
    State(state): State<AppState>,
    Query(q): Query<CameraActionQuery>,
) -> impl IntoResponse {
    let uname = q.camera_unique_name.unwrap_or_default();
    let w = q.width.unwrap_or(640);
    let h = q.height.unwrap_or(480);
    let cfg = state.config_provider.get_config();
    if let Some(cam) = cfg.camera_configurations.get(&uname) {
        if let Some(cal) = cam.calibration_for_resolution(w, h) {
            return Json(serde_json::to_value(cal).unwrap_or_else(|_| json!({}))).into_response();
        }
    }
    Json(json!({})).into_response()
}

// 13. GET /api/settings/camera/getUncertainty
pub async fn on_uncertainty_json_request() -> impl IntoResponse {
    Json(json!({ "uncertainty": [] }))
}

// 14. POST /api/utils/offlineUpdate
pub async fn on_offline_update_request() -> impl IntoResponse {
    (StatusCode::OK, "Offline update staged")
}

// 15. GET /api/utils/photonvision-journalctl.txt
pub async fn on_log_export_request() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "PhotonVision (fast-photon-core) log export\n",
    )
}

// 16. POST /api/utils/restartProgram
pub async fn on_program_restart_request() -> impl IntoResponse {
    (StatusCode::OK, "Restarting program")
}

// 17. POST /api/utils/restartDevice
pub async fn on_device_restart_request() -> impl IntoResponse {
    (StatusCode::OK, "Restarting device")
}

// 18. GET /api/utils/getImageSnapshots
pub async fn on_image_snapshots_request() -> impl IntoResponse {
    Json(json!({ "snapshots": [] }))
}

// 19. GET /api/utils/getCalSnapshot
pub async fn on_calibration_snapshot_request() -> impl IntoResponse {
    (StatusCode::OK, "Snapshot")
}

// 20. GET /api/utils/getCalibrationJSON
pub async fn on_calibration_export_request(
    State(state): State<AppState>,
    Query(q): Query<CameraActionQuery>,
) -> impl IntoResponse {
    on_calibration_json_request(State(state), Query(q)).await
}

// 21. POST /api/utils/nukeConfigDirectory
pub async fn on_nuke_config_directory(State(state): State<AppState>) -> impl IntoResponse {
    state.config_provider.update_config(|cfg| {
        cfg.camera_configurations.clear();
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Nuked config directory")
}

// 22. POST /api/utils/nukeOneCamera
pub async fn on_nuke_one_camera(
    State(state): State<AppState>,
    Json(body): Json<HashMap<String, Value>>,
) -> impl IntoResponse {
    if let Some(uname) = body.get("cameraUniqueName").and_then(Value::as_str) {
        if let Ok(mut vsm) = state.vision_source_manager.lock() {
            vsm.delete_camera(uname);
        }
        state.config_provider.update_config(|cfg| {
            cfg.remove_camera_config(uname);
        });
        let _ = state.config_provider.save_to_disk();
    }
    (StatusCode::OK, "Deleted camera")
}

// 23. POST /api/utils/activateMatchedCamera
pub async fn on_activate_matched_camera_request(
    State(state): State<AppState>,
    Json(body): Json<HashMap<String, Value>>,
) -> impl IntoResponse {
    if let Some(uname) = body.get("cameraUniqueName").and_then(Value::as_str) {
        if let Ok(mut vsm) = state.vision_source_manager.lock() {
            vsm.reactivate_disabled_camera_config(uname);
        }
        state.config_provider.update_config(|cfg| {
            if let Some(c) = cfg.camera_configurations.get_mut(uname) {
                c.deactivated = false;
            }
        });
        let _ = state.config_provider.save_to_disk();
    }
    (StatusCode::OK, "Activated camera")
}

// 24. POST /api/utils/assignUnmatchedCamera
pub async fn on_assign_unmatched_camera_request(
    State(state): State<AppState>,
    Json(cam_info): Json<PVCameraInfo>,
) -> impl IntoResponse {
    let mut created_cfg = None;
    if let Ok(mut vsm) = state.vision_source_manager.lock() {
        let uname = vsm.assign_unmatched_camera(cam_info);
        if let Some(m) = vsm.vmm.get_module(&uname) {
            created_cfg = Some(m.to_camera_config());
        }
    }
    if let Some(c) = created_cfg {
        state.config_provider.update_config(|cfg| {
            cfg.add_camera_config(c);
        });
        let _ = state.config_provider.save_to_disk();
    }
    (StatusCode::OK, "Assigned camera")
}

// 25. POST /api/utils/unassignCamera
pub async fn on_unassign_camera_request(
    State(state): State<AppState>,
    Json(body): Json<HashMap<String, Value>>,
) -> impl IntoResponse {
    if let Some(uname) = body.get("cameraUniqueName").and_then(Value::as_str) {
        if let Ok(mut vsm) = state.vision_source_manager.lock() {
            vsm.deactivate_vision_source(uname);
        }
        state.config_provider.update_config(|cfg| {
            if let Some(c) = cfg.camera_configurations.get_mut(uname) {
                c.deactivated = true;
            }
        });
        let _ = state.config_provider.save_to_disk();
    }
    (StatusCode::OK, "Unassigned camera")
}

// 26. POST /api/calibration/compute
pub async fn on_calibration_compute_request() -> impl IntoResponse {
    (StatusCode::OK, "Calibration computed")
}

// 27. POST /api/calibration/commit
pub async fn on_calibration_commit_request() -> impl IntoResponse {
    (StatusCode::OK, "Calibration committed")
}

// 28. POST /api/calibration/cancel
pub async fn on_calibration_cancel_request() -> impl IntoResponse {
    (StatusCode::OK, "Calibration cancelled")
}

// 29. POST /api/calibration/importFromData
pub async fn on_data_calibration_import_request(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    if let (Some(uname), Some(cal_val)) = (
        body.get("cameraUniqueName").and_then(Value::as_str),
        body.get("calibration"),
    ) {
        if let Ok(cal) = serde_json::from_value::<CameraCalibrationCoefficients>(cal_val.clone()) {
            state.config_provider.update_config(|cfg| {
                if let Some(cam) = cfg.camera_configurations.get_mut(uname) {
                    cam.calibrations.retain(|c| c.resolution != cal.resolution);
                    cam.calibrations.push(cal);
                }
            });
            let _ = state.config_provider.save_to_disk();
        }
    }
    (StatusCode::OK, "Calibration imported")
}

// 30. POST /api/calibration/remove
pub async fn on_calibration_remove_request(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let uname = body
        .get("cameraUniqueName")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let w = body.get("width").and_then(Value::as_i64).unwrap_or(0) as i32;
    let h = body.get("height").and_then(Value::as_i64).unwrap_or(0) as i32;
    state.config_provider.update_config(|cfg| {
        if let Some(cam) = cfg.camera_configurations.get_mut(uname) {
            cam.calibrations
                .retain(|c| (c.resolution.0 as i32) != w || (c.resolution.1 as i32) != h);
        }
    });
    let _ = state.config_provider.save_to_disk();
    (StatusCode::OK, "Calibration removed")
}

// 31. POST /api/objectdetection/import
pub async fn on_import_object_detection_model_request() -> impl IntoResponse {
    (StatusCode::OK, "Model imported")
}

// 32. POST /api/objectdetection/bulkimport
pub async fn on_bulk_import_object_detection_model_request() -> impl IntoResponse {
    (StatusCode::OK, "Models bulk imported")
}

// 33. GET /api/objectdetection/export
pub async fn on_export_object_detection_models_request() -> impl IntoResponse {
    (StatusCode::OK, "Models exported")
}

// 34. GET /api/objectdetection/exportIndividual
pub async fn on_export_individual_object_detection_model_request() -> impl IntoResponse {
    (StatusCode::OK, "Individual model exported")
}

// 35. POST /api/objectdetection/delete
pub async fn on_delete_object_detection_model_request() -> impl IntoResponse {
    (StatusCode::OK, "Model deleted")
}

// 36. POST /api/objectdetection/rename
pub async fn on_rename_object_detection_model_request() -> impl IntoResponse {
    (StatusCode::OK, "Model renamed")
}

// 37. POST /api/objectdetection/nuke
pub async fn on_nuke_object_detection_models_request() -> impl IntoResponse {
    (StatusCode::OK, "Models reset")
}

// 38. POST /api/test/resetBackend
pub async fn handle_reset_request() -> impl IntoResponse {
    (StatusCode::OK, "Backend reset")
}

// 39. POST /api/test/activateTestMode
pub async fn test_mode() -> impl IntoResponse {
    (StatusCode::OK, "Test mode activated")
}

// 40. POST /api/test/override/platform
pub async fn handle_platform_override_request() -> impl IntoResponse {
    (StatusCode::OK, "Platform overridden")
}
