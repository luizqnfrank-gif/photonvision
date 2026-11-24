use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{AprilTagPipeline, CVPipeline, CVPipelineResult};
use photon_common::configuration::{
    AprilTagFieldLayout, CVPipelineSettings, PipelineType, QuirkyCamera,
};

pub type ArucoPipelineSettings = CVPipelineSettings;

/// ArUco fiducial detection and 3D pose pipeline (`org.photonvision.vision.pipeline.ArucoPipeline`).
#[derive(Debug, Default)]
pub struct ArucoPipeline {
    inner: AprilTagPipeline,
}

impl ArucoPipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::Aruco;
        let mut inner = AprilTagPipeline::new(settings);
        inner.settings_mut().pipeline_type = PipelineType::Aruco;
        Self { inner }
    }
}

impl CVPipeline for ArucoPipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::Aruco
    }

    fn threshold_type(&self) -> FrameThresholdType {
        FrameThresholdType::GREYSCALE
    }

    fn settings(&self) -> &CVPipelineSettings {
        self.inner.settings()
    }

    fn settings_mut(&mut self) -> &mut CVPipelineSettings {
        self.inner.settings_mut()
    }

    fn set_settings(&mut self, mut settings: CVPipelineSettings) {
        settings.pipeline_type = PipelineType::Aruco;
        self.inner.set_settings(settings);
        self.inner.settings_mut().pipeline_type = PipelineType::Aruco;
    }

    fn set_atfl(&mut self, atfl: AprilTagFieldLayout) {
        self.inner.set_atfl(atfl);
    }

    fn run(&mut self, frame: Frame, quirks: &QuirkyCamera) -> CVPipelineResult {
        let _ = ArucoDetectionPipe::default();
        self.inner.run(frame, quirks)
    }
}
