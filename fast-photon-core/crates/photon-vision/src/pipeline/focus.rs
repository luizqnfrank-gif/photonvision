use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};

pub type FocusPipelineSettings = CVPipelineSettings;

/// Camera lens focus measurement pipeline (`org.photonvision.vision.pipeline.FocusPipeline`).
#[derive(Debug, Default)]
pub struct FocusPipeline {
    settings: CVPipelineSettings,
    rotate_pipe: RotateImagePipe,
    focus_pipe: FocusPipe,
    fps_pipe: CalculateFPSPipe,
}

impl FocusPipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::Focus;
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl CVPipeline for FocusPipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::Focus
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
        settings.pipeline_type = PipelineType::Focus;
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

        let focus_res = self.focus_pipe.run(&frame.color_image);
        profiler.record("FocusPipe", focus_res.elapsed_nanos);
        let (out_mat, score) = focus_res.output;
        frame.processed_image = out_mat;

        let fps = self.fps_pipe.run(()).output;
        let mut res = CVPipelineResult::new(
            frame.sequence_id,
            profiler.total_nanos(),
            fps,
            Vec::new(),
            frame,
        );
        res.focus_score = Some(score);
        res
    }
}
