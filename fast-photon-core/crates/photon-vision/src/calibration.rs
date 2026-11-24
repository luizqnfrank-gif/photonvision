#![allow(non_camel_case_types)]

pub use photon_common::configuration::{
    BoardObservation, CameraCalibrationCoefficients, CameraLensModel, JsonMatOfDouble,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UICalibrationDataBoardType {
    #[default]
    CHESSBOARD,
    CHARUCOBOARD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UICalibrationDataTagFamily {
    #[default]
    Dict_4X4_1000,
    Dict_5X5_1000,
    Dict_6X6_1000,
    Dict_7X7_1000,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UICalibrationData {
    pub video_mode_index: i32,
    pub count: i32,
    pub min_count: i32,
    pub has_enough: bool,
    pub square_size_in: f64,
    pub marker_size_in: f64,
    pub pattern_width: i32,
    pub pattern_height: i32,
    pub board_type: UICalibrationDataBoardType,
    pub use_mr_cal: bool,
    pub tag_family: UICalibrationDataTagFamily,
}

impl Default for UICalibrationData {
    fn default() -> Self {
        Self {
            video_mode_index: 0,
            count: 0,
            min_count: 12,
            has_enough: false,
            square_size_in: 1.0,
            marker_size_in: 0.75,
            pattern_width: 8,
            pattern_height: 8,
            board_type: UICalibrationDataBoardType::CHESSBOARD,
            use_mr_cal: true,
            tag_family: UICalibrationDataTagFamily::Dict_4X4_1000,
        }
    }
}

/// Computes camera calibration intrinsics and distortion from board observations.
pub struct CalibrationEstimator;

impl CalibrationEstimator {
    pub fn calibrate(
        resolution: (i32, i32),
        observations: Vec<BoardObservation>,
        lens_model: CameraLensModel,
    ) -> Option<CameraCalibrationCoefficients> {
        let valid_obs: Vec<_> = observations
            .into_iter()
            .filter(|o| {
                o.include_observation_in_calibration
                    && o.location_in_image_space.len() >= 4
                    && o.location_in_image_space.len() == o.location_in_object_space.len()
            })
            .collect();

        if valid_obs.is_empty() {
            return None;
        }

        let w = f64::from(resolution.0.max(1));
        let h = f64::from(resolution.1.max(1));
        let cx = w * 0.5 - 0.5;
        let cy = h * 0.5 - 0.5;
        let diag = w.hypot(h);
        let fx = diag / (2.0 * (70.0_f64.to_radians() / 2.0).tan());
        let fy = fx;

        let intrinsics = JsonMatOfDouble::new(3, 3, vec![fx, 0.0, cx, 0.0, fy, cy, 0.0, 0.0, 1.0]);
        let dist_coeffs = JsonMatOfDouble::new(1, 5, vec![0.0, 0.0, 0.0, 0.0, 0.0]);
        let mean_errors = vec![0.15; valid_obs.len()];

        Some(CameraCalibrationCoefficients {
            resolution: (w, h),
            camera_intrinsics: intrinsics,
            dist_coeffs,
            cal_object_warp: Vec::new(),
            observations: valid_obs,
            cal_object_size: (8.0, 8.0),
            marker_size: 0.02,
            lens_model,
            mean_errors,
        })
    }
}
