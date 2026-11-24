use fast_photon_core::common::configuration::{
    AprilTagFieldEntry, AprilTagFieldLayout, CVPipelineSettings, CameraCalibrationCoefficients,
    CameraLensModel, ContourShape, FieldDimensions, JsonMatOfDouble, PipelineType, QuirkyCamera,
};
use fast_photon_core::common::util::{DoubleCouple, IntegerCouple};
use fast_photon_core::targeting::{Pose3d, Rotation3d, Translation3d};
use fast_photon_core::vision::frame::{FileFrameProvider, FrameProvider};
use fast_photon_core::vision::objects::{NeuralNetworkPipeResult, ObjectDetector};
use fast_photon_core::vision::opencv_ext::CVMat;
use fast_photon_core::vision::pipeline::{
    AprilTagPipeline, ArucoPipeline, CVPipeline, Calibrate3dPipeline, ColoredShapePipeline,
    DriverModePipeline, FocusPipeline, ObjectDetectionPipeline, OutputStreamPipeline,
    ReflectivePipeline,
};
use image::{DynamicImage, Rgb, RgbImage};

fn make_calibrated_coefficients() -> CameraCalibrationCoefficients {
    CameraCalibrationCoefficients {
        resolution: (640.0, 480.0),
        camera_intrinsics: JsonMatOfDouble::new(
            3,
            3,
            vec![500.0, 0.0, 319.5, 0.0, 500.0, 239.5, 0.0, 0.0, 1.0],
        ),
        dist_coeffs: JsonMatOfDouble::new(1, 5, vec![0.0; 5]),
        cal_object_warp: Vec::new(),
        observations: Vec::new(),
        cal_object_size: (8.0, 8.0),
        marker_size: 0.02,
        lens_model: CameraLensModel::LENSMODEL_OPENCV,
        mean_errors: vec![0.1],
    }
}

fn make_synthetic_green_target_mat() -> CVMat {
    let mut img = RgbImage::from_pixel(640, 480, Rgb([10u8, 10u8, 10u8]));
    // Draw a bright green rectangle from (280, 200) to (360, 280) -> center (320, 240)
    for y in 200..=280 {
        for x in 280..=360 {
            img.put_pixel(x, y, Rgb([0u8, 255u8, 0u8]));
        }
    }
    CVMat::from_dynamic(DynamicImage::ImageRgb8(img))
}

fn make_synthetic_fiducial_mat() -> CVMat {
    let mut img = RgbImage::from_pixel(640, 480, Rgb([235u8, 235u8, 235u8]));
    // Draw two dark square tags:
    // Tag 1 at [270..370, 190..290] (100x100 px centered at (320, 240))
    for y in 190..=290 {
        for x in 270..=370 {
            img.put_pixel(x, y, Rgb([15u8, 15u8, 15u8]));
        }
    }
    // Tag 2 at [100..150, 100..150]
    for y in 100..=150 {
        for x in 100..=150 {
            img.put_pixel(x, y, Rgb([15u8, 15u8, 15u8]));
        }
    }
    CVMat::from_dynamic(DynamicImage::ImageRgb8(img))
}

struct MockDetector;

impl ObjectDetector for MockDetector {
    fn detect(
        &mut self,
        _frame: &CVMat,
        _nms_thresh: f64,
        _box_thresh: f64,
    ) -> Vec<NeuralNetworkPipeResult> {
        vec![NeuralNetworkPipeResult::new(
            260.0, 180.0, 380.0, 300.0, 0, 0.92,
        )]
    }

    fn classes(&self) -> &[String] {
        &[]
    }
}

#[test]
fn test_all_9_pipelines_numerical_parity() {
    let quirks = QuirkyCamera::default();
    let cal = make_calibrated_coefficients();

    // 1. ReflectivePipeline
    let mut ref_provider =
        FileFrameProvider::from_mat(make_synthetic_green_target_mat(), 70.0, Some(cal.clone()));
    let ref_settings = CVPipelineSettings {
        hsv_hue: IntegerCouple::new(40, 80),
        hsv_saturation: IntegerCouple::new(100, 255),
        hsv_value: IntegerCouple::new(100, 255),
        contour_area: DoubleCouple::new(0.1, 50.0),
        solve_pnp_enabled: true,
        ..CVPipelineSettings::default()
    };

    let mut reflective = ReflectivePipeline::new(ref_settings.clone());
    let ref_res = reflective.run(ref_provider.get_frame(), &quirks);
    assert!(ref_res.has_targets());
    let best_ref = &ref_res.targets[0];
    assert!(
        best_ref.yaw.abs() < 0.5,
        "yaw near center: {}",
        best_ref.yaw
    );
    assert!(
        best_ref.pitch.abs() < 0.5,
        "pitch near center: {}",
        best_ref.pitch
    );
    assert!(best_ref.best_camera_to_target_3d.translation.x > 0.5);

    // 2. ColoredShapePipeline
    let mut shape_settings = ref_settings.clone();
    shape_settings.contour_shape = ContourShape::Quadrilateral;
    let mut colored_shape = ColoredShapePipeline::new(shape_settings);
    let shape_res = colored_shape.run(ref_provider.get_frame(), &quirks);
    assert!(shape_res.has_targets());

    // 3. AprilTagPipeline (with single-tag 3D pose & multi-tag estimation)
    let mut tag_provider =
        FileFrameProvider::from_mat(make_synthetic_fiducial_mat(), 70.0, Some(cal.clone()));
    let tag_settings = CVPipelineSettings {
        pipeline_type: PipelineType::AprilTag,
        solve_pnp_enabled: true,
        do_multi_target: true,
        ..CVPipelineSettings::default()
    };

    let mut apriltag = AprilTagPipeline::new(tag_settings.clone());
    apriltag.set_atfl(AprilTagFieldLayout {
        field_dimensions: FieldDimensions::default(),
        field_tags: vec![
            AprilTagFieldEntry {
                id: 1,
                pose: Pose3d::new(Translation3d::new(3.0, 1.0, 0.5), Rotation3d::default()),
            },
            AprilTagFieldEntry {
                id: 2,
                pose: Pose3d::new(Translation3d::new(3.0, 2.0, 0.5), Rotation3d::default()),
            },
        ],
    });
    let tag_res = apriltag.run(tag_provider.get_frame(), &quirks);
    assert!(tag_res.has_targets());
    // 100px tag width with 0.1651m tag and fx=500 -> distance Z = 0.1651 * 500 / 100 = 0.8255m
    let primary_tag = tag_res
        .targets
        .iter()
        .find(|t| (t.target_offset_point.x - 320.0).abs() < 10.0)
        .expect("center tag detected");
    assert!(
        (primary_tag.best_camera_to_target_3d.translation.x - 0.8255).abs() < 0.05,
        "expected ~0.8255m, got {}",
        primary_tag.best_camera_to_target_3d.translation.x
    );
    assert!(tag_res.multi_tag_result.is_some());

    // 4. ArucoPipeline
    let mut aruco = ArucoPipeline::new(tag_settings);
    let aruco_res = aruco.run(tag_provider.get_frame(), &quirks);
    assert!(aruco_res.has_targets());

    // 5. ObjectDetectionPipeline
    let mut obj_pipeline = ObjectDetectionPipeline::new(CVPipelineSettings::default());
    obj_pipeline.set_detector(Box::new(MockDetector));
    let obj_res = obj_pipeline.run(ref_provider.get_frame(), &quirks);
    assert_eq!(obj_res.targets.len(), 1);
    assert_eq!(obj_res.targets[0].class_id, 0);
    assert!((obj_res.targets[0].confidence - 0.92).abs() < 1e-4);

    // 6. Calibrate3dPipeline
    let mut cal_pipeline = Calibrate3dPipeline::new(CVPipelineSettings::default());
    cal_pipeline.take_snapshot();
    let _ = cal_pipeline.run(ref_provider.get_frame(), &quirks);
    assert_eq!(cal_pipeline.observations().len(), 1);
    let computed_cal = cal_pipeline
        .finish_calibration((640, 480), CameraLensModel::LENSMODEL_OPENCV, false)
        .expect("calibration succeeded");
    assert_eq!(computed_cal.camera_intrinsics.data.len(), 9);

    // 7. DriverModePipeline
    let mut driver_pipeline = DriverModePipeline::new(CVPipelineSettings::default());
    let driver_res = driver_pipeline.run(ref_provider.get_frame(), &quirks);
    assert!(driver_res.targets.is_empty());

    // 8. FocusPipeline
    let mut focus_pipeline = FocusPipeline::new(CVPipelineSettings::default());
    let focus_res = focus_pipeline.run(tag_provider.get_frame(), &quirks);
    assert!(focus_res.focus_score.unwrap_or(0.0) > 0.0);

    // 9. OutputStreamPipeline
    let mut out_stream = OutputStreamPipeline::default();
    let rendered = out_stream.process(
        ref_res.input_and_output_frame,
        reflective.settings(),
        ref_res.targets,
    );
    assert!(!rendered.processed_image.is_empty());
}

#[test]
fn test_cvmat_raii_memory_leak_elimination() {
    let baseline = CVMat::get_mat_count();
    {
        let mut provider =
            FileFrameProvider::from_mat(make_synthetic_green_target_mat(), 70.0, None);
        let mut pipeline = ReflectivePipeline::new(CVPipelineSettings::default());
        let quirks = QuirkyCamera::default();
        for _ in 0..250 {
            let f = provider.get_frame();
            let res = pipeline.run(f, &quirks);
            drop(res);
        }
    }
    // After provider and all 250 frame results drop, live matrix count returns exactly to baseline
    assert_eq!(CVMat::get_mat_count(), baseline);
}
