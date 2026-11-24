pub mod driverstation;
pub mod timesync;

pub use driverstation::*;
pub use timesync::*;

use crate::configuration::NetworkConfig;
use crate::dataflow::CVPipelineResultConsumer;
use photon_targeting::proto::{ProtobufDeviceMetrics, ProtobufPhotonPipelineResult};
use photon_targeting::{Packet, PacketSerde, PhotonPipelineResult, Transform3d};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NtTopicValue {
    Boolean(bool),
    Integer(i64),
    Double(f64),
    String(String),
    DoubleArray(Vec<f64>),
    RawBytes { type_string: String, bytes: Vec<u8> },
}

/// Per-camera NetworkTables topic paths and cached values (`org.photonvision.common.networktables.NTTopicSet`).
#[derive(Debug, Clone)]
pub struct NTTopicSet {
    pub subtable_path: String,
    pub topics: HashMap<String, NtTopicValue>,
}

impl NTTopicSet {
    pub fn new(camera_nickname: &str) -> Self {
        let subtable_path = format!("/photonvision/{camera_nickname}");
        let mut s = Self {
            subtable_path,
            topics: HashMap::new(),
        };
        s.init_defaults();
        s
    }

    pub fn topic_path(&self, leaf: &str) -> String {
        format!("{}/{leaf}", self.subtable_path)
    }

    pub fn init_defaults(&mut self) {
        self.topics.insert(
            "rawBytes".to_string(),
            NtTopicValue::RawBytes {
                type_string: PhotonPipelineResult::type_string(),
                bytes: Vec::new(),
            },
        );
        self.topics.insert(
            "result_proto".to_string(),
            NtTopicValue::RawBytes {
                type_string: "protobuf:photonvision.proto.ProtobufPhotonPipelineResult".to_string(),
                bytes: Vec::new(),
            },
        );
        self.topics
            .insert("pipelineIndexState".to_string(), NtTopicValue::Integer(0));
        self.topics
            .insert("pipelineIndexRequest".to_string(), NtTopicValue::Integer(0));
        self.topics
            .insert("driverMode".to_string(), NtTopicValue::Boolean(false));
        self.topics.insert(
            "driverModeRequest".to_string(),
            NtTopicValue::Boolean(false),
        );
        self.topics
            .insert("fpsLimit".to_string(), NtTopicValue::Integer(-1));
        self.topics
            .insert("fpsLimitRequest".to_string(), NtTopicValue::Integer(-1));
        self.topics
            .insert("enabled".to_string(), NtTopicValue::Boolean(true));
        self.topics
            .insert("enabledRequest".to_string(), NtTopicValue::Boolean(true));
        self.topics
            .insert("latencyMillis".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("fps".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("hasTarget".to_string(), NtTopicValue::Boolean(false));
        self.topics
            .insert("targetPitch".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("targetYaw".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("targetArea".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("targetSkew".to_string(), NtTopicValue::Double(0.0));
        self.topics.insert(
            "targetPose".to_string(),
            NtTopicValue::RawBytes {
                type_string: "struct:Transform3d".to_string(),
                bytes: Transform3d::default().to_struct_bytes().to_vec(),
            },
        );
        self.topics
            .insert("targetPixelsX".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("targetPixelsY".to_string(), NtTopicValue::Double(0.0));
        self.topics
            .insert("heartbeat".to_string(), NtTopicValue::Integer(0));
        self.topics.insert(
            "cameraIntrinsics".to_string(),
            NtTopicValue::DoubleArray(Vec::new()),
        );
        self.topics.insert(
            "cameraDistortion".to_string(),
            NtTopicValue::DoubleArray(Vec::new()),
        );
    }
}

/// Publishes `PhotonPipelineResult` to NetworkTables v4 (`org.photonvision.common.dataflow.networktables.NTDataPublisher`).
#[derive(Clone)]
pub struct NTDataPublisher {
    camera_nickname: Arc<RwLock<String>>,
    topic_set: Arc<RwLock<NTTopicSet>>,
    should_publish_proto: Arc<RwLock<bool>>,
    pipeline_index_callback: Option<Arc<dyn Fn(i32) + Send + Sync>>,
    driver_mode_callback: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    fps_limit_callback: Option<Arc<dyn Fn(i32) + Send + Sync>>,
    enabled_callback: Option<Arc<dyn Fn(bool) + Send + Sync>>,
}

impl NTDataPublisher {
    pub fn new(camera_nickname: impl Into<String>, should_publish_proto: bool) -> Self {
        let name = camera_nickname.into();
        let ts = NTTopicSet::new(&name);
        Self {
            camera_nickname: Arc::new(RwLock::new(name)),
            topic_set: Arc::new(RwLock::new(ts)),
            should_publish_proto: Arc::new(RwLock::new(should_publish_proto)),
            pipeline_index_callback: None,
            driver_mode_callback: None,
            fps_limit_callback: None,
            enabled_callback: None,
        }
    }

    pub fn with_callbacks(
        mut self,
        on_pipeline_index: impl Fn(i32) + Send + Sync + 'static,
        on_driver_mode: impl Fn(bool) + Send + Sync + 'static,
        on_fps_limit: impl Fn(i32) + Send + Sync + 'static,
        on_enabled: impl Fn(bool) + Send + Sync + 'static,
    ) -> Self {
        self.pipeline_index_callback = Some(Arc::new(on_pipeline_index));
        self.driver_mode_callback = Some(Arc::new(on_driver_mode));
        self.fps_limit_callback = Some(Arc::new(on_fps_limit));
        self.enabled_callback = Some(Arc::new(on_enabled));
        self
    }

    pub fn update_camera_nickname(&self, new_nickname: impl Into<String>) {
        let name = new_nickname.into();
        if let Ok(mut n) = self.camera_nickname.write() {
            *n = name.clone();
        }
        if let Ok(mut ts) = self.topic_set.write() {
            ts.subtable_path = format!("/photonvision/{name}");
        }
    }

    pub fn get_topic_set(&self) -> NTTopicSet {
        self.topic_set
            .read()
            .map(|g| g.clone())
            .unwrap_or_else(|_| NTTopicSet::new("unknown"))
    }

    pub fn handle_pipeline_index_request(&self, new_index: i32) {
        if new_index < 0 {
            return;
        }
        if let Some(cb) = &self.pipeline_index_callback {
            cb(new_index);
        }
        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics.insert(
                "pipelineIndexState".to_string(),
                NtTopicValue::Integer(i64::from(new_index)),
            );
        }
    }

    pub fn handle_driver_mode_request(&self, driver_mode: bool) {
        if let Some(cb) = &self.driver_mode_callback {
            cb(driver_mode);
        }
        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics
                .insert("driverMode".to_string(), NtTopicValue::Boolean(driver_mode));
        }
    }

    pub fn handle_fps_limit_request(&self, fps_limit: i32) {
        if let Some(cb) = &self.fps_limit_callback {
            cb(fps_limit);
        }
        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics.insert(
                "fpsLimit".to_string(),
                NtTopicValue::Integer(i64::from(fps_limit)),
            );
        }
    }

    pub fn handle_enabled_request(&self, enabled: bool) {
        if let Some(cb) = &self.enabled_callback {
            cb(enabled);
        }
        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics
                .insert("enabled".to_string(), NtTopicValue::Boolean(enabled));
        }
    }

    pub fn set_calibration_coefficients(&self, intrinsics: Vec<f64>, distortion: Vec<f64>) {
        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics.insert(
                "cameraIntrinsics".to_string(),
                NtTopicValue::DoubleArray(intrinsics),
            );
            ts.topics.insert(
                "cameraDistortion".to_string(),
                NtTopicValue::DoubleArray(distortion),
            );
        }
    }
}

impl CVPipelineResultConsumer for NTDataPublisher {
    fn accept(&self, result: &PhotonPipelineResult, fps: f64) {
        let mut pkt = Packet::with_capacity(256);
        if result.pack(&mut pkt).is_err() {
            return;
        }
        let publish_proto = self.should_publish_proto.read().map(|g| *g).unwrap_or(true);

        if let Ok(mut ts) = self.topic_set.write() {
            ts.topics.insert(
                "rawBytes".to_string(),
                NtTopicValue::RawBytes {
                    type_string: PhotonPipelineResult::type_string(),
                    bytes: pkt.into_bytes(),
                },
            );

            if publish_proto {
                let proto_msg = ProtobufPhotonPipelineResult::from(result);
                ts.topics.insert(
                    "result_proto".to_string(),
                    NtTopicValue::RawBytes {
                        type_string: "protobuf:photonvision.proto.ProtobufPhotonPipelineResult"
                            .to_string(),
                        bytes: proto_msg.encode_to_vec(),
                    },
                );
            }

            ts.topics.insert(
                "latencyMillis".to_string(),
                NtTopicValue::Double(result.metadata.latency_millis()),
            );
            ts.topics
                .insert("fps".to_string(), NtTopicValue::Double(fps));
            ts.topics.insert(
                "hasTarget".to_string(),
                NtTopicValue::Boolean(result.has_targets()),
            );
            ts.topics.insert(
                "heartbeat".to_string(),
                NtTopicValue::Integer(result.metadata.sequence_id),
            );

            if let Some(best) = result.best_target() {
                ts.topics
                    .insert("targetPitch".to_string(), NtTopicValue::Double(best.pitch));
                ts.topics
                    .insert("targetYaw".to_string(), NtTopicValue::Double(best.yaw));
                ts.topics
                    .insert("targetArea".to_string(), NtTopicValue::Double(best.area));
                ts.topics
                    .insert("targetSkew".to_string(), NtTopicValue::Double(best.skew));
                ts.topics.insert(
                    "targetPose".to_string(),
                    NtTopicValue::RawBytes {
                        type_string: "struct:Transform3d".to_string(),
                        bytes: best.best_camera_to_target.to_struct_bytes().to_vec(),
                    },
                );
                if let Some(c0) = best.min_area_rect_corners.first() {
                    ts.topics
                        .insert("targetPixelsX".to_string(), NtTopicValue::Double(c0.x));
                    ts.topics
                        .insert("targetPixelsY".to_string(), NtTopicValue::Double(c0.y));
                }
            } else {
                ts.topics
                    .insert("targetPitch".to_string(), NtTopicValue::Double(0.0));
                ts.topics
                    .insert("targetYaw".to_string(), NtTopicValue::Double(0.0));
                ts.topics
                    .insert("targetArea".to_string(), NtTopicValue::Double(0.0));
                ts.topics
                    .insert("targetSkew".to_string(), NtTopicValue::Double(0.0));
                ts.topics.insert(
                    "targetPose".to_string(),
                    NtTopicValue::RawBytes {
                        type_string: "struct:Transform3d".to_string(),
                        bytes: Transform3d::default().to_struct_bytes().to_vec(),
                    },
                );
            }
        }
    }
}

/// Global NetworkTables manager (`org.photonvision.common.dataflow.networktables.NetworkTablesManager`).
#[derive(Clone)]
pub struct NetworkTablesManager {
    pub root_table_name: String,
    pub time_sync: TimeSyncManager,
    pub driver_station: Arc<RwLock<NTDriverStation>>,
    global_topics: Arc<RwLock<HashMap<String, NtTopicValue>>>,
}

impl Default for NetworkTablesManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkTablesManager {
    pub const ROOT_TABLE_NAME: &'static str = "/photonvision";

    pub fn new() -> Self {
        let mut globals = HashMap::new();
        globals.insert(
            "/photonvision/version".to_string(),
            NtTopicValue::String("2027.0.0-rust".to_string()),
        );
        globals.insert(
            "/photonvision/ledMode".to_string(),
            NtTopicValue::Integer(0),
        );
        globals.insert(
            "/photonvision/ledModeRequest".to_string(),
            NtTopicValue::Integer(0),
        );
        Self {
            root_table_name: Self::ROOT_TABLE_NAME.to_string(),
            time_sync: TimeSyncManager::default(),
            driver_station: Arc::new(RwLock::new(NTDriverStation::default())),
            global_topics: Arc::new(RwLock::new(globals)),
        }
    }

    pub fn set_config(&mut self, config: &NetworkConfig) {
        self.time_sync =
            TimeSyncManager::new(config.nt_server_address.clone(), 5810, config.run_nt_server);
    }

    pub fn broadcast_metrics(&self, metrics: &ProtobufDeviceMetrics) {
        if let Ok(mut globals) = self.global_topics.write() {
            globals.insert(
                "/photonvision/metrics".to_string(),
                NtTopicValue::RawBytes {
                    type_string: "protobuf:photonvision.proto.ProtobufDeviceMetrics".to_string(),
                    bytes: metrics.encode_to_vec(),
                },
            );
        }
    }

    pub fn get_global_topics(&self) -> HashMap<String, NtTopicValue> {
        self.global_topics
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}
