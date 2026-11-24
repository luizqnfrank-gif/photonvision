use crate::geometry::Transform3d;
use crate::packet::{Packet, PacketResult, PacketSerde};
use serde::{Deserialize, Serialize};

/// 2D corner coordinate in pixels (`org.photonvision.targeting.TargetCorner`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct TargetCorner {
    pub x: f64,
    pub y: f64,
}

impl TargetCorner {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl PacketSerde for TargetCorner {
    const INTERFACE_UUID: &'static str = "16f6ac0dedc8eaccb951f4895d9e18b6";
    const SCHEMA: &'static str = "float64 x;float64 y;";
    const TYPE_NAME: &'static str = "TargetCorner";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        packet.encode_f64(self.x);
        packet.encode_f64(self.y);
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            x: packet.decode_f64()?,
            y: packet.decode_f64()?,
        })
    }
}

/// 3D PnP solution (`org.photonvision.targeting.PnpResult`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PnpResult {
    pub best: Transform3d,
    pub best_reproj_err: f64,
    pub alt: Transform3d,
    pub alt_reproj_err: f64,
    pub ambiguity: f64,
}

impl PacketSerde for PnpResult {
    const INTERFACE_UUID: &'static str = "ae4d655c0a3104dff6634d304ef3a1d3";
    const SCHEMA: &'static str =
        "Transform3d best;float64 bestReprojErr;Transform3d alt;float64 altReprojErr;float64 ambiguity;";
    const TYPE_NAME: &'static str = "PnpResult";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        self.best.pack(packet);
        packet.encode_f64(self.best_reproj_err);
        self.alt.pack(packet);
        packet.encode_f64(self.alt_reproj_err);
        packet.encode_f64(self.ambiguity);
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            best: Transform3d::unpack(packet)?,
            best_reproj_err: packet.decode_f64()?,
            alt: Transform3d::unpack(packet)?,
            alt_reproj_err: packet.decode_f64()?,
            ambiguity: packet.decode_f64()?,
        })
    }
}

/// Multi-tag PnP field-to-camera result (`org.photonvision.targeting.MultiTargetPNPResult`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MultiTargetPNPResult {
    pub estimated_pose: PnpResult,
    pub fiducial_ids_used: Vec<i16>,
}

impl PacketSerde for MultiTargetPNPResult {
    const INTERFACE_UUID: &'static str = "541096947e9f3ca2d3f425ff7b04aa7b";
    const SCHEMA: &'static str =
        "PnpResult:ae4d655c0a3104dff6634d304ef3a1d3 estimatedPose;int16 fiducialIDsUsed[?];";
    const TYPE_NAME: &'static str = "MultiTargetPNPResult";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        self.estimated_pose.pack(packet)?;
        packet.encode_i16_list(&self.fiducial_ids_used)?;
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            estimated_pose: PnpResult::unpack(packet)?,
            fiducial_ids_used: packet.decode_i16_list()?,
        })
    }
}

/// Wire target model (`org.photonvision.targeting.PhotonTrackedTarget`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhotonTrackedTarget {
    pub yaw: f64,
    pub pitch: f64,
    pub area: f64,
    pub skew: f64,
    pub fiducial_id: i32,
    pub obj_detect_id: i32,
    pub obj_detect_conf: f32,
    pub best_camera_to_target: Transform3d,
    pub alt_camera_to_target: Transform3d,
    pub pose_ambiguity: f64,
    pub min_area_rect_corners: Vec<TargetCorner>,
    pub detected_corners: Vec<TargetCorner>,
}

impl Default for PhotonTrackedTarget {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            area: 0.0,
            skew: 0.0,
            fiducial_id: -1,
            obj_detect_id: -1,
            obj_detect_conf: -1.0,
            best_camera_to_target: Transform3d::default(),
            alt_camera_to_target: Transform3d::default(),
            pose_ambiguity: -1.0,
            min_area_rect_corners: vec![TargetCorner::default(); 4],
            detected_corners: Vec::new(),
        }
    }
}

impl PacketSerde for PhotonTrackedTarget {
    const INTERFACE_UUID: &'static str = "cc6dbb5c5c1e0fa808108019b20863f1";
    const SCHEMA: &'static str = "float64 yaw;float64 pitch;float64 area;float64 skew;int32 fiducialId;int32 objDetectId;float32 objDetectConf;Transform3d bestCameraToTarget;Transform3d altCameraToTarget;float64 poseAmbiguity;TargetCorner:16f6ac0dedc8eaccb951f4895d9e18b6 minAreaRectCorners[?];TargetCorner:16f6ac0dedc8eaccb951f4895d9e18b6 detectedCorners[?];";
    const TYPE_NAME: &'static str = "PhotonTrackedTarget";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        packet.encode_f64(self.yaw);
        packet.encode_f64(self.pitch);
        packet.encode_f64(self.area);
        packet.encode_f64(self.skew);
        packet.encode_i32(self.fiducial_id);
        packet.encode_i32(self.obj_detect_id);
        packet.encode_f32(self.obj_detect_conf);
        self.best_camera_to_target.pack(packet);
        self.alt_camera_to_target.pack(packet);
        packet.encode_f64(self.pose_ambiguity);
        packet.encode_list(&self.min_area_rect_corners)?;
        packet.encode_list(&self.detected_corners)?;
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            yaw: packet.decode_f64()?,
            pitch: packet.decode_f64()?,
            area: packet.decode_f64()?,
            skew: packet.decode_f64()?,
            fiducial_id: packet.decode_i32()?,
            obj_detect_id: packet.decode_i32()?,
            obj_detect_conf: packet.decode_f32()?,
            best_camera_to_target: Transform3d::unpack(packet)?,
            alt_camera_to_target: Transform3d::unpack(packet)?,
            pose_ambiguity: packet.decode_f64()?,
            min_area_rect_corners: packet.decode_list()?,
            detected_corners: packet.decode_list()?,
        })
    }
}
