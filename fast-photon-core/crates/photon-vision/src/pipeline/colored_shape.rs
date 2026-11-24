use crate::frame::{Frame, FrameThresholdType};
use crate::opencv_ext::ContourShape;
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use crate::target::{PotentialTarget, TargetCalculationParameters};
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};

pub type ColoredShapePipelineSettings = CVPipelineSettings;

/// Colored geometric shape detection pipeline (`org.photonvision.vision.pipeline.ColoredShapePipeline`).
#[derive(Debug, Default)]
pub struct ColoredShapePipeline {
    settings: CVPipelineSettings,
    rotate_pipe: RotateImagePipe,
    hsv_pipe: HSVPipe,
    find_contours_pipe: FindContoursPipe,
    speckle_reject_pipe: SpeckleRejectPipe,
    filter_contours_pipe: FilterContoursPipe,
    find_polygons_pipe: FindPolygonPipe,
    find_circles_pipe: FindCirclesPipe,
    filter_shapes_pipe: FilterShapesPipe,
    sort_contours_pipe: SortContoursPipe,
    collect_2d_targets_pipe: Collect2dTargetsPipe,
    solve_pnp_pipe: SolvePNPPipe,
    fps_pipe: CalculateFPSPipe,
}

impl ColoredShapePipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::ColoredShape;
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl CVPipeline for ColoredShapePipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::ColoredShape
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
        settings.pipeline_type = PipelineType::ColoredShape;
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

        let shapes = if self.settings.contour_shape == ContourShape::Circle {
            let circ_res = self
                .find_circles_pipe
                .run((&frame.processed_image, filter_res.output));
            profiler.record("FindCircles", circ_res.elapsed_nanos);
            circ_res.output
        } else {
            self.find_polygons_pipe.set_params(FindPolygonParams {
                accuracy_percentage: self.settings.accuracy_percentage,
            });
            let poly_res = self.find_polygons_pipe.run(filter_res.output);
            profiler.record("FindPolygons", poly_res.elapsed_nanos);
            poly_res.output
        };

        self.filter_shapes_pipe.set_params(FilterShapesParams {
            desired_shape: self.settings.contour_shape,
            min_area: self.settings.contour_area.first,
            max_area: self.settings.contour_area.second,
        });
        let filtered_shapes = self.filter_shapes_pipe.run(shapes);
        profiler.record("FilterShapes", filtered_shapes.elapsed_nanos);

        let potentials: Vec<PotentialTarget> = filtered_shapes
            .output
            .into_iter()
            .map(PotentialTarget::from_shape)
            .collect();

        self.sort_contours_pipe.set_params(SortContoursParams {
            sort_mode: self.settings.contour_sort_mode,
            max_targets: self.settings.output_maximum_targets.max(1) as usize,
            frame_properties: (*frame.static_properties).clone(),
        });
        let sorted = self.sort_contours_pipe.run(potentials);
        profiler.record("SortContours", sorted.elapsed_nanos);

        self.collect_2d_targets_pipe
            .set_params(Collect2dTargetsParams {
                calc_params: TargetCalculationParameters::from_frame_props(
                    &frame.static_properties,
                ),
            });
        let targets_2d = self.collect_2d_targets_pipe.run(sorted.output);
        profiler.record("Collect2dTargets", targets_2d.elapsed_nanos);

        let final_targets = if self.settings.solve_pnp_enabled {
            self.solve_pnp_pipe.set_params(SolvePNPParams {
                calibration: frame.static_properties.camera_calibration.clone(),
                target_model: self.settings.target_model.clone(),
            });
            self.solve_pnp_pipe.run(targets_2d.output).output
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
