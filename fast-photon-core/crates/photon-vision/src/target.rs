use crate::frame::FrameStaticProperties;
use crate::opencv_ext::{CVShape, Contour, DualOffsetValues};
pub use photon_common::configuration::RobotOffsetPointMode;
use photon_targeting::{PhotonTrackedTarget, TargetCorner, Transform3d};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RobotOffsetPointOperation {
    #[default]
    CLEAR = 0,
    TAKEFIRSTDUAL = 1,
    TAKESECONDDUAL = 2,
    TAKESINGLE = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TargetOffsetPointEdge {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TargetOrientation {
    #[default]
    Portrait,
    Landscape,
}

/// Candidate target prior to 2D/3D metric calculation (`org.photonvision.vision.target.PotentialTarget`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PotentialTarget {
    pub main_contour: Contour,
    pub sub_contours: Vec<Contour>,
    pub shape: Option<CVShape>,
    pub detected_corners: Vec<TargetCorner>,
    pub cls_id: i32,
    pub confidence: f32,
}

impl PotentialTarget {
    pub fn from_contour(main_contour: Contour) -> Self {
        let detected_corners = main_contour.min_area_rect_corners.clone();
        Self {
            main_contour,
            sub_contours: Vec::new(),
            shape: None,
            detected_corners,
            cls_id: -1,
            confidence: -1.0,
        }
    }

    pub fn from_contour_group(contours: Vec<Contour>) -> Option<Self> {
        let combined = Contour::combine(&contours)?;
        let detected_corners = combined.min_area_rect_corners.clone();
        Some(Self {
            main_contour: combined,
            sub_contours: contours,
            shape: None,
            detected_corners,
            cls_id: -1,
            confidence: -1.0,
        })
    }

    pub fn from_shape(shape: CVShape) -> Self {
        let main_contour = shape.contour.clone();
        let detected_corners = shape.corners.clone();
        Self {
            main_contour,
            sub_contours: Vec::new(),
            shape: Some(shape),
            detected_corners,
            cls_id: -1,
            confidence: -1.0,
        }
    }

    pub fn from_object_detection(
        bbox_corners: Vec<TargetCorner>,
        cls_id: i32,
        confidence: f32,
    ) -> Self {
        let main_contour = Contour::from_points(bbox_corners.clone());
        Self {
            main_contour,
            sub_contours: Vec::new(),
            shape: None,
            detected_corners: bbox_corners,
            cls_id,
            confidence,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetCalculationParameters {
    pub is_landscape: bool,
    pub target_offset_point_edge: TargetOffsetPointEdge,
    pub robot_offset_point_mode: RobotOffsetPointMode,
    pub robot_offset_single_point: TargetCorner,
    pub dual_offset_values: DualOffsetValues,
    pub camera_center_point: TargetCorner,
    pub horizontal_focal_length: f64,
    pub vertical_focal_length: f64,
    pub image_area: f64,
}

impl TargetCalculationParameters {
    pub fn from_frame_props(props: &FrameStaticProperties) -> Self {
        Self {
            is_landscape: true,
            target_offset_point_edge: TargetOffsetPointEdge::Center,
            robot_offset_point_mode: RobotOffsetPointMode::None,
            robot_offset_single_point: TargetCorner::default(),
            dual_offset_values: DualOffsetValues::default(),
            camera_center_point: TargetCorner::new(props.center_x, props.center_y),
            horizontal_focal_length: props.horizontal_focal_length,
            vertical_focal_length: props.vertical_focal_length,
            image_area: props.image_area,
        }
    }
}

/// Pinhole yaw/pitch/aspect/offset calculations (`org.photonvision.vision.target.TargetCalculations`).
pub struct TargetCalculations;

impl TargetCalculations {
    /// Calculates `(yaw_deg, pitch_deg)` using pinhole lens projection matching `TargetCalculations.calculateYawPitch`:
    /// +Yaw is target to the right of center (`(tx - cx) / fx`), +Pitch is target above center (`(cy - ty) / fy`).
    pub fn calculate_yaw_pitch(
        camera_center_x: f64,
        target_center_x: f64,
        horizontal_focal_length: f64,
        camera_center_y: f64,
        target_center_y: f64,
        vertical_focal_length: f64,
    ) -> (f64, f64) {
        let fx = horizontal_focal_length.max(1e-6);
        let fy = vertical_focal_length.max(1e-6);
        let yaw_rad = ((target_center_x - camera_center_x) / fx).atan();
        let pitch_rad = ((camera_center_y - target_center_y) / fy).atan();
        (yaw_rad.to_degrees(), pitch_rad.to_degrees())
    }

    pub fn calculate_aspect_ratio(width: f64, height: f64, is_landscape: bool) -> f64 {
        if width <= 0.0 || height <= 0.0 {
            return 0.0;
        }
        let (long_side, short_side) = if width >= height {
            (width, height)
        } else {
            (height, width)
        };
        if is_landscape {
            long_side / short_side
        } else {
            short_side / long_side
        }
    }

    pub fn calculate_target_offset_point(
        contour: &Contour,
        edge: TargetOffsetPointEdge,
    ) -> TargetCorner {
        let c = contour.center;
        let hw = contour.bounding_width / 2.0;
        let hh = contour.bounding_height / 2.0;
        match edge {
            TargetOffsetPointEdge::Center => c,
            TargetOffsetPointEdge::Top => TargetCorner::new(c.x, c.y - hh),
            TargetOffsetPointEdge::Bottom => TargetCorner::new(c.x, c.y + hh),
            TargetOffsetPointEdge::Left => TargetCorner::new(c.x - hw, c.y),
            TargetOffsetPointEdge::Right => TargetCorner::new(c.x + hw, c.y),
        }
    }

    pub fn calculate_robot_offset_point(
        target_area: f64,
        camera_center: TargetCorner,
        mode: RobotOffsetPointMode,
        single_point: TargetCorner,
        dual_values: &DualOffsetValues,
    ) -> TargetCorner {
        match mode {
            RobotOffsetPointMode::None => camera_center,
            RobotOffsetPointMode::Single => {
                if single_point.x == 0.0 && single_point.y == 0.0 {
                    camera_center
                } else {
                    single_point
                }
            }
            RobotOffsetPointMode::Dual => {
                let a1 = dual_values.first_point_area;
                let a2 = dual_values.second_point_area;
                if (a2 - a1).abs() < 1e-6 {
                    return dual_values.first_point;
                }
                let t = ((target_area - a1) / (a2 - a1)).clamp(0.0, 1.0);
                let x = photon_common::util::MathUtils::lerp(
                    dual_values.first_point.x,
                    dual_values.second_point.x,
                    t,
                );
                let y = photon_common::util::MathUtils::lerp(
                    dual_values.first_point.y,
                    dual_values.second_point.y,
                    t,
                );
                TargetCorner::new(x, y)
            }
        }
    }
}

/// Internal vision target (`org.photonvision.vision.target.TrackedTarget`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackedTarget {
    pub main_contour: Contour,
    pub sub_contours: Vec<Contour>,
    pub target_corners: Vec<TargetCorner>,
    pub target_offset_point: TargetCorner,
    pub robot_offset_point: TargetCorner,
    pub pitch: f64,
    pub yaw: f64,
    pub area: f64,
    pub skew: f64,
    pub best_camera_to_target_3d: Transform3d,
    pub alt_camera_to_target_3d: Transform3d,
    pub shape: Option<CVShape>,
    pub fiducial_id: i32,
    pub pose_ambiguity: f64,
    pub class_id: i32,
    pub confidence: f32,
}

impl TrackedTarget {
    pub fn from_potential_target(
        orig: PotentialTarget,
        params: &TargetCalculationParameters,
    ) -> Self {
        let target_offset_point = TargetCalculations::calculate_target_offset_point(
            &orig.main_contour,
            params.target_offset_point_edge,
        );
        let area = (orig.main_contour.area / params.image_area.max(1.0)) * 100.0;
        let robot_offset_point = TargetCalculations::calculate_robot_offset_point(
            area,
            params.camera_center_point,
            params.robot_offset_point_mode,
            params.robot_offset_single_point,
            &params.dual_offset_values,
        );
        let (yaw, pitch) = TargetCalculations::calculate_yaw_pitch(
            robot_offset_point.x,
            target_offset_point.x,
            params.horizontal_focal_length,
            robot_offset_point.y,
            target_offset_point.y,
            params.vertical_focal_length,
        );

        Self {
            main_contour: orig.main_contour,
            sub_contours: orig.sub_contours,
            target_corners: orig.detected_corners,
            target_offset_point,
            robot_offset_point,
            pitch,
            yaw,
            area,
            skew: 0.0,
            best_camera_to_target_3d: Transform3d::default(),
            alt_camera_to_target_3d: Transform3d::default(),
            shape: orig.shape,
            fiducial_id: -1,
            pose_ambiguity: -1.0,
            class_id: orig.cls_id,
            confidence: orig.confidence,
        }
    }

    pub fn from_fiducial(
        fiducial_id: i32,
        corners: Vec<TargetCorner>,
        best_pose: Transform3d,
        alt_pose: Transform3d,
        ambiguity: f64,
        params: &TargetCalculationParameters,
    ) -> Self {
        let contour = Contour::from_points(corners.clone());
        let center = contour.center;
        let area = (contour.area / params.image_area.max(1.0)) * 100.0;
        let (yaw, pitch) = TargetCalculations::calculate_yaw_pitch(
            params.camera_center_point.x,
            center.x,
            params.horizontal_focal_length,
            params.camera_center_point.y,
            center.y,
            params.vertical_focal_length,
        );
        Self {
            main_contour: contour,
            sub_contours: Vec::new(),
            target_corners: corners,
            target_offset_point: center,
            robot_offset_point: params.camera_center_point,
            pitch,
            yaw,
            area,
            skew: 0.0,
            best_camera_to_target_3d: best_pose,
            alt_camera_to_target_3d: alt_pose,
            shape: None,
            fiducial_id,
            pose_ambiguity: ambiguity,
            class_id: -1,
            confidence: -1.0,
        }
    }

    pub fn to_photon_tracked_target(&self) -> PhotonTrackedTarget {
        let min_rect = if self.main_contour.min_area_rect_corners.len() == 4 {
            self.main_contour.min_area_rect_corners.clone()
        } else {
            vec![TargetCorner::default(); 4]
        };
        PhotonTrackedTarget {
            yaw: self.yaw,
            pitch: self.pitch,
            area: self.area,
            skew: self.skew,
            fiducial_id: self.fiducial_id,
            obj_detect_id: self.class_id,
            obj_detect_conf: self.confidence,
            best_camera_to_target: self.best_camera_to_target_3d,
            alt_camera_to_target: self.alt_camera_to_target_3d,
            pose_ambiguity: self.pose_ambiguity,
            min_area_rect_corners: min_rect,
            detected_corners: self.target_corners.clone(),
        }
    }
}
