use crate::geometry::{Quaternion, Rotation3d, Transform3d, Translation3d};
use crate::result::{PhotonPipelineMetadata, PhotonPipelineResult};
use crate::targets::{MultiTargetPNPResult, PhotonTrackedTarget, PnpResult, TargetCorner};
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufVector3 {
    #[prost(double, tag = "1")]
    pub x: f64,
    #[prost(double, tag = "2")]
    pub y: f64,
    #[prost(double, tag = "3")]
    pub z: f64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufQuaternion {
    #[prost(double, tag = "1")]
    pub w: f64,
    #[prost(double, tag = "2")]
    pub x: f64,
    #[prost(double, tag = "3")]
    pub y: f64,
    #[prost(double, tag = "4")]
    pub z: f64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufRotation3d {
    #[prost(message, optional, tag = "1")]
    pub q: Option<ProtobufQuaternion>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufTranslation3d {
    #[prost(double, tag = "1")]
    pub x: f64,
    #[prost(double, tag = "2")]
    pub y: f64,
    #[prost(double, tag = "3")]
    pub z: f64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufTransform3d {
    #[prost(message, optional, tag = "1")]
    pub translation: Option<ProtobufTranslation3d>,
    #[prost(message, optional, tag = "2")]
    pub rotation: Option<ProtobufRotation3d>,
}

impl From<&Transform3d> for ProtobufTransform3d {
    fn from(t: &Transform3d) -> Self {
        Self {
            translation: Some(ProtobufTranslation3d {
                x: t.translation.x,
                y: t.translation.y,
                z: t.translation.z,
            }),
            rotation: Some(ProtobufRotation3d {
                q: Some(ProtobufQuaternion {
                    w: t.rotation.quaternion.w,
                    x: t.rotation.quaternion.x,
                    y: t.rotation.quaternion.y,
                    z: t.rotation.quaternion.z,
                }),
            }),
        }
    }
}

impl From<ProtobufTransform3d> for Transform3d {
    fn from(p: ProtobufTransform3d) -> Self {
        let tr = p
            .translation
            .map(|t| Translation3d::new(t.x, t.y, t.z))
            .unwrap_or_default();
        let rot = p
            .rotation
            .and_then(|r| r.q)
            .map(|q| Rotation3d::new(Quaternion::new(q.w, q.x, q.y, q.z)))
            .unwrap_or_default();
        Self::new(tr, rot)
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufTargetCorner {
    #[prost(double, tag = "1")]
    pub x: f64,
    #[prost(double, tag = "2")]
    pub y: f64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufPnpResult {
    #[prost(message, optional, tag = "2")]
    pub best: Option<ProtobufTransform3d>,
    #[prost(double, tag = "3")]
    pub best_reproj_err: f64,
    #[prost(message, optional, tag = "4")]
    pub alt: Option<ProtobufTransform3d>,
    #[prost(double, optional, tag = "5")]
    pub alt_reproj_err: Option<f64>,
    #[prost(double, tag = "6")]
    pub ambiguity: f64,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufMultiTargetPnpResult {
    #[prost(message, optional, tag = "1")]
    pub estimated_pose: Option<ProtobufPnpResult>,
    #[prost(int32, repeated, tag = "2")]
    pub fiducial_ids_used: Vec<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufPhotonTrackedTarget {
    #[prost(double, tag = "1")]
    pub yaw: f64,
    #[prost(double, tag = "2")]
    pub pitch: f64,
    #[prost(double, tag = "3")]
    pub area: f64,
    #[prost(double, tag = "4")]
    pub skew: f64,
    #[prost(int32, tag = "5")]
    pub fiducial_id: i32,
    #[prost(message, optional, tag = "6")]
    pub best_camera_to_target: Option<ProtobufTransform3d>,
    #[prost(message, optional, tag = "7")]
    pub alt_camera_to_target: Option<ProtobufTransform3d>,
    #[prost(double, tag = "8")]
    pub pose_ambiguity: f64,
    #[prost(message, repeated, tag = "9")]
    pub min_area_rect_corners: Vec<ProtobufTargetCorner>,
    #[prost(message, repeated, tag = "10")]
    pub detected_corners: Vec<ProtobufTargetCorner>,
    #[prost(int32, tag = "11")]
    pub obj_detection_id: i32,
    #[prost(float, tag = "12")]
    pub obj_detection_conf: f32,
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufPhotonPipelineResult {
    #[prost(double, tag = "1")]
    pub latency_ms: f64,
    #[prost(message, repeated, tag = "2")]
    pub targets: Vec<ProtobufPhotonTrackedTarget>,
    #[prost(message, optional, tag = "3")]
    pub multi_target_result: Option<ProtobufMultiTargetPnpResult>,
    #[prost(int64, tag = "4")]
    pub sequence_id: i64,
    #[prost(int64, tag = "5")]
    pub capture_timestamp_nanos: i64,
    #[prost(int64, tag = "6")]
    pub nt_publish_timestamp_nanos: i64,
    #[prost(int64, tag = "7")]
    pub time_since_last_pong_nanos: i64,
}

impl From<&PhotonPipelineResult> for ProtobufPhotonPipelineResult {
    fn from(r: &PhotonPipelineResult) -> Self {
        Self {
            latency_ms: r.metadata.latency_millis(),
            targets: r
                .targets
                .iter()
                .map(|t| ProtobufPhotonTrackedTarget {
                    yaw: t.yaw,
                    pitch: t.pitch,
                    area: t.area,
                    skew: t.skew,
                    fiducial_id: t.fiducial_id,
                    best_camera_to_target: Some((&t.best_camera_to_target).into()),
                    alt_camera_to_target: Some((&t.alt_camera_to_target).into()),
                    pose_ambiguity: t.pose_ambiguity,
                    min_area_rect_corners: t
                        .min_area_rect_corners
                        .iter()
                        .map(|c| ProtobufTargetCorner { x: c.x, y: c.y })
                        .collect(),
                    detected_corners: t
                        .detected_corners
                        .iter()
                        .map(|c| ProtobufTargetCorner { x: c.x, y: c.y })
                        .collect(),
                    obj_detection_id: t.obj_detect_id,
                    obj_detection_conf: t.obj_detect_conf,
                })
                .collect(),
            multi_target_result: r
                .multitag_result
                .as_ref()
                .map(|m| ProtobufMultiTargetPnpResult {
                    estimated_pose: Some(ProtobufPnpResult {
                        best: Some((&m.estimated_pose.best).into()),
                        best_reproj_err: m.estimated_pose.best_reproj_err,
                        alt: Some((&m.estimated_pose.alt).into()),
                        alt_reproj_err: Some(m.estimated_pose.alt_reproj_err),
                        ambiguity: m.estimated_pose.ambiguity,
                    }),
                    fiducial_ids_used: m
                        .fiducial_ids_used
                        .iter()
                        .map(|&id| i32::from(id))
                        .collect(),
                }),
            sequence_id: r.metadata.sequence_id,
            capture_timestamp_nanos: r.metadata.capture_timestamp_micros * 1000,
            nt_publish_timestamp_nanos: r.metadata.publish_timestamp_micros * 1000,
            time_since_last_pong_nanos: r.metadata.time_since_last_pong * 1000,
        }
    }
}

impl From<ProtobufPhotonPipelineResult> for PhotonPipelineResult {
    fn from(p: ProtobufPhotonPipelineResult) -> Self {
        Self {
            metadata: PhotonPipelineMetadata {
                sequence_id: p.sequence_id,
                capture_timestamp_micros: p.capture_timestamp_nanos / 1000,
                publish_timestamp_micros: p.nt_publish_timestamp_nanos / 1000,
                time_since_last_pong: p.time_since_last_pong_nanos / 1000,
            },
            targets: p
                .targets
                .into_iter()
                .map(|t| PhotonTrackedTarget {
                    yaw: t.yaw,
                    pitch: t.pitch,
                    area: t.area,
                    skew: t.skew,
                    fiducial_id: t.fiducial_id,
                    obj_detect_id: t.obj_detection_id,
                    obj_detect_conf: t.obj_detection_conf,
                    best_camera_to_target: t
                        .best_camera_to_target
                        .map(Into::into)
                        .unwrap_or_default(),
                    alt_camera_to_target: t
                        .alt_camera_to_target
                        .map(Into::into)
                        .unwrap_or_default(),
                    pose_ambiguity: t.pose_ambiguity,
                    min_area_rect_corners: t
                        .min_area_rect_corners
                        .into_iter()
                        .map(|c| TargetCorner::new(c.x, c.y))
                        .collect(),
                    detected_corners: t
                        .detected_corners
                        .into_iter()
                        .map(|c| TargetCorner::new(c.x, c.y))
                        .collect(),
                })
                .collect(),
            multitag_result: p.multi_target_result.map(|m| MultiTargetPNPResult {
                estimated_pose: m
                    .estimated_pose
                    .map(|ep| PnpResult {
                        best: ep.best.map(Into::into).unwrap_or_default(),
                        best_reproj_err: ep.best_reproj_err,
                        alt: ep.alt.map(Into::into).unwrap_or_default(),
                        alt_reproj_err: ep.alt_reproj_err.unwrap_or(0.0),
                        ambiguity: ep.ambiguity,
                    })
                    .unwrap_or_default(),
                fiducial_ids_used: m
                    .fiducial_ids_used
                    .into_iter()
                    .map(|id| id as i16)
                    .collect(),
            }),
        }
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct ProtobufDeviceMetrics {
    #[prost(double, tag = "1")]
    pub cpu_temp: f64,
    #[prost(double, tag = "2")]
    pub cpu_util: f64,
    #[prost(string, tag = "3")]
    pub cpu_thr: String,
    #[prost(double, tag = "4")]
    pub ram_mem: f64,
    #[prost(double, tag = "5")]
    pub ram_util: f64,
    #[prost(double, tag = "6")]
    pub gpu_mem: f64,
    #[prost(double, tag = "7")]
    pub gpu_mem_util: f64,
    #[prost(double, tag = "8")]
    pub disk_util_pct: f64,
    #[prost(double, repeated, tag = "9")]
    pub npu_usage: Vec<f64>,
    #[prost(string, tag = "10")]
    pub ip_address: String,
    #[prost(double, tag = "11")]
    pub uptime: f64,
    #[prost(double, tag = "12")]
    pub sent_bit_rate: f64,
    #[prost(double, tag = "13")]
    pub recv_bit_rate: f64,
    #[prost(double, tag = "14")]
    pub disk_usable_space: f64,
}
