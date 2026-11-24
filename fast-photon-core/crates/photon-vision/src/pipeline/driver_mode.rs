use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};
use photon_targeting::TargetCorner;

/// Low-latency driver mode camera streaming pipeline (`org.photonvision.vision.pipeline.DriverModePipeline`).
#[derive(Debug, Default)]
pub struct DriverModePipeline {
    settings: CVPipelineSettings,
    rotate_pipe: RotateImagePipe,
    crosshair_pipe: Draw2dCrosshairPipe,
    fps_pipe: CalculateFPSPipe,
}

impl DriverModePipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::DriverMode;
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl CVPipeline for DriverModePipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::DriverMode
    }

    fn threshold_type(&self) -> FrameThresholdType {
        FrameThresholdType::NONE
    }

    fn settings(&self) -> &CVPipelineSettings {
        &self.settings
    }

    fn settings_mut(&mut self) -> &mut CVPipelineSettings {
        &mut self.settings
    }

    fn set_settings(&mut self, mut settings: CVPipelineSettings) {
        settings.pipeline_type = PipelineType::DriverMode;
        self.settings = settings;
    }

    fn run(&mut self, mut frame: Frame, _quirks: &QuirkyCamera) -> CVPipelineResult {
        let mut profiler = PipelineProfiler::default();
        self.rotate_pipe.set_params(RotateImageParams {
            rotation: self.settings.input_image_rotation_mode,
            flip: self.settings.input_image_flip_mode,
        });
        profiler.record(
            "RotateImage",
            self.rotate_pipe.run_in_place(&mut frame.color_image),
        );
        frame.processed_image = frame.color_image.clone();

        self.crosshair_pipe.set_params(Draw2dCrosshairParams {
            should_draw: true,
            center: TargetCorner::new(
                frame.static_properties.center_x,
                frame.static_properties.center_y,
            ),
        });
        profiler.record(
            "Draw2dCrosshair",
            self.crosshair_pipe.run_in_place(&mut frame.processed_image),
        );

        let fps = self.fps_pipe.run(()).output;
        CVPipelineResult::new(
            frame.sequence_id,
            profiler.total_nanos(),
            fps,
            Vec::new(),
            frame,
        )
    }
}
