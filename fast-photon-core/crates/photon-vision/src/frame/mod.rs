pub mod consumer;
pub mod provider;

pub use consumer::*;
pub use provider::*;

use crate::opencv_ext::CVMat;
use photon_common::configuration::CameraCalibrationCoefficients;
pub use photon_common::configuration::FrameDivisor;
use photon_common::dataflow::networktables::now_nanos;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FrameThresholdType {
    #[default]
    NONE,
    HSV,
    GREYSCALE,
    ROBOT,
}

/// Immutable per-resolution camera and lens properties (`org.photonvision.vision.frame.FrameStaticProperties`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameStaticProperties {
    pub image_width: i32,
    pub image_height: i32,
    pub fov: f64,
    pub image_area: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub horizontal_focal_length: f64,
    pub vertical_focal_length: f64,
    pub camera_calibration: Option<CameraCalibrationCoefficients>,
}

impl Default for FrameStaticProperties {
    fn default() -> Self {
        Self::new(640, 480, 70.0, None)
    }
}

impl FrameStaticProperties {
    pub fn new(
        image_width: i32,
        image_height: i32,
        fov_deg: f64,
        camera_calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let w = f64::from(image_width.max(1));
        let h = f64::from(image_height.max(1));
        let image_area = w * h;

        let (center_x, center_y, fx, fy) = if let Some(cal) = &camera_calibration {
            if cal.camera_intrinsics.data.len() >= 9 {
                (cal.cx(), cal.cy(), cal.fx(), cal.fy())
            } else {
                Self::compute_pinhole_from_fov(w, h, fov_deg)
            }
        } else {
            Self::compute_pinhole_from_fov(w, h, fov_deg)
        };

        Self {
            image_width,
            image_height,
            fov: fov_deg,
            image_area,
            center_x,
            center_y,
            horizontal_focal_length: fx,
            vertical_focal_length: fy,
            camera_calibration,
        }
    }

    fn compute_pinhole_from_fov(w: f64, h: f64, fov_deg: f64) -> (f64, f64, f64, f64) {
        let diag = w.hypot(h);
        let fov_rad = fov_deg.to_radians().clamp(0.1, 3.0);
        let focal = diag / (2.0 * (fov_rad / 2.0).tan());
        (w / 2.0 - 0.5, h / 2.0 - 0.5, focal, focal)
    }
}

/// Captured and processed camera frame (`org.photonvision.vision.frame.Frame`).
#[derive(Debug, Clone)]
pub struct Frame {
    pub sequence_id: i64,
    pub color_image: CVMat,
    pub processed_image: CVMat,
    pub threshold_type: FrameThresholdType,
    pub timestamp_nanos: i64,
    pub static_properties: Arc<FrameStaticProperties>,
}

impl Frame {
    pub fn new(
        sequence_id: i64,
        color_image: CVMat,
        processed_image: CVMat,
        threshold_type: FrameThresholdType,
        static_properties: Arc<FrameStaticProperties>,
    ) -> Self {
        Self {
            sequence_id,
            color_image,
            processed_image,
            threshold_type,
            timestamp_nanos: now_nanos(),
            static_properties,
        }
    }

    pub fn from_single_mat(
        sequence_id: i64,
        mat: CVMat,
        static_properties: Arc<FrameStaticProperties>,
    ) -> Self {
        let copy = mat.clone();
        Self::new(
            sequence_id,
            mat,
            copy,
            FrameThresholdType::NONE,
            static_properties,
        )
    }
}

pub trait FrameProvider: Send + Sync {
    fn get_frame(&mut self) -> Frame;
    fn name(&self) -> &str;
    fn is_connected(&self) -> bool;
}
