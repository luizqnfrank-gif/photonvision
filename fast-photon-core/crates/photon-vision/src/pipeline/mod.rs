pub mod apriltag;
pub mod aruco;
pub mod calibrate3d;
pub mod colored_shape;
pub mod driver_mode;
pub mod focus;
pub mod object_detection;
pub mod output_stream;
pub mod reflective;

pub use apriltag::*;
pub use aruco::*;
pub use calibrate3d::*;
pub use colored_shape::*;
pub use driver_mode::*;
pub use focus::*;
pub use object_detection::*;
pub use output_stream::*;
pub use reflective::*;

use crate::calibration::CameraCalibrationCoefficients;
use crate::frame::{Frame, FrameThresholdType};
use crate::target::TrackedTarget;
pub use photon_common::configuration::{
    AprilTagFieldLayout, CVPipelineSettings, PipelineSettingsVariant, PipelineType, QuirkyCamera,
};
use photon_common::dataflow::networktables::now_micros;
use photon_targeting::{MultiTargetPNPResult, PhotonPipelineMetadata, PhotonPipelineResult};

/// Tracks nanosecond execution profiles across pipe stages (`org.photonvision.vision.pipeline.PipelineProfiler`).
#[derive(Debug, Clone, Default)]
pub struct PipelineProfiler {
    pub pipe_timings_nanos: Vec<(&'static str, u64)>,
}

impl PipelineProfiler {
    pub fn record(&mut self, name: &'static str, nanos: u64) {
        self.pipe_timings_nanos.push((name, nanos));
    }

    pub fn total_nanos(&self) -> u64 {
        self.pipe_timings_nanos.iter().map(|(_, n)| *n).sum()
    }

    pub fn total_millis(&self) -> f64 {
        self.total_nanos() as f64 / 1_000_000.0
    }
}

/// Output of a `CVPipeline` run (`org.photonvision.vision.pipeline.result.CVPipelineResult`).
#[derive(Debug, Clone)]
pub struct CVPipelineResult {
    pub sequence_id: i64,
    pub processing_nanos: u64,
    pub fps: i32,
    pub targets: Vec<TrackedTarget>,
    pub multi_tag_result: Option<MultiTargetPNPResult>,
    pub input_and_output_frame: Frame,
    pub image_capture_timestamp_nanos: i64,
    pub focus_score: Option<f64>,
    pub calibration_result: Option<CameraCalibrationCoefficients>,
    pub object_detection_class_names: Vec<String>,
}

impl CVPipelineResult {
    pub fn new(
        sequence_id: i64,
        processing_nanos: u64,
        fps: i32,
        targets: Vec<TrackedTarget>,
        input_and_output_frame: Frame,
    ) -> Self {
        let ts = input_and_output_frame.timestamp_nanos;
        Self {
            sequence_id,
            processing_nanos,
            fps,
            targets,
            multi_tag_result: None,
            input_and_output_frame,
            image_capture_timestamp_nanos: ts,
            focus_score: None,
            calibration_result: None,
            object_detection_class_names: Vec::new(),
        }
    }

    pub fn has_targets(&self) -> bool {
        !self.targets.is_empty()
    }

    pub fn latency_millis(&self) -> f64 {
        self.processing_nanos as f64 / 1_000_000.0
    }

    pub fn to_photon_pipeline_result(&self, time_since_last_pong_us: i64) -> PhotonPipelineResult {
        let capture_us = self.image_capture_timestamp_nanos / 1000;
        let publish_us = now_micros().max(capture_us + (self.processing_nanos as i64 / 1000));
        PhotonPipelineResult {
            metadata: PhotonPipelineMetadata {
                sequence_id: self.sequence_id,
                capture_timestamp_micros: capture_us,
                publish_timestamp_micros: publish_us,
                time_since_last_pong: time_since_last_pong_us,
            },
            targets: self
                .targets
                .iter()
                .map(TrackedTarget::to_photon_tracked_target)
                .collect(),
            multitag_result: self.multi_tag_result.clone(),
        }
    }
}

pub type DriverModePipelineResult = CVPipelineResult;
pub type FocusPipelineResult = CVPipelineResult;
pub type CalibrationPipelineResult = CVPipelineResult;

/// Polymorphic vision pipeline trait (`org.photonvision.vision.pipeline.CVPipeline`).
pub trait CVPipeline: Send + Sync {
    fn pipeline_type(&self) -> PipelineType;
    fn threshold_type(&self) -> FrameThresholdType;
    fn settings(&self) -> &CVPipelineSettings;
    fn settings_mut(&mut self) -> &mut CVPipelineSettings;
    fn set_settings(&mut self, settings: CVPipelineSettings);
    fn set_atfl(&mut self, _atfl: AprilTagFieldLayout) {}
    fn run(&mut self, frame: Frame, quirks: &QuirkyCamera) -> CVPipelineResult;
}
