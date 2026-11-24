use crate::frame::{Frame, FrameThresholdType};
use crate::objects::ObjectDetector;
use crate::pipe::*;
use crate::pipeline::{CVPipeline, CVPipelineResult, PipelineProfiler};
use crate::target::{PotentialTarget, TargetCalculationParameters};
use photon_common::configuration::{CVPipelineSettings, PipelineType, QuirkyCamera};

pub type ObjectDetectionPipelineSettings = CVPipelineSettings;

/// Neural network object detection pipeline (`org.photonvision.vision.pipeline.ObjectDetectionPipeline`).
pub struct ObjectDetectionPipeline {
    settings: CVPipelineSettings,
    rotate_pipe: RotateImagePipe,
    detect_pipe: ObjectDetectionPipe,
    filter_pipe: FilterObjectDetectionsPipe,
    sort_pipe: SortContoursPipe,
    collect_2d_pipe: Collect2dTargetsPipe,
    fps_pipe: CalculateFPSPipe,
    class_names: Vec<String>,
}

impl Default for ObjectDetectionPipeline {
    fn default() -> Self {
        let settings = CVPipelineSettings {
            pipeline_type: PipelineType::ObjectDetection,
            ..CVPipelineSettings::default()
        };
        Self {
            settings,
            rotate_pipe: RotateImagePipe::default(),
            detect_pipe: ObjectDetectionPipe::default(),
            filter_pipe: FilterObjectDetectionsPipe::default(),
            sort_pipe: SortContoursPipe::default(),
            collect_2d_pipe: Collect2dTargetsPipe::default(),
            fps_pipe: CalculateFPSPipe::default(),
            class_names: Vec::new(),
        }
    }
}

impl ObjectDetectionPipeline {
    pub fn new(mut settings: CVPipelineSettings) -> Self {
        settings.pipeline_type = PipelineType::ObjectDetection;
        Self {
            settings,
            ..Self::default()
        }
    }

    pub fn set_detector(&mut self, detector: Box<dyn ObjectDetector>) {
        self.class_names = detector.classes().to_vec();
        self.detect_pipe.set_detector(detector);
    }
}

impl CVPipeline for ObjectDetectionPipeline {
    fn pipeline_type(&self) -> PipelineType {
        PipelineType::ObjectDetection
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
        settings.pipeline_type = PipelineType::ObjectDetection;
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

        self.detect_pipe.set_params(ObjectDetectionParams {
            confidence: self.settings.confidence,
            nms: self.settings.nms,
            max_detections: self.settings.output_maximum_targets.max(1) as usize,
        });
        let raw_dets = self.detect_pipe.run(&frame.color_image);
        profiler.record("ObjectDetection", raw_dets.elapsed_nanos);

        self.filter_pipe.set_params(FilterObjectDetectionsParams {
            area_min: self.settings.contour_area.first,
            area_max: self.settings.contour_area.second,
            ratio_min: self.settings.contour_ratio.first,
            ratio_max: self.settings.contour_ratio.second,
            frame_properties: (*frame.static_properties).clone(),
            is_landscape: true,
        });
        let filtered = self.filter_pipe.run(raw_dets.output);
        profiler.record("FilterObjectDetections", filtered.elapsed_nanos);

        let potentials: Vec<PotentialTarget> = filtered
            .output
            .into_iter()
            .map(|d| {
                PotentialTarget::from_object_detection(d.bbox_corners, d.class_id, d.confidence)
            })
            .collect();

        self.sort_pipe.set_params(SortContoursParams {
            sort_mode: self.settings.contour_sort_mode,
            max_targets: self.settings.output_maximum_targets.max(1) as usize,
            frame_properties: (*frame.static_properties).clone(),
        });
        let sorted = self.sort_pipe.run(potentials);
        profiler.record("SortContours", sorted.elapsed_nanos);

        self.collect_2d_pipe.set_params(Collect2dTargetsParams {
            calc_params: TargetCalculationParameters::from_frame_props(&frame.static_properties),
        });
        let targets = self.collect_2d_pipe.run(sorted.output);
        profiler.record("Collect2dTargets", targets.elapsed_nanos);

        let fps = self.fps_pipe.run(()).output;
        let mut res = CVPipelineResult::new(
            frame.sequence_id,
            profiler.total_nanos(),
            fps,
            targets.output,
            frame,
        );
        res.object_detection_class_names = self.class_names.clone();
        res
    }
}
