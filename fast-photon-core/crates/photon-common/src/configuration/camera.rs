#![allow(non_camel_case_types)]

use crate::util::{DoubleCouple, IntegerCouple};
use photon_targeting::TargetModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CameraType {
    #[default]
    UsbCamera,
    ZeroCopyPicam,
    FileCamera,
    TcpCamera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraQuirk {
    Gain,
    AutoExposure,
    Brightness,
    AdjustableFocus,
    CompletelyBroken,
    FPSCap100,
    PiCam,
    LifeCamControls,
    PsEyeControls,
    ArduCamCamera,
    ArduOV9281Controls,
    ArduOV2311Controls,
    ArduOV9782Controls,
    StickyFPS,
    See3Cam24CUGControls,
    AwbRedBlueGain,
    InnoOV9281Controls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuirkyCamera {
    #[serde(default)]
    pub base_name: String,
    #[serde(default = "default_neg_one")]
    pub usb_vid: i32,
    #[serde(default = "default_neg_one")]
    pub usb_pid: i32,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub quirks: HashMap<CameraQuirk, bool>,
}

const fn default_neg_one() -> i32 {
    -1
}

impl Default for QuirkyCamera {
    fn default() -> Self {
        Self {
            base_name: "DefaultCamera".to_string(),
            usb_vid: -1,
            usb_pid: -1,
            display_name: String::new(),
            quirks: HashMap::new(),
        }
    }
}

impl QuirkyCamera {
    pub fn has_quirk(&self, quirk: CameraQuirk) -> bool {
        self.quirks.get(&quirk).copied().unwrap_or(false)
    }

    pub fn find(usb_vid: i32, usb_pid: i32, base_name: &str) -> Self {
        let mut quirks = HashMap::new();
        let lower = base_name.to_lowercase();

        if (usb_vid == 0x0c45 && usb_pid == 0x6366) || lower.contains("ov9281") {
            quirks.insert(CameraQuirk::ArduCamCamera, true);
            quirks.insert(CameraQuirk::ArduOV9281Controls, true);
            quirks.insert(CameraQuirk::Gain, true);
        } else if lower.contains("ov2311") {
            quirks.insert(CameraQuirk::ArduCamCamera, true);
            quirks.insert(CameraQuirk::ArduOV2311Controls, true);
            quirks.insert(CameraQuirk::Gain, true);
        } else if lower.contains("ov9782") {
            quirks.insert(CameraQuirk::ArduCamCamera, true);
            quirks.insert(CameraQuirk::ArduOV9782Controls, true);
            quirks.insert(CameraQuirk::Gain, true);
            quirks.insert(CameraQuirk::AwbRedBlueGain, true);
        } else if (usb_vid == 0x045e && usb_pid == 0x0779) || lower.contains("lifecam") {
            quirks.insert(CameraQuirk::LifeCamControls, true);
        } else if (usb_vid == 0x1415 && usb_pid == 0x2000) || lower.contains("playstation") {
            quirks.insert(CameraQuirk::PsEyeControls, true);
            quirks.insert(CameraQuirk::Gain, true);
            quirks.insert(CameraQuirk::FPSCap100, true);
        } else if lower.contains("see3cam") {
            quirks.insert(CameraQuirk::See3Cam24CUGControls, true);
            quirks.insert(CameraQuirk::Gain, true);
        } else {
            quirks.insert(CameraQuirk::Gain, true);
            quirks.insert(CameraQuirk::Brightness, true);
            quirks.insert(CameraQuirk::AutoExposure, true);
        }

        Self {
            base_name: base_name.to_string(),
            usb_vid,
            usb_pid,
            display_name: base_name.to_string(),
            quirks,
        }
    }
}

/// Hardware camera descriptor (`org.photonvision.vision.camera.PVCameraInfo`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PVCameraInfo {
    #[serde(rename = "PVCameraInfo.PVUsbCameraInfo", alias = "PVUsbCameraInfo")]
    Usb {
        #[serde(default)]
        dev: i32,
        #[serde(default)]
        path: String,
        #[serde(default)]
        name: String,
        #[serde(default, rename = "otherPaths")]
        other_paths: Vec<String>,
        #[serde(default = "default_neg_one", rename = "vendorId")]
        vendor_id: i32,
        #[serde(default = "default_neg_one", rename = "productId")]
        product_id: i32,
        #[serde(default, rename = "uniquePath")]
        unique_path: String,
    },
    #[serde(rename = "PVCameraInfo.PVCSICameraInfo", alias = "PVCSICameraInfo")]
    Csi {
        #[serde(default)]
        path: String,
        #[serde(default, rename = "baseName")]
        base_name: String,
        #[serde(default, rename = "uniquePath")]
        unique_path: String,
        #[serde(default, rename = "otherPaths")]
        other_paths: Vec<String>,
    },
    #[serde(rename = "PVCameraInfo.PVFileCameraInfo", alias = "PVFileCameraInfo")]
    File {
        #[serde(default)]
        path: String,
        #[serde(default)]
        name: String,
        #[serde(default, rename = "uniquePath")]
        unique_path: String,
    },
    #[serde(rename = "PVCameraInfo.PVTcpCameraInfo", alias = "PVTcpCameraInfo")]
    Tcp {
        #[serde(default)]
        uri: String,
        #[serde(default = "default_tcp_cam_name")]
        name: String,
        #[serde(default = "default_tcp_width")]
        width: i32,
        #[serde(default = "default_tcp_height")]
        height: i32,
        #[serde(default = "default_tcp_fps")]
        fps: i32,
        #[serde(default, rename = "uniquePath")]
        unique_path: String,
    },
}

fn default_tcp_cam_name() -> String {
    "rpicam-tcp".to_string()
}

const fn default_tcp_width() -> i32 {
    1280
}

const fn default_tcp_height() -> i32 {
    720
}

const fn default_tcp_fps() -> i32 {
    30
}

impl Default for PVCameraInfo {
    fn default() -> Self {
        Self::File {
            path: "/dev/null".to_string(),
            name: "FileCamera".to_string(),
            unique_path: "/dev/null".to_string(),
        }
    }
}

impl PVCameraInfo {
    pub fn tcp(
        uri: impl Into<String>,
        name: impl Into<String>,
        width: i32,
        height: i32,
        fps: i32,
    ) -> Self {
        let uri_str = uri.into();
        let unique_path = uri_str.clone();
        Self::Tcp {
            uri: uri_str,
            name: name.into(),
            width: if width > 0 { width } else { 1280 },
            height: if height > 0 { height } else { 720 },
            fps: if fps > 0 { fps } else { 30 },
            unique_path,
        }
    }

    pub fn camera_type(&self) -> CameraType {
        match self {
            Self::Usb { .. } => CameraType::UsbCamera,
            Self::Csi { .. } => CameraType::ZeroCopyPicam,
            Self::File { .. } => CameraType::FileCamera,
            Self::Tcp { .. } => CameraType::TcpCamera,
        }
    }

    pub fn unique_path(&self) -> &str {
        match self {
            Self::Usb {
                unique_path, path, ..
            } => {
                if !unique_path.is_empty() {
                    unique_path
                } else {
                    path
                }
            }
            Self::Csi {
                unique_path, path, ..
            } => {
                if !unique_path.is_empty() {
                    unique_path
                } else {
                    path
                }
            }
            Self::File {
                unique_path, path, ..
            } => {
                if !unique_path.is_empty() {
                    unique_path
                } else {
                    path
                }
            }
            Self::Tcp {
                unique_path, uri, ..
            } => {
                if !unique_path.is_empty() {
                    unique_path
                } else {
                    uri
                }
            }
        }
    }

    pub fn human_readable_name(&self) -> &str {
        match self {
            Self::Usb { name, .. } => name,
            Self::Csi { base_name, .. } => base_name,
            Self::File { name, .. } => name,
            Self::Tcp { name, uri, .. } => {
                if !name.is_empty() {
                    name
                } else {
                    uri
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CameraLensModel {
    #[default]
    LENSMODEL_OPENCV,
    LENSMODEL_SPLINED_STEREOGRAPHIC,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct JsonMatOfDouble {
    pub rows: i32,
    pub cols: i32,
    #[serde(default, rename = "type")]
    pub mat_type: i32,
    #[serde(default)]
    pub data: Vec<f64>,
}

impl JsonMatOfDouble {
    pub fn new(rows: i32, cols: i32, data: Vec<f64>) -> Self {
        Self {
            rows,
            cols,
            mat_type: 6, // CV_64F
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardObservation {
    #[serde(default)]
    pub location_in_object_space: Vec<(f64, f64, f64)>,
    #[serde(default)]
    pub location_in_image_space: Vec<(f64, f64)>,
    #[serde(default)]
    pub reprojection_errors: Vec<(f64, f64)>,
    #[serde(default)]
    pub include_observation_in_calibration: bool,
    #[serde(default)]
    pub snapshot_name: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraCalibrationCoefficients {
    #[serde(default)]
    pub resolution: (f64, f64),
    #[serde(default)]
    pub camera_intrinsics: JsonMatOfDouble,
    #[serde(default)]
    pub dist_coeffs: JsonMatOfDouble,
    #[serde(default)]
    pub cal_object_warp: Vec<f64>,
    #[serde(default)]
    pub observations: Vec<BoardObservation>,
    #[serde(default)]
    pub cal_object_size: (f64, f64),
    #[serde(default)]
    pub marker_size: f64,
    #[serde(default)]
    pub lens_model: CameraLensModel,
    #[serde(default)]
    pub mean_errors: Vec<f64>,
}

impl CameraCalibrationCoefficients {
    pub fn fx(&self) -> f64 {
        self.camera_intrinsics
            .data
            .first()
            .copied()
            .unwrap_or(500.0)
    }

    pub fn fy(&self) -> f64 {
        self.camera_intrinsics.data.get(4).copied().unwrap_or(500.0)
    }

    pub fn cx(&self) -> f64 {
        self.camera_intrinsics.data.get(2).copied().unwrap_or(320.0)
    }

    pub fn cy(&self) -> f64 {
        self.camera_intrinsics.data.get(5).copied().unwrap_or(240.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PipelineType {
    Calibrate3d = -3,
    DriverMode = -2,
    Focus = -1,
    #[default]
    Reflective = 0,
    ColoredShape = 1,
    AprilTag = 2,
    Aruco = 3,
    ObjectDetection = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImageFlipMode {
    #[default]
    NONE,
    HORIZONTAL,
    VERTICAL,
    BOTH,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImageRotationMode {
    #[default]
    DEG_0,
    DEG_90_CCW,
    DEG_180_CCW,
    DEG_270_CCW,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FrameDivisor {
    #[default]
    NONE = 1,
    HALF = 2,
    QUARTER = 4,
    SIXTH = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AprilTagFamily {
    #[default]
    kTag36h11,
    kTag16h5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContourGroupingMode {
    #[default]
    Single,
    Two,
    TwoOrMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContourIntersectionDirection {
    #[default]
    None,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContourSortMode {
    #[default]
    Largest,
    Smallest,
    Highest,
    Lowest,
    Rightmost,
    Leftmost,
    Centermost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContourShape {
    #[default]
    Circle,
    Polygon,
    Triangle,
    Quadrilateral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RobotOffsetPointMode {
    #[default]
    None,
    Single,
    Dual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CVPipelineSettings {
    #[serde(default)]
    pub pipeline_index: i32,
    #[serde(default)]
    pub pipeline_type: PipelineType,
    #[serde(default)]
    pub input_image_flip_mode: ImageFlipMode,
    #[serde(default)]
    pub input_image_rotation_mode: ImageRotationMode,
    #[serde(default = "default_pipeline_nickname")]
    pub pipeline_nickname: String,
    #[serde(default = "default_true")]
    pub camera_auto_exposure: bool,
    #[serde(default = "default_exposure")]
    pub camera_exposure_raw: f64,
    #[serde(default = "default_brightness")]
    pub camera_brightness: i32,
    #[serde(default = "default_gain")]
    pub camera_gain: i32,
    #[serde(default = "default_neg_one")]
    pub camera_red_gain: i32,
    #[serde(default = "default_neg_one")]
    pub camera_blue_gain: i32,
    #[serde(default)]
    pub camera_video_mode_index: i32,
    #[serde(default)]
    pub streaming_frame_divisor: FrameDivisor,
    #[serde(default)]
    pub led_mode: bool,
    #[serde(default = "default_true")]
    pub input_should_show: bool,
    #[serde(default = "default_true")]
    pub output_should_show: bool,
    #[serde(default = "default_hsv_hue")]
    pub hsv_hue: IntegerCouple,
    #[serde(default = "default_hsv_sat")]
    pub hsv_saturation: IntegerCouple,
    #[serde(default = "default_hsv_val")]
    pub hsv_value: IntegerCouple,
    #[serde(default)]
    pub hue_inverted: bool,
    #[serde(default = "default_true")]
    pub output_should_draw: bool,
    #[serde(default = "default_max_targets")]
    pub output_maximum_targets: i32,
    #[serde(default = "default_area")]
    pub contour_area: DoubleCouple,
    #[serde(default = "default_ratio")]
    pub contour_ratio: DoubleCouple,
    #[serde(default = "default_fullness")]
    pub contour_fullness: DoubleCouple,
    #[serde(default = "default_speckle")]
    pub contour_speckle_percentage: i32,
    #[serde(default)]
    pub contour_sort_mode: ContourSortMode,
    #[serde(default)]
    pub contour_grouping_mode: ContourGroupingMode,
    #[serde(default)]
    pub contour_intersection: ContourIntersectionDirection,
    #[serde(default)]
    pub solve_pnp_enabled: bool,
    #[serde(default)]
    pub target_model: TargetModel,
    // AprilTag / ArUco specific
    #[serde(default)]
    pub tag_family: AprilTagFamily,
    #[serde(default = "default_one")]
    pub decimate: i32,
    #[serde(default)]
    pub blur: f64,
    #[serde(default = "default_threads")]
    pub threads: i32,
    #[serde(default = "default_true")]
    pub refine_edges: bool,
    #[serde(default = "default_iterations")]
    pub num_iterations: i32,
    #[serde(default)]
    pub hamming_dist: i32,
    #[serde(default = "default_margin")]
    pub decision_margin: i32,
    #[serde(default)]
    pub do_multi_target: bool,
    #[serde(default)]
    pub do_single_target_always: bool,
    // ColoredShape specific
    #[serde(default)]
    pub contour_shape: ContourShape,
    #[serde(default = "default_accuracy")]
    pub accuracy_percentage: f64,
    #[serde(default = "default_circle_thresh")]
    pub circle_detect_threshold: i32,
    // ObjectDetection specific
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_nms")]
    pub nms: f64,
    // Extra unmapped fields preserved for lossless round-tripping
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn default_pipeline_nickname() -> String {
    "New Pipeline".to_string()
}
const fn default_true() -> bool {
    true
}
const fn default_exposure() -> f64 {
    20.0
}
const fn default_brightness() -> i32 {
    50
}
const fn default_gain() -> i32 {
    75
}
const fn default_hsv_hue() -> IntegerCouple {
    IntegerCouple::new(50, 180)
}
const fn default_hsv_sat() -> IntegerCouple {
    IntegerCouple::new(50, 255)
}
const fn default_hsv_val() -> IntegerCouple {
    IntegerCouple::new(50, 255)
}
const fn default_max_targets() -> i32 {
    20
}
const fn default_area() -> DoubleCouple {
    DoubleCouple::new(0.0, 100.0)
}
const fn default_ratio() -> DoubleCouple {
    DoubleCouple::new(0.0, 20.0)
}
const fn default_fullness() -> DoubleCouple {
    DoubleCouple::new(0.0, 100.0)
}
const fn default_speckle() -> i32 {
    5
}
const fn default_one() -> i32 {
    1
}
const fn default_threads() -> i32 {
    4
}
const fn default_iterations() -> i32 {
    40
}
const fn default_margin() -> i32 {
    35
}
const fn default_accuracy() -> f64 {
    10.0
}
const fn default_circle_thresh() -> i32 {
    5
}
const fn default_confidence() -> f64 {
    0.5
}
const fn default_nms() -> f64 {
    0.45
}

impl Default for CVPipelineSettings {
    fn default() -> Self {
        Self {
            pipeline_index: 0,
            pipeline_type: PipelineType::Reflective,
            input_image_flip_mode: ImageFlipMode::NONE,
            input_image_rotation_mode: ImageRotationMode::DEG_0,
            pipeline_nickname: default_pipeline_nickname(),
            camera_auto_exposure: true,
            camera_exposure_raw: 20.0,
            camera_brightness: 50,
            camera_gain: 75,
            camera_red_gain: -1,
            camera_blue_gain: -1,
            camera_video_mode_index: 0,
            streaming_frame_divisor: FrameDivisor::NONE,
            led_mode: false,
            input_should_show: true,
            output_should_show: true,
            hsv_hue: default_hsv_hue(),
            hsv_saturation: default_hsv_sat(),
            hsv_value: default_hsv_val(),
            hue_inverted: false,
            output_should_draw: true,
            output_maximum_targets: 20,
            contour_area: default_area(),
            contour_ratio: default_ratio(),
            contour_fullness: default_fullness(),
            contour_speckle_percentage: 5,
            contour_sort_mode: ContourSortMode::Largest,
            contour_grouping_mode: ContourGroupingMode::Single,
            contour_intersection: ContourIntersectionDirection::None,
            solve_pnp_enabled: false,
            target_model: TargetModel::default(),
            tag_family: AprilTagFamily::kTag36h11,
            decimate: 1,
            blur: 0.0,
            threads: 4,
            refine_edges: true,
            num_iterations: 40,
            hamming_dist: 0,
            decision_margin: 35,
            do_multi_target: false,
            do_single_target_always: false,
            contour_shape: ContourShape::Circle,
            accuracy_percentage: 10.0,
            circle_detect_threshold: 5,
            confidence: 0.5,
            model: String::new(),
            nms: 0.45,
            extra: HashMap::new(),
        }
    }
}

/// Polymorphic pipeline settings wrapper supporting `"type": "AprilTagPipelineSettings"` etc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PipelineSettingsVariant {
    ReflectivePipelineSettings(CVPipelineSettings),
    AprilTagPipelineSettings(CVPipelineSettings),
    ArucoPipelineSettings(CVPipelineSettings),
    ColoredShapePipelineSettings(CVPipelineSettings),
    ObjectDetectionPipelineSettings(CVPipelineSettings),
    Calibration3dPipelineSettings(CVPipelineSettings),
    DriverModePipelineSettings(CVPipelineSettings),
    FocusPipelineSettings(CVPipelineSettings),
}

impl Default for PipelineSettingsVariant {
    fn default() -> Self {
        Self::ReflectivePipelineSettings(CVPipelineSettings::default())
    }
}

impl PipelineSettingsVariant {
    pub fn settings(&self) -> &CVPipelineSettings {
        match self {
            Self::ReflectivePipelineSettings(s)
            | Self::AprilTagPipelineSettings(s)
            | Self::ArucoPipelineSettings(s)
            | Self::ColoredShapePipelineSettings(s)
            | Self::ObjectDetectionPipelineSettings(s)
            | Self::Calibration3dPipelineSettings(s)
            | Self::DriverModePipelineSettings(s)
            | Self::FocusPipelineSettings(s) => s,
        }
    }

    pub fn settings_mut(&mut self) -> &mut CVPipelineSettings {
        match self {
            Self::ReflectivePipelineSettings(s)
            | Self::AprilTagPipelineSettings(s)
            | Self::ArucoPipelineSettings(s)
            | Self::ColoredShapePipelineSettings(s)
            | Self::ObjectDetectionPipelineSettings(s)
            | Self::Calibration3dPipelineSettings(s)
            | Self::DriverModePipelineSettings(s)
            | Self::FocusPipelineSettings(s) => s,
        }
    }

    pub fn pipeline_type(&self) -> PipelineType {
        match self {
            Self::ReflectivePipelineSettings(_) => PipelineType::Reflective,
            Self::AprilTagPipelineSettings(_) => PipelineType::AprilTag,
            Self::ArucoPipelineSettings(_) => PipelineType::Aruco,
            Self::ColoredShapePipelineSettings(_) => PipelineType::ColoredShape,
            Self::ObjectDetectionPipelineSettings(_) => PipelineType::ObjectDetection,
            Self::Calibration3dPipelineSettings(_) => PipelineType::Calibrate3d,
            Self::DriverModePipelineSettings(_) => PipelineType::DriverMode,
            Self::FocusPipelineSettings(_) => PipelineType::Focus,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverModePipelineSettings {
    #[serde(default = "default_drivermode_index")]
    pub pipeline_index: i32,
    #[serde(default = "default_drivermode_type")]
    pub pipeline_type: PipelineType,
    #[serde(default)]
    pub input_image_flip_mode: ImageFlipMode,
    #[serde(default)]
    pub input_image_rotation_mode: ImageRotationMode,
    #[serde(default = "default_drivermode_nickname")]
    pub pipeline_nickname: String,
    #[serde(default = "default_true")]
    pub camera_auto_exposure: bool,
    #[serde(default = "default_exposure")]
    pub camera_exposure_raw: f64,
    #[serde(default = "default_brightness")]
    pub camera_brightness: i32,
    #[serde(default = "default_gain")]
    pub camera_gain: i32,
    #[serde(default = "default_neg_one")]
    pub camera_red_gain: i32,
    #[serde(default = "default_neg_one")]
    pub camera_blue_gain: i32,
    #[serde(default)]
    pub camera_video_mode_index: i32,
    #[serde(default)]
    pub streaming_frame_divisor: FrameDivisor,
    #[serde(default)]
    pub led_mode: bool,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

const fn default_drivermode_index() -> i32 {
    -2
}
const fn default_drivermode_type() -> PipelineType {
    PipelineType::DriverMode
}
fn default_drivermode_nickname() -> String {
    "Driver Mode".to_string()
}

impl Default for DriverModePipelineSettings {
    fn default() -> Self {
        Self {
            pipeline_index: -2,
            pipeline_type: PipelineType::DriverMode,
            input_image_flip_mode: ImageFlipMode::NONE,
            input_image_rotation_mode: ImageRotationMode::DEG_0,
            pipeline_nickname: default_drivermode_nickname(),
            camera_auto_exposure: true,
            camera_exposure_raw: 20.0,
            camera_brightness: 50,
            camera_gain: 75,
            camera_red_gain: -1,
            camera_blue_gain: -1,
            camera_video_mode_index: 0,
            streaming_frame_divisor: FrameDivisor::NONE,
            led_mode: false,
            extra: HashMap::new(),
        }
    }
}

/// Per-camera configuration (`org.photonvision.common.configuration.CameraConfiguration`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraConfiguration {
    #[serde(default)]
    pub unique_name: String,
    #[serde(default)]
    pub matched_camera_info: PVCameraInfo,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub deactivated: bool,
    #[serde(default)]
    pub camera_quirks: QuirkyCamera,
    #[serde(default = "default_fov", rename = "FOV")]
    pub fov: f64,
    #[serde(default)]
    pub calibrations: Vec<CameraCalibrationCoefficients>,
    #[serde(default)]
    pub current_pipeline_index: i32,
    #[serde(default)]
    pub stream_index: i32,
    #[serde(default)]
    pub pipeline_settings: Vec<PipelineSettingsVariant>,
    #[serde(default)]
    pub drive_mode_settings: DriverModePipelineSettings,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

const fn default_fov() -> f64 {
    70.0
}

impl Default for CameraConfiguration {
    fn default() -> Self {
        Self::new(PVCameraInfo::default())
    }
}

impl CameraConfiguration {
    pub fn new(cam_info: PVCameraInfo) -> Self {
        let nickname = cam_info.human_readable_name().to_string();
        Self {
            unique_name: Uuid::new_v4().to_string(),
            matched_camera_info: cam_info,
            nickname,
            deactivated: false,
            camera_quirks: QuirkyCamera::default(),
            fov: 70.0,
            calibrations: Vec::new(),
            current_pipeline_index: 0,
            stream_index: 0,
            pipeline_settings: vec![PipelineSettingsVariant::default()],
            drive_mode_settings: DriverModePipelineSettings::default(),
            extra: HashMap::new(),
        }
    }

    pub fn add_pipeline_setting(&mut self, mut setting: PipelineSettingsVariant) {
        let exists = self.pipeline_settings.iter().any(|s| {
            s.settings()
                .pipeline_nickname
                .eq_ignore_ascii_case(&setting.settings().pipeline_nickname)
        });
        if exists {
            setting.settings_mut().pipeline_nickname.push_str("_1");
        }
        let new_index = self.pipeline_settings.len() as i32;
        setting.settings_mut().pipeline_index = new_index;
        self.pipeline_settings.push(setting);
    }

    pub fn calibration_for_resolution(
        &self,
        width: i32,
        height: i32,
    ) -> Option<&CameraCalibrationCoefficients> {
        self.calibrations
            .iter()
            .find(|c| (c.resolution.0 as i32) == width && (c.resolution.1 as i32) == height)
    }
}
