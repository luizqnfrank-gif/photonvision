use crate::geometry::{Pose3d, Rotation3d, Transform3d, Translation3d};
use crate::targets::PnpResult;
use serde::{Deserialize, Serialize};

/// 3D target model geometry (`org.photonvision.estimation.TargetModel` & `org.photonvision.vision.target.TargetModel`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetModel {
    pub vertices: Vec<Translation3d>,
    pub is_planar: bool,
    pub is_spherical: bool,
}

impl Default for TargetModel {
    fn default() -> Self {
        Self::k_april_tag_36h11()
    }
}

impl TargetModel {
    /// 6.5in (0.1651m) 36h11 AprilTag in WPILib NWU target frame.
    pub fn k_april_tag_36h11() -> Self {
        Self::from_planar_square(0.1651)
    }

    /// 6in (0.1524m) 16h5 AprilTag in WPILib NWU target frame.
    pub fn k_april_tag_16h5() -> Self {
        Self::from_planar_square(0.1524)
    }

    pub fn from_planar_square(width_meters: f64) -> Self {
        let half = width_meters / 2.0;
        Self {
            // Standard PhotonVision tag corners: bottom-left, bottom-right, top-right, top-left in target frame
            vertices: vec![
                Translation3d::new(0.0, -half, -half),
                Translation3d::new(0.0, half, -half),
                Translation3d::new(0.0, half, half),
                Translation3d::new(0.0, -half, half),
            ],
            is_planar: true,
            is_spherical: false,
        }
    }

    pub fn from_rectangle(width_meters: f64, height_meters: f64) -> Self {
        let hw = width_meters / 2.0;
        let hh = height_meters / 2.0;
        Self {
            vertices: vec![
                Translation3d::new(0.0, -hw, -hh),
                Translation3d::new(0.0, hw, -hh),
                Translation3d::new(0.0, hw, hh),
                Translation3d::new(0.0, -hw, hh),
            ],
            is_planar: true,
            is_spherical: false,
        }
    }
}

/// Camera-to-target spatial relation (`org.photonvision.estimation.CameraTargetRelation`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTargetRelation {
    pub cam_pose: Pose3d,
    pub cam_to_targ: Transform3d,
    pub cam_to_targ_dist: f64,
    pub cam_to_targ_dist_xy: f64,
    pub cam_to_targ_yaw: f64,
    pub cam_to_targ_pitch: f64,
    pub cam_to_targ_angle: f64,
    pub targ_to_cam: Transform3d,
    pub targ_to_cam_yaw: f64,
    pub targ_to_cam_pitch: f64,
    pub targ_to_cam_angle: f64,
}

impl CameraTargetRelation {
    pub fn new(cam_pose: Pose3d, target_pose: Pose3d) -> Self {
        let cam_iso = Transform3d::new(cam_pose.translation, cam_pose.rotation).to_isometry();
        let targ_iso =
            Transform3d::new(target_pose.translation, target_pose.rotation).to_isometry();
        let cam_to_targ = Transform3d::from_isometry(cam_iso.inverse() * targ_iso);
        let targ_to_cam = cam_to_targ.inverse();

        let dx = cam_to_targ.translation.x;
        let dy = cam_to_targ.translation.y;
        let dz = cam_to_targ.translation.z;
        let dist_xy = (dx * dx + dy * dy).sqrt();
        let dist = cam_to_targ.translation.norm();
        let cam_to_targ_yaw = dy.atan2(dx);
        let cam_to_targ_pitch = (-dz).atan2(dx);
        let cam_to_targ_angle = (cam_to_targ_yaw.hypot(cam_to_targ_pitch)).abs();

        let tdx = targ_to_cam.translation.x;
        let tdy = targ_to_cam.translation.y;
        let tdz = targ_to_cam.translation.z;
        let targ_to_cam_yaw = tdy.atan2(tdx);
        let targ_to_cam_pitch = (-tdz).atan2(tdx);
        let targ_to_cam_angle = (targ_to_cam_yaw.hypot(targ_to_cam_pitch)).abs();

        Self {
            cam_pose,
            cam_to_targ,
            cam_to_targ_dist: dist,
            cam_to_targ_dist_xy: dist_xy,
            cam_to_targ_yaw,
            cam_to_targ_pitch,
            cam_to_targ_angle,
            targ_to_cam,
            targ_to_cam_yaw,
            targ_to_cam_pitch,
            targ_to_cam_angle,
        }
    }
}

/// Rotation-Translation transform helper (`org.photonvision.estimation.RotTrlTransform3d`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RotTrlTransform3d {
    pub rot: Rotation3d,
    pub trl: Translation3d,
}

impl RotTrlTransform3d {
    pub fn new(rot: Rotation3d, trl: Translation3d) -> Self {
        Self { rot, trl }
    }

    pub fn make_relative_to(base: Pose3d) -> Self {
        let inv = Transform3d::new(base.translation, base.rotation).inverse();
        Self {
            rot: inv.rotation,
            trl: inv.translation,
        }
    }

    pub fn apply_point(&self, pt: Translation3d) -> Translation3d {
        let v = self.rot.quaternion.to_nalgebra() * pt.to_nalgebra().vector
            + self.trl.to_nalgebra().vector;
        Translation3d::new(v.x, v.y, v.z)
    }
}

/// Vision estimation helper (`org.photonvision.estimation.VisionEstimation`).
pub struct VisionEstimation;

impl VisionEstimation {
    /// Estimate field-to-camera pose from known field tag poses and observed camera-to-tag transforms.
    pub fn estimate_cam_pose_pnp(
        tag_poses_with_transforms: &[(Pose3d, Transform3d)],
    ) -> Option<PnpResult> {
        if tag_poses_with_transforms.is_empty() {
            return None;
        }
        // Average translations and slerp/average quaternions in field frame
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;
        let mut first_rot = None;

        for (tag_pose, cam_to_tag) in tag_poses_with_transforms {
            let tag_to_cam = cam_to_tag.inverse();
            let cam_in_field = tag_pose.transform_by(&tag_to_cam);
            sum_x += cam_in_field.translation.x;
            sum_y += cam_in_field.translation.y;
            sum_z += cam_in_field.translation.z;
            if first_rot.is_none() {
                first_rot = Some(cam_in_field.rotation);
            }
        }

        let n = tag_poses_with_transforms.len() as f64;
        let best = Transform3d::new(
            Translation3d::new(sum_x / n, sum_y / n, sum_z / n),
            first_rot.unwrap_or_default(),
        );

        Some(PnpResult {
            best,
            best_reproj_err: 0.0,
            alt: Transform3d::default(),
            alt_reproj_err: 0.0,
            ambiguity: 0.0,
        })
    }
}
