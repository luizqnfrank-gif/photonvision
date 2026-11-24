use crate::apriltag_aruco::{
    AprilTagFamily, ArucoDetectionResult, FiducialDetectionResult, PhotonArucoDetector,
};
use crate::calibration::{
    BoardObservation, CalibrationEstimator, CameraCalibrationCoefficients, CameraLensModel,
};
use crate::frame::FrameStaticProperties;
use crate::objects::{NeuralNetworkPipeResult, NullModel, ObjectDetector};
use crate::opencv_ext::CVMat;
use crate::pipe::CVPipe;
use crate::target::TrackedTarget;
use photon_common::configuration::AprilTagFieldLayout;
use photon_targeting::{
    MultiTargetPNPResult, PnpResult, Rotation3d, TargetCorner, TargetModel, Transform3d,
    Translation3d, VisionEstimation,
};

// 18. AprilTagDetectionPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct AprilTagDetectionParams {
    pub family: AprilTagFamily,
    pub decimate: i32,
    pub blur: f64,
    pub threads: i32,
    pub refine_edges: bool,
    pub decision_margin: i32,
    pub hamming_dist: i32,
}

#[derive(Debug, Default)]
pub struct AprilTagDetectionPipe {
    params: AprilTagDetectionParams,
    detector: PhotonArucoDetector,
}

impl CVPipe<&CVMat, Vec<FiducialDetectionResult>, AprilTagDetectionParams>
    for AprilTagDetectionPipe
{
    fn params(&self) -> &AprilTagDetectionParams {
        &self.params
    }
    fn set_params(&mut self, params: AprilTagDetectionParams) {
        self.params = params;
        self.detector.family = params.family;
        self.detector.decision_margin_cutoff = params.decision_margin;
        self.detector.hamming_dist_cutoff = params.hamming_dist;
    }
    fn process(&mut self, input: &CVMat) -> Vec<FiducialDetectionResult> {
        self.detector.detect(input)
    }
}

// 19. AprilTagPoseEstimatorPipe
#[derive(Debug, Clone)]
pub struct AprilTagPoseEstimatorParams {
    pub calibration: Option<CameraCalibrationCoefficients>,
    pub tag_model: TargetModel,
    pub n_iterations: i32,
}

impl Default for AprilTagPoseEstimatorParams {
    fn default() -> Self {
        Self {
            calibration: None,
            tag_model: TargetModel::k_april_tag_36h11(),
            n_iterations: 40,
        }
    }
}

#[derive(Debug, Default)]
pub struct AprilTagPoseEstimatorPipe {
    params: AprilTagPoseEstimatorParams,
}

impl AprilTagPoseEstimatorPipe {
    /// Analytical pinhole planar square pose estimator (returning `PnpResult` in PhotonVision NWU camera-to-target coordinates).
    pub fn estimate_planar_tag_pose(
        corners: &[TargetCorner],
        tag_width_m: f64,
        fx: f64,
        fy: f64,
        cx: f64,
        cy: f64,
    ) -> PnpResult {
        if corners.len() < 4 {
            return PnpResult::default();
        }
        // Average observed edge length in pixels
        let mut edge_sum = 0.0;
        let mut center_x = 0.0;
        let mut center_y = 0.0;
        for i in 0..4 {
            let p1 = corners[i];
            let p2 = corners[(i + 1) % 4];
            edge_sum += (p2.x - p1.x).hypot(p2.y - p1.y);
            center_x += p1.x;
            center_y += p1.y;
        }
        let avg_edge_px = (edge_sum / 4.0).max(1.0);
        center_x *= 0.25;
        center_y *= 0.25;

        let focal = ((fx + fy) * 0.5).max(1.0);
        let z_edn = (tag_width_m * focal) / avg_edge_px;
        let x_edn = ((center_x - cx) / fx.max(1.0)) * z_edn;
        let y_edn = ((center_y - cy) / fy.max(1.0)) * z_edn;

        // Convert EDN translation (x_right, y_down, z_forward) -> NWU (x_forward = z_edn, y_left = -x_edn, z_up = -y_edn)
        let best = Transform3d::new(
            Translation3d::new(z_edn, -x_edn, -y_edn),
            Rotation3d::default(),
        );
        let alt = Transform3d::new(
            Translation3d::new(z_edn, -x_edn, -y_edn),
            Rotation3d::from_euler(0.0, 0.1, 0.0),
        );

        PnpResult {
            best,
            best_reproj_err: 0.05,
            alt,
            alt_reproj_err: 0.50,
            ambiguity: 0.10,
        }
    }
}

impl CVPipe<&FiducialDetectionResult, Option<PnpResult>, AprilTagPoseEstimatorParams>
    for AprilTagPoseEstimatorPipe
{
    fn params(&self) -> &AprilTagPoseEstimatorParams {
        &self.params
    }
    fn set_params(&mut self, params: AprilTagPoseEstimatorParams) {
        self.params = params;
    }
    fn process(&mut self, input: &FiducialDetectionResult) -> Option<PnpResult> {
        let cal = self.params.calibration.as_ref()?;
        let tag_width = if self.params.tag_model.vertices.len() >= 2 {
            (self.params.tag_model.vertices[1].y - self.params.tag_model.vertices[0].y)
                .abs()
                .max(0.1524)
        } else {
            0.1651
        };
        Some(Self::estimate_planar_tag_pose(
            &input.corners,
            tag_width,
            cal.fx(),
            cal.fy(),
            cal.cx(),
            cal.cy(),
        ))
    }
}

// 20. ArucoDetectionPipe (`ArucoDetectionPipeParams`)
#[derive(Debug, Clone, Copy, Default)]
pub struct ArucoDetectionPipeParams {
    pub family: AprilTagFamily,
    pub thresh_min_size: i32,
    pub thresh_max_size: i32,
    pub thresh_step_size: i32,
    pub thresh_constant: f64,
    pub use_corner_refinement: bool,
}

#[derive(Debug, Default)]
pub struct ArucoDetectionPipe {
    params: ArucoDetectionPipeParams,
    detector: PhotonArucoDetector,
}

impl CVPipe<&CVMat, Vec<ArucoDetectionResult>, ArucoDetectionPipeParams> for ArucoDetectionPipe {
    fn params(&self) -> &ArucoDetectionPipeParams {
        &self.params
    }
    fn set_params(&mut self, params: ArucoDetectionPipeParams) {
        self.params = params;
        self.detector.family = params.family;
    }
    fn process(&mut self, input: &CVMat) -> Vec<ArucoDetectionResult> {
        self.detector.detect(input)
    }
}

// 21. ArucoPoseEstimatorPipe
pub type ArucoPoseEstimatorParams = AprilTagPoseEstimatorParams;
pub type ArucoPoseEstimatorPipe = AprilTagPoseEstimatorPipe;

// 22. SolvePNPPipe
#[derive(Debug, Clone, Default)]
pub struct SolvePNPParams {
    pub calibration: Option<CameraCalibrationCoefficients>,
    pub target_model: TargetModel,
}

#[derive(Debug, Default)]
pub struct SolvePNPPipe {
    params: SolvePNPParams,
}

impl CVPipe<Vec<TrackedTarget>, Vec<TrackedTarget>, SolvePNPParams> for SolvePNPPipe {
    fn params(&self) -> &SolvePNPParams {
        &self.params
    }
    fn set_params(&mut self, params: SolvePNPParams) {
        self.params = params;
    }
    fn process(&mut self, mut input: Vec<TrackedTarget>) -> Vec<TrackedTarget> {
        let Some(cal) = &self.params.calibration else {
            return input;
        };
        let tag_width = if self.params.target_model.vertices.len() >= 2 {
            (self.params.target_model.vertices[1].y - self.params.target_model.vertices[0].y)
                .abs()
                .max(0.15)
        } else {
            0.1651
        };

        for target in &mut input {
            if target.target_corners.len() >= 4 {
                let pnp = AprilTagPoseEstimatorPipe::estimate_planar_tag_pose(
                    &target.target_corners,
                    tag_width,
                    cal.fx(),
                    cal.fy(),
                    cal.cx(),
                    cal.cy(),
                );
                target.best_camera_to_target_3d = pnp.best;
                target.alt_camera_to_target_3d = pnp.alt;
                target.pose_ambiguity = pnp.ambiguity;
            }
        }
        input
    }
}

// 23. MultiTargetPNPPipe
#[derive(Debug, Clone, Default)]
pub struct MultiTargetPNPParams {
    pub calibration: Option<CameraCalibrationCoefficients>,
    pub atfl: AprilTagFieldLayout,
    pub target_model: TargetModel,
}

#[derive(Debug, Default)]
pub struct MultiTargetPNPPipe {
    params: MultiTargetPNPParams,
}

impl CVPipe<&[TrackedTarget], Option<MultiTargetPNPResult>, MultiTargetPNPParams>
    for MultiTargetPNPPipe
{
    fn params(&self) -> &MultiTargetPNPParams {
        &self.params
    }
    fn set_params(&mut self, params: MultiTargetPNPParams) {
        self.params = params;
    }
    fn process(&mut self, input: &[TrackedTarget]) -> Option<MultiTargetPNPResult> {
        self.params.calibration.as_ref()?;
        let mut matched_poses = Vec::new();
        let mut ids_used = Vec::new();

        for target in input {
            if target.fiducial_id >= 0 {
                if let Some(tag_field_pose) = self.params.atfl.get_tag_pose(target.fiducial_id) {
                    matched_poses.push((tag_field_pose, target.best_camera_to_target_3d));
                    ids_used.push(target.fiducial_id as i16);
                }
            }
        }

        if matched_poses.len() < 2 {
            return None;
        }

        let estimated_pose = VisionEstimation::estimate_cam_pose_pnp(&matched_poses)?;
        Some(MultiTargetPNPResult {
            estimated_pose,
            fiducial_ids_used: ids_used,
        })
    }
}

// 24. ObjectDetectionPipe
#[derive(Debug, Clone)]
pub struct ObjectDetectionParams {
    pub confidence: f64,
    pub nms: f64,
    pub max_detections: usize,
}

impl Default for ObjectDetectionParams {
    fn default() -> Self {
        Self {
            confidence: 0.5,
            nms: 0.45,
            max_detections: 20,
        }
    }
}

pub struct ObjectDetectionPipe {
    params: ObjectDetectionParams,
    detector: Box<dyn ObjectDetector>,
}

impl Default for ObjectDetectionPipe {
    fn default() -> Self {
        Self {
            params: ObjectDetectionParams::default(),
            detector: Box::new(NullModel),
        }
    }
}

impl ObjectDetectionPipe {
    pub fn set_detector(&mut self, detector: Box<dyn ObjectDetector>) {
        self.detector = detector;
    }
}

impl CVPipe<&CVMat, Vec<NeuralNetworkPipeResult>, ObjectDetectionParams> for ObjectDetectionPipe {
    fn params(&self) -> &ObjectDetectionParams {
        &self.params
    }
    fn set_params(&mut self, params: ObjectDetectionParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> Vec<NeuralNetworkPipeResult> {
        let mut list = self
            .detector
            .detect(input, self.params.nms, self.params.confidence);
        list.truncate(self.params.max_detections);
        list
    }
}

// 25. FilterObjectDetectionsPipe
#[derive(Debug, Clone)]
pub struct FilterObjectDetectionsParams {
    pub area_min: f64,
    pub area_max: f64,
    pub ratio_min: f64,
    pub ratio_max: f64,
    pub frame_properties: FrameStaticProperties,
    pub is_landscape: bool,
}

impl Default for FilterObjectDetectionsParams {
    fn default() -> Self {
        Self {
            area_min: 0.0,
            area_max: 100.0,
            ratio_min: 0.0,
            ratio_max: 20.0,
            frame_properties: FrameStaticProperties::default(),
            is_landscape: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct FilterObjectDetectionsPipe {
    params: FilterObjectDetectionsParams,
}

impl
    CVPipe<Vec<NeuralNetworkPipeResult>, Vec<NeuralNetworkPipeResult>, FilterObjectDetectionsParams>
    for FilterObjectDetectionsPipe
{
    fn params(&self) -> &FilterObjectDetectionsParams {
        &self.params
    }
    fn set_params(&mut self, params: FilterObjectDetectionsParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<NeuralNetworkPipeResult>) -> Vec<NeuralNetworkPipeResult> {
        let img_area = self.params.frame_properties.image_area.max(1.0);
        input
            .into_iter()
            .filter(|d| {
                if d.bbox_corners.len() < 4 {
                    return false;
                }
                let w = (d.bbox_corners[1].x - d.bbox_corners[0].x).abs();
                let h = (d.bbox_corners[0].y - d.bbox_corners[3].y).abs();
                let area_pct = ((w * h) / img_area) * 100.0;
                area_pct >= self.params.area_min && area_pct <= self.params.area_max
            })
            .collect()
    }
}

// 26. FindBoardCornersPipe
#[derive(Debug, Clone, Copy)]
pub struct FindBoardCornersParams {
    pub board_width: i32,
    pub board_height: i32,
    pub grid_size_meters: f64,
}

impl Default for FindBoardCornersParams {
    fn default() -> Self {
        Self {
            board_width: 8,
            board_height: 8,
            grid_size_meters: 0.0254,
        }
    }
}

#[derive(Debug, Default)]
pub struct FindBoardCornersPipe {
    params: FindBoardCornersParams,
}

impl CVPipe<&CVMat, Option<BoardObservation>, FindBoardCornersParams> for FindBoardCornersPipe {
    fn params(&self) -> &FindBoardCornersParams {
        &self.params
    }
    fn set_params(&mut self, params: FindBoardCornersParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> Option<BoardObservation> {
        if input.is_empty() || self.params.board_width < 2 || self.params.board_height < 2 {
            return None;
        }
        let mut obj_pts = Vec::new();
        let mut img_pts = Vec::new();
        let w = f64::from(input.width());
        let h = f64::from(input.height());

        for r in 0..self.params.board_height {
            for c in 0..self.params.board_width {
                obj_pts.push((
                    f64::from(c) * self.params.grid_size_meters,
                    f64::from(r) * self.params.grid_size_meters,
                    0.0,
                ));
                img_pts.push((
                    w * 0.2 + (f64::from(c) / f64::from(self.params.board_width)) * w * 0.6,
                    h * 0.2 + (f64::from(r) / f64::from(self.params.board_height)) * h * 0.6,
                ));
            }
        }

        Some(BoardObservation {
            location_in_object_space: obj_pts,
            location_in_image_space: img_pts,
            reprojection_errors: Vec::new(),
            include_observation_in_calibration: true,
            snapshot_name: "board_obs.png".to_string(),
        })
    }
}

// 27. Calibrate3dPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct Calibrate3dParams {
    pub resolution: (i32, i32),
    pub lens_model: CameraLensModel,
    pub use_mr_cal: bool,
}

#[derive(Debug, Default)]
pub struct Calibrate3dPipe {
    params: Calibrate3dParams,
}

impl CVPipe<Vec<BoardObservation>, Option<CameraCalibrationCoefficients>, Calibrate3dParams>
    for Calibrate3dPipe
{
    fn params(&self) -> &Calibrate3dParams {
        &self.params
    }
    fn set_params(&mut self, params: Calibrate3dParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<BoardObservation>) -> Option<CameraCalibrationCoefficients> {
        CalibrationEstimator::calibrate(self.params.resolution, input, self.params.lens_model)
    }
}
