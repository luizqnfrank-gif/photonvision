use crate::calibration::{BoardObservation, CameraCalibrationCoefficients, CameraLensModel};
use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};

pub type Calibration3dPipelineSettings = CVPipelineSettings;

/// 3D chessboard/ChArUco camera calibration pipeline (`org.photonvision.vision.pipeline.Calibrate3dPipeline`).
#[derive(Debug, Default)]
pub struct Calibrate3dPipeline {
    settings: CVPipelineSettings,
    find_board_pipe: FindBoardCornersPipe,
    calibrate_pipe: Calibrate3dPipe,
    draw_cal_pipe: DrawCalibrationPipe,
    fps_pipe: CalculateFPSPipe,
    observations: Vec<BoardObservation>,
    take_snapshot_requested: bool,
}

impl Calibrate3dPipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::Calibrate3d;
        Self {
            settings,
            ..Self::default()
        }
    }

    pub fn take_snapshot(&mut self) {
        self.take_snapshot_requested = true;
    }

    pub fn observations(&self) -> &[BoardObservation] {
        &self.observations
    }

    pub fn finish_calibration(
        &mut self,
        resolution: (i32, i32),
        lens_model: CameraLensModel,
        use_mr_cal: bool,
    ) -> Option<CameraCalibrationCoefficients> {
        self.calibrate_pipe.set_params(Calibrate3dParams {
            resolution,
            lens_model,
            use_mr_cal,
        });
        let obs = std::mem::take(&mut self.observations);
        self.calibrate_pipe.run(obs).output
    }
}

impl CVPipeline for Calibrate3dPipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::Calibrate3d
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
        settings.pipeline_type = PipelineType::Calibrate3d;
        self.settings = settings;
    }

    fn run(&mut self, mut frame: Frame, _quirks: &QuirkyCamera) -> CVPipelineResult {
        let mut profiler = PipelineProfiler::default();
        let board_res = self.find_board_pipe.run(&frame.color_image);
        profiler.record("FindBoardCorners", board_res.elapsed_nanos);

        if self.take_snapshot_requested {
            self.take_snapshot_requested = false;
            if let Some(obs) = &board_res.output {
                self.observations.push(obs.clone());
            }
        }

        self.draw_cal_pipe
            .set_params(DrawCalibrationParams { should_draw: true });
        let mut draw_tuple = (frame.processed_image, board_res.output);
        profiler.record(
            "DrawCalibration",
            self.draw_cal_pipe.run_in_place(&mut draw_tuple),
        );
        frame.processed_image = draw_tuple.0;

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
