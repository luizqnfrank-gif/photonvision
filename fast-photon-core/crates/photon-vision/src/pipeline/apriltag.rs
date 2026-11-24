use crate::apriltag_aruco::AprilTagFamily;
use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use crate::target::{TargetCalculationParameters, TrackedTarget};
use photon_common::configuration::{
    AprilTagFieldLayout, CVPipelineSettings, PipelineType, QuirkyCamera,
};
use photon_targeting::{TargetModel, Transform3d};

pub type AprilTagPipelineSettings = CVPipelineSettings;

/// AprilTag detection and single/multi-tag 3D localization pipeline (`org.photonvision.vision.pipeline.AprilTagPipeline`).
#[derive(Debug, Default)]
pub struct AprilTagPipeline {
    settings: CVPipelineSettings,
    atfl: AprilTagFieldLayout,
    rotate_pipe: RotateImagePipe,
    grayscale_pipe: GrayscalePipe,
    detect_pipe: AprilTagDetectionPipe,
    pose_pipe: AprilTagPoseEstimatorPipe,
    multitag_pipe: MultiTargetPNPPipe,
    fps_pipe: CalculateFPSPipe,
}

impl AprilTagPipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::AprilTag;
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl CVPipeline for AprilTagPipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::AprilTag
    }

    fn threshold_type(&self) -> FrameThresholdType {
        FrameThresholdType::GREYSCALE
    }

    fn settings(&self) -> &CVPipelineSettings {
        &self.settings
    }

    fn settings_mut(&mut self) -> &mut CVPipelineSettings {
        &mut self.settings
    }

    fn set_settings(&mut self, mut settings: CVPipelineSettings) {
        settings.pipeline_type = PipelineType::AprilTag;
        self.settings = settings;
    }

    fn set_atfl(&mut self, atfl: AprilTagFieldLayout) {
        self.atfl = atfl;
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

        let gray_res = self.grayscale_pipe.run(&frame.color_image);
        profiler.record("Grayscale", gray_res.elapsed_nanos);
        frame.processed_image = gray_res.output;
        frame.threshold_type = FrameThresholdType::GREYSCALE;

        self.detect_pipe.set_params(AprilTagDetectionParams {
            family: self.settings.tag_family,
            decimate: self.settings.decimate,
            blur: self.settings.blur,
            threads: self.settings.threads,
            refine_edges: self.settings.refine_edges,
            decision_margin: self.settings.decision_margin,
            hamming_dist: self.settings.hamming_dist,
        });
        let det_res = self.detect_pipe.run(&frame.processed_image);
        profiler.record("AprilTagDetection", det_res.elapsed_nanos);

        let tag_model = match self.settings.tag_family {
            AprilTagFamily::kTag36h11 => TargetModel::k_april_tag_36h11(),
            AprilTagFamily::kTag16h5 => TargetModel::k_april_tag_16h5(),
        };

        self.pose_pipe.set_params(AprilTagPoseEstimatorParams {
            calibration: frame.static_properties.camera_calibration.clone(),
            tag_model: tag_model.clone(),
            n_iterations: self.settings.num_iterations,
        });

        let calc_params = TargetCalculationParameters::from_frame_props(&frame.static_properties);
        let mut targets = Vec::new();

        for det in det_res.output {
            if det.hamming > self.settings.hamming_dist
                || (det.decision_margin as i32) < self.settings.decision_margin
            {
                continue;
            }
            let (best_pose, alt_pose, ambiguity) = if self.settings.solve_pnp_enabled {
                if let Some(pnp) = self.pose_pipe.run(&det).output {
                    (pnp.best, pnp.alt, pnp.ambiguity)
                } else {
                    (Transform3d::default(), Transform3d::default(), -1.0)
                }
            } else {
                (Transform3d::default(), Transform3d::default(), -1.0)
            };

            targets.push(TrackedTarget::from_fiducial(
                det.id,
                det.corners.to_vec(),
                best_pose,
                alt_pose,
                ambiguity,
                &calc_params,
            ));
        }

        let multi_tag_result = if self.settings.solve_pnp_enabled && self.settings.do_multi_target {
            self.multitag_pipe.set_params(MultiTargetPNPParams {
                calibration: frame.static_properties.camera_calibration.clone(),
                atfl: self.atfl.clone(),
                target_model: tag_model,
            });
            let mt_res = self.multitag_pipe.run(&targets);
            profiler.record("MultiTargetPNP", mt_res.elapsed_nanos);
            mt_res.output
        } else {
            None
        };

        let fps = self.fps_pipe.run(()).output;
        let mut result = CVPipelineResult::new(
            frame.sequence_id,
            profiler.total_nanos(),
            fps,
            targets,
            frame,
        );
        result.multi_tag_result = multi_tag_result;
        result
    }
}
