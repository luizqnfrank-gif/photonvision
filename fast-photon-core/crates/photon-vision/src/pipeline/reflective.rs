use crate::frame::{Frame, FrameThresholdType};
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use crate::target::TargetCalculationParameters;
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};

pub type ReflectivePipelineSettings = CVPipelineSettings;

/// HSV + contour reflective tape pipeline (`org.photonvision.vision.pipeline.ReflectivePipeline`).
#[derive(Debug, Default)]
pub struct ReflectivePipeline {
    settings: CVPipelineSettings,
    rotate_pipe: RotateImagePipe,
    hsv_pipe: HSVPipe,
    find_contours_pipe: FindContoursPipe,
    speckle_reject_pipe: SpeckleRejectPipe,
    filter_contours_pipe: FilterContoursPipe,
    group_contours_pipe: GroupContoursPipe,
    sort_contours_pipe: SortContoursPipe,
    collect_2d_targets_pipe: Collect2dTargetsPipe,
    corner_detection_pipe: CornerDetectionPipe,
    solve_pnp_pipe: SolvePNPPipe,
    fps_pipe: CalculateFPSPipe,
}

impl ReflectivePipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::Reflective;
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl CVPipeline for ReflectivePipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::Reflective
    }

    fn threshold_type(&self) -> FrameThresholdType {
        FrameThresholdType::HSV
    }

    fn settings(&self) -> &CVPipelineSettings {
        &self.settings
    }

    fn settings_mut(&mut self) -> &mut CVPipelineSettings {
        &mut self.settings
    }

    fn set_settings(&mut self, mut settings: CVPipelineSettings) {
        settings.pipeline_type = PipelineType::Reflective;
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

        self.hsv_pipe.set_params(HSVParams {
            hue: self.settings.hsv_hue,
            saturation: self.settings.hsv_saturation,
            value: self.settings.hsv_value,
            hue_inverted: self.settings.hue_inverted,
        });
        let hsv_res = self.hsv_pipe.run(&frame.color_image);
        profiler.record("HSV", hsv_res.elapsed_nanos);
        frame.processed_image = hsv_res.output;
        frame.threshold_type = FrameThresholdType::HSV;

        let contours_res = self.find_contours_pipe.run(&frame.processed_image);
        profiler.record("FindContours", contours_res.elapsed_nanos);

        self.speckle_reject_pipe.set_params(SpeckleRejectParams {
            min_percent_of_avg: f64::from(self.settings.contour_speckle_percentage),
        });
        let speckle_res = self.speckle_reject_pipe.run(contours_res.output);
        profiler.record("SpeckleReject", speckle_res.elapsed_nanos);

        self.filter_contours_pipe.set_params(FilterContoursParams {
            area: self.settings.contour_area,
            ratio: self.settings.contour_ratio,
            fullness: self.settings.contour_fullness,
            frame_properties: (*frame.static_properties).clone(),
            is_landscape: true,
        });
        let filter_res = self.filter_contours_pipe.run(speckle_res.output);
        profiler.record("FilterContours", filter_res.elapsed_nanos);

        self.group_contours_pipe.set_params(GroupContoursParams {
            group: self.settings.contour_grouping_mode,
            intersection: self.settings.contour_intersection,
        });
        let group_res = self.group_contours_pipe.run(filter_res.output);
        profiler.record("GroupContours", group_res.elapsed_nanos);

        self.sort_contours_pipe.set_params(SortContoursParams {
            sort_mode: self.settings.contour_sort_mode,
            max_targets: self.settings.output_maximum_targets.max(1) as usize,
            frame_properties: (*frame.static_properties).clone(),
        });
        let sort_res = self.sort_contours_pipe.run(group_res.output);
        profiler.record("SortContours", sort_res.elapsed_nanos);

        let corners_res = self.corner_detection_pipe.run(sort_res.output);
        profiler.record("CornerDetection", corners_res.elapsed_nanos);

        self.collect_2d_targets_pipe
            .set_params(Collect2dTargetsParams {
                calc_params: TargetCalculationParameters::from_frame_props(
                    &frame.static_properties,
                ),
            });
        let targets_2d = self.collect_2d_targets_pipe.run(corners_res.output);
        profiler.record("Collect2dTargets", targets_2d.elapsed_nanos);

        let final_targets = if self.settings.solve_pnp_enabled {
            self.solve_pnp_pipe.set_params(SolvePNPParams {
                calibration: frame.static_properties.camera_calibration.clone(),
                target_model: self.settings.target_model.clone(),
            });
            let pnp_res = self.solve_pnp_pipe.run(targets_2d.output);
            profiler.record("SolvePNP", pnp_res.elapsed_nanos);
            pnp_res.output
        } else {
            targets_2d.output
        };

        let fps = self.fps_pipe.run(()).output;
        CVPipelineResult::new(
            frame.sequence_id,
            profiler.total_nanos(),
            fps,
            final_targets,
            frame,
        )
    }
}
