use crate::configuration::{
    CameraCalibrationCoefficients, CameraConfiguration, NetworkConfig, PVCameraInfo,
    PhotonConfiguration, QuirkyCamera,
};
use crate::dataflow::{CVPipelineResultConsumer, DataChangeEvent, DataChangeService};
use photon_targeting::PhotonPipelineResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIGeneralSettings {
    pub version: String,
    pub gpu_acceleration: String,
    pub hardware_model: String,
    pub hardware_platform: String,
    pub wpilib_supported: bool,
    pub mr_cal_working: bool,
    pub available_models: Value,
    pub supported_backends: Vec<String>,
}

impl Default for UIGeneralSettings {
    fn default() -> Self {
        Self {
            version: "2027.0.0-rust".to_string(),
            gpu_acceleration: String::new(),
            hardware_model: "Generic Linux".to_string(),
            hardware_platform: std::env::consts::OS.to_string(),
            wpilib_supported: true,
            mr_cal_working: true,
            available_models: json!({}),
            supported_backends: vec!["TFLITE".to_string()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UILightingConfig {
    pub brightness: i32,
    pub supported: bool,
}

impl Default for UILightingConfig {
    fn default() -> Self {
        Self {
            brightness: 100,
            supported: true,
        }
    }
}

pub type UINetConfig = NetworkConfig;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UICameraConfiguration {
    pub fov: f64,
    pub nickname: String,
    pub unique_name: String,
    pub current_pipeline_index: i32,
    pub current_pipeline_settings: Value,
    pub pipeline_nicknames: Vec<String>,
    pub video_format_list: HashMap<String, Value>,
    pub output_stream_port: i32,
    pub input_stream_port: i32,
    pub calibrations: Vec<CameraCalibrationCoefficients>,
    pub is_fms_connected: bool,
    pub camera_quirks: QuirkyCamera,
    pub is_cscore_connected: bool,
    pub has_connected: bool,
    pub matched_camera_info: PVCameraInfo,
    pub mismatch: bool,
}

use std::collections::HashMap;

impl From<&CameraConfiguration> for UICameraConfiguration {
    fn from(c: &CameraConfiguration) -> Self {
        let current_pipeline_settings = c
            .pipeline_settings
            .get(c.current_pipeline_index.max(0) as usize)
            .and_then(|s| serde_json::to_value(s).ok())
            .unwrap_or_else(|| json!({}));
        let pipeline_nicknames = c
            .pipeline_settings
            .iter()
            .map(|s| s.settings().pipeline_nickname.clone())
            .collect();
        let input_stream_port = 1181 + (c.stream_index * 2);
        let output_stream_port = 1182 + (c.stream_index * 2);

        Self {
            fov: c.fov,
            nickname: c.nickname.clone(),
            unique_name: c.unique_name.clone(),
            current_pipeline_index: c.current_pipeline_index,
            current_pipeline_settings,
            pipeline_nicknames,
            video_format_list: HashMap::new(),
            output_stream_port,
            input_stream_port,
            calibrations: c.calibrations.clone(),
            is_fms_connected: false,
            camera_quirks: c.camera_quirks.clone(),
            is_cscore_connected: !c.deactivated,
            has_connected: !c.deactivated,
            matched_camera_info: c.matched_camera_info.clone(),
            mismatch: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIPhotonConfiguration {
    pub camera_settings: Vec<UICameraConfiguration>,
    pub general: UIGeneralSettings,
    pub lighting: UILightingConfig,
    pub network_settings: UINetConfig,
    pub atfl: Value,
}

impl From<&PhotonConfiguration> for UIPhotonConfiguration {
    fn from(cfg: &PhotonConfiguration) -> Self {
        let camera_settings = cfg
            .camera_configurations
            .values()
            .filter(|c| !c.deactivated)
            .map(UICameraConfiguration::from)
            .collect();
        let lighting = UILightingConfig {
            brightness: cfg.hardware_settings.led_brightness_percentage,
            supported: !cfg.hardware_config.led_pins.is_empty(),
        };
        let general = UIGeneralSettings {
            gpu_acceleration: if cfg.hardware_settings.gpu_accel_enabled {
                "Enabled".to_string()
            } else {
                String::new()
            },
            hardware_model: cfg.hardware_config.device_name.clone(),
            ..UIGeneralSettings::default()
        };
        Self {
            camera_settings,
            general,
            lighting,
            network_settings: cfg.network_config.clone(),
            atfl: serde_json::to_value(&cfg.atfl).unwrap_or_else(|_| json!({})),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIProgramSettings {
    pub log_level: i32,
}

/// Publishes real-time vision pipeline results to WebSocket UI subscribers (`UIDataPublisher`).
#[derive(Clone)]
pub struct UIDataPublisher {
    camera_unique_name: String,
    data_change_service: DataChangeService,
}

impl UIDataPublisher {
    pub fn new(
        camera_unique_name: impl Into<String>,
        data_change_service: DataChangeService,
    ) -> Self {
        Self {
            camera_unique_name: camera_unique_name.into(),
            data_change_service,
        }
    }
}

impl CVPipelineResultConsumer for UIDataPublisher {
    fn accept(&self, result: &PhotonPipelineResult, fps: f64) {
        let payload = json!({
            &self.camera_unique_name: {
                "fps": fps,
                "latency": result.metadata.latency_millis(),
                "targets": result.targets,
                "multitagResult": result.multitag_result,
            }
        });
        self.data_change_service
            .publish_event(DataChangeEvent::outgoing_ui("uiResults", payload, None));
    }
}
