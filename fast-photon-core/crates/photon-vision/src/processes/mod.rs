pub mod pipeline_manager;
pub mod vision_module;
pub mod vision_runner;
pub mod vision_source_manager;

pub use pipeline_manager::*;
pub use vision_module::*;
pub use vision_runner::*;
pub use vision_source_manager::*;

use crate::frame::FrameProvider;
use photon_common::configuration::CameraConfiguration;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoMode {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub pixel_format: String,
}

impl Default for VideoMode {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            fps: 60,
            pixel_format: "MJPEG".to_string(),
        }
    }
}

/// Camera hardware settables trait (`org.photonvision.vision.processes.VisionSourceSettables`).
pub trait VisionSourceSettables: Send + Sync {
    fn configuration(&self) -> &CameraConfiguration;
    fn set_exposure_raw(&mut self, exposure_raw: f64);
    fn get_exposure_raw(&self) -> f64;
    fn set_auto_exposure(&mut self, auto_exposure: bool);
    fn set_brightness(&mut self, brightness: i32);
    fn set_gain(&mut self, gain: i32);
    fn set_red_gain(&mut self, red_gain: i32);
    fn set_blue_gain(&mut self, blue_gain: i32);
    fn get_fov(&self) -> f64;
    fn set_fov(&mut self, fov: f64);
    fn get_video_modes(&self) -> &HashMap<i32, VideoMode>;
    fn get_current_video_mode(&self) -> VideoMode;
    fn set_video_mode_index(&mut self, index: i32);
}

/// Abstract camera vision source (`org.photonvision.vision.processes.VisionSource`).
pub trait VisionSource: Send + Sync {
    fn frame_provider(&mut self) -> &mut dyn FrameProvider;
    fn settables(&self) -> &dyn VisionSourceSettables;
    fn settables_mut(&mut self) -> &mut dyn VisionSourceSettables;
    fn camera_configuration(&self) -> &CameraConfiguration;
    fn camera_configuration_mut(&mut self) -> &mut CameraConfiguration;
    fn is_vendor_camera(&self) -> bool;
    fn has_leds(&self) -> bool;
}
