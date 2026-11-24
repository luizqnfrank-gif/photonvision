pub mod csi;
pub mod tcp;
pub mod usb;

pub use csi::*;
pub use tcp::*;
pub use usb::*;

use crate::frame::{FileFrameProvider, FrameProvider};
use crate::processes::{VideoMode, VisionSource, VisionSourceSettables};
pub use photon_common::configuration::{
    CameraConfiguration, CameraQuirk, CameraType, PVCameraInfo, QuirkyCamera,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BasicSettables {
    config: CameraConfiguration,
    exposure_raw: f64,
    auto_exposure: bool,
    brightness: i32,
    gain: i32,
    red_gain: i32,
    blue_gain: i32,
    video_modes: HashMap<i32, VideoMode>,
    current_mode_index: i32,
}

impl BasicSettables {
    pub fn new(config: CameraConfiguration) -> Self {
        let mut video_modes = HashMap::new();
        video_modes.insert(
            0,
            VideoMode {
                width: 640,
                height: 480,
                fps: 60,
                pixel_format: "MJPEG".to_string(),
            },
        );
        video_modes.insert(
            1,
            VideoMode {
                width: 1280,
                height: 720,
                fps: 60,
                pixel_format: "MJPEG".to_string(),
            },
        );
        Self {
            config,
            exposure_raw: 20.0,
            auto_exposure: true,
            brightness: 50,
            gain: 75,
            red_gain: 11,
            blue_gain: 20,
            video_modes,
            current_mode_index: 0,
        }
    }
}

impl VisionSourceSettables for BasicSettables {
    fn configuration(&self) -> &CameraConfiguration {
        &self.config
    }
    fn set_exposure_raw(&mut self, exposure_raw: f64) {
        self.exposure_raw = exposure_raw;
    }
    fn get_exposure_raw(&self) -> f64 {
        self.exposure_raw
    }
    fn set_auto_exposure(&mut self, auto_exposure: bool) {
        self.auto_exposure = auto_exposure;
    }
    fn set_brightness(&mut self, brightness: i32) {
        self.brightness = brightness;
    }
    fn set_gain(&mut self, gain: i32) {
        self.gain = gain;
    }
    fn set_red_gain(&mut self, red_gain: i32) {
        self.red_gain = red_gain;
    }
    fn set_blue_gain(&mut self, blue_gain: i32) {
        self.blue_gain = blue_gain;
    }
    fn get_fov(&self) -> f64 {
        self.config.fov
    }
    fn set_fov(&mut self, fov: f64) {
        self.config.fov = fov;
    }
    fn get_video_modes(&self) -> &HashMap<i32, VideoMode> {
        &self.video_modes
    }
    fn get_current_video_mode(&self) -> VideoMode {
        self.video_modes
            .get(&self.current_mode_index)
            .cloned()
            .unwrap_or_default()
    }
    fn set_video_mode_index(&mut self, index: i32) {
        if self.video_modes.contains_key(&index) {
            self.current_mode_index = index;
        }
    }
}

/// File-backed vision source (`org.photonvision.vision.camera.FileVisionSource`).
pub struct FileVisionSource {
    config: CameraConfiguration,
    settables: BasicSettables,
    frame_provider: FileFrameProvider,
}

impl FileVisionSource {
    pub fn new(config: CameraConfiguration) -> Self {
        let path = PathBuf::from(config.matched_camera_info.unique_path());
        let cal = config.calibrations.first().cloned();
        let frame_provider = FileFrameProvider::new(path, config.fov, cal);
        let settables = BasicSettables::new(config.clone());
        Self {
            config,
            settables,
            frame_provider,
        }
    }
}

impl VisionSource for FileVisionSource {
    fn frame_provider(&mut self) -> &mut dyn FrameProvider {
        &mut self.frame_provider
    }
    fn settables(&self) -> &dyn VisionSourceSettables {
        &self.settables
    }
    fn settables_mut(&mut self) -> &mut dyn VisionSourceSettables {
        &mut self.settables
    }
    fn camera_configuration(&self) -> &CameraConfiguration {
        &self.config
    }
    fn camera_configuration_mut(&mut self) -> &mut CameraConfiguration {
        &mut self.config
    }
    fn is_vendor_camera(&self) -> bool {
        false
    }
    fn has_leds(&self) -> bool {
        false
    }
}

pub type TestSource = FileVisionSource;
