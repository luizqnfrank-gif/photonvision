use crate::handlers::*;
use crate::websocket::DataSocketHandler;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use photon_common::dataflow::websocket::UIPhotonConfiguration;
use photon_common::dataflow::DataChangeDestination;
use serde_json::json;
use tower_http::cors::CorsLayer;

async fn ws_route_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(mut socket: WebSocket, state: AppState) {
    // Send initial full UIPhotonConfiguration snapshot as MessagePack binary message
    let ui_cfg = UIPhotonConfiguration::from(&state.config_provider.get_config());
    let init_payload = json!({ "settings": ui_cfg });
    if let Ok(bytes) = DataSocketHandler::encode_msgpack_value(&init_payload) {
        let _ = socket.send(Message::Binary(bytes)).await;
    }

    let mut rx = state.data_change_service.subscribe_channel();

    loop {
        tokio::select! {
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Binary(bin) = msg {
                    let _ = DataSocketHandler::handle_binary_message(
                        &bin,
                        &state.data_change_service,
                        Some(&state.hardware_manager),
                        None,
                    );
                }
            }
            Ok(ev) = rx.recv() => {
                if ev.dest_type == DataChangeDestination::DCD_UI {
                    let out = json!({ ev.property_name: ev.data });
                    if let Ok(bytes) = DataSocketHandler::encode_msgpack_value(&out) {
                        if socket.send(Message::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                }
            }
            else => break,
        }
    }
}

/// Builds the complete Axum router matching `org.photonvision.server.Server` (port 5800).
pub fn build_app_router(state: AppState) -> Router {
    Router::new()
        .route("/websocket_data", get(ws_route_handler))
        .route("/api/status", get(on_status_request))
        // Settings
        .route("/api/settings", post(on_settings_import_request))
        .route(
            "/api/settings/photonvision_config.zip",
            get(on_settings_export_request),
        )
        .route(
            "/api/settings/hardwareConfig",
            post(on_hardware_config_request),
        )
        .route(
            "/api/settings/hardwareSettings",
            post(on_hardware_settings_request),
        )
        .route(
            "/api/settings/networkConfig",
            post(on_network_config_request),
        )
        .route("/api/settings/fieldLayout", post(on_field_layout_request))
        .route("/api/settings/general", post(on_general_settings_request))
        .route("/api/settings/camera", post(on_camera_settings_request))
        .route(
            "/api/settings/camera/addTcpCamera",
            post(on_add_tcp_camera_request),
        )
        .route(
            "/api/settings/camera/setNickname",
            post(on_camera_nickname_change_request),
        )
        .route(
            "/api/settings/camera/getCalibImages",
            get(on_camera_calib_images_request),
        )
        .route(
            "/api/settings/camera/getCalibration",
            get(on_calibration_json_request),
        )
        .route(
            "/api/settings/camera/getUncertainty",
            get(on_uncertainty_json_request),
        )
        // Utilities
        .route("/api/utils/offlineUpdate", post(on_offline_update_request))
        .route(
            "/api/utils/photonvision-journalctl.txt",
            get(on_log_export_request),
        )
        .route(
            "/api/utils/restartProgram",
            post(on_program_restart_request),
        )
        .route("/api/utils/restartDevice", post(on_device_restart_request))
        .route(
            "/api/utils/getImageSnapshots",
            get(on_image_snapshots_request),
        )
        .route(
            "/api/utils/getCalSnapshot",
            get(on_calibration_snapshot_request),
        )
        .route(
            "/api/utils/getCalibrationJSON",
            get(on_calibration_export_request),
        )
        .route(
            "/api/utils/nukeConfigDirectory",
            post(on_nuke_config_directory),
        )
        .route("/api/utils/nukeOneCamera", post(on_nuke_one_camera))
        .route(
            "/api/utils/activateMatchedCamera",
            post(on_activate_matched_camera_request),
        )
        .route(
            "/api/utils/assignUnmatchedCamera",
            post(on_assign_unmatched_camera_request),
        )
        .route(
            "/api/utils/unassignCamera",
            post(on_unassign_camera_request),
        )
        // Calibration
        .route(
            "/api/calibration/compute",
            post(on_calibration_compute_request),
        )
        .route(
            "/api/calibration/commit",
            post(on_calibration_commit_request),
        )
        .route(
            "/api/calibration/cancel",
            post(on_calibration_cancel_request),
        )
        .route(
            "/api/calibration/importFromData",
            post(on_data_calibration_import_request),
        )
        .route(
            "/api/calibration/remove",
            post(on_calibration_remove_request),
        )
        // Object Detection
        .route(
            "/api/objectdetection/import",
            post(on_import_object_detection_model_request),
        )
        .route(
            "/api/objectdetection/bulkimport",
            post(on_bulk_import_object_detection_model_request),
        )
        .route(
            "/api/objectdetection/export",
            get(on_export_object_detection_models_request),
        )
        .route(
            "/api/objectdetection/exportIndividual",
            get(on_export_individual_object_detection_model_request),
        )
        .route(
            "/api/objectdetection/delete",
            post(on_delete_object_detection_model_request),
        )
        .route(
            "/api/objectdetection/rename",
            post(on_rename_object_detection_model_request),
        )
        .route(
            "/api/objectdetection/nuke",
            post(on_nuke_object_detection_models_request),
        )
        // Testing API
        .route("/api/test/resetBackend", post(handle_reset_request))
        .route("/api/test/activateTestMode", post(test_mode))
        .route(
            "/api/test/override/platform",
            post(handle_platform_override_request),
        )
        .layer(CorsLayer::permissive())
        .with_state(state)
}
