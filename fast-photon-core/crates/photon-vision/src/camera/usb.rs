use crate::camera::BasicSettables;
use crate::frame::{FrameProvider, USBFrameProvider};
use crate::processes::{VisionSource, VisionSourceSettables};
use photon_common::configuration::{CameraConfiguration, CameraQuirk, QuirkyCamera};

pub type GenericUSBCameraSettables = BasicSettables;
pub type ArduOV9281CameraSettables = BasicSettables;
pub type ArduOV2311CameraSettables = BasicSettables;
pub type ArduOV2311WindowsCameraSettables = BasicSettables;
pub type ArduOV9782CameraSettables = BasicSettables;
pub type InnoOV9281CameraSettables = BasicSettables;
pub type ThriftyOV9281CameraSettables = BasicSettables;
pub type LifeCam3kCameraSettables = BasicSettables;
pub type LifeCam3kWindowsCameraSettables = BasicSettables;
pub type PsEyeCameraSettables = BasicSettables;
pub type See3Cam24CUGSettables = BasicSettables;

/// USB camera source with sensor-specific settables and quirk detection (`USBCameraSource`).
pub struct USBCameraSource {
    config: CameraConfiguration,
    settables: BasicSettables,
    frame_provider: USBFrameProvider,
}

impl USBCameraSource {
    pub fn new(mut config: CameraConfiguration) -> Self {
        if config.camera_quirks.quirks.is_empty() {
            let (vid, pid, name) = match &config.matched_camera_info {
                photon_common::configuration::PVCameraInfo::Usb {
                    vendor_id,
                    product_id,
                    name,
                    ..
                } => (*vendor_id, *product_id, name.as_str()),
                other => (-1, -1, other.human_readable_name()),
            };
            config.camera_quirks = QuirkyCamera::find(vid, pid, name);
        }

        // Apply quirk defaults on pipeline settings
        if config.camera_quirks.has_quirk(CameraQuirk::Gain) {
            for p in &mut config.pipeline_settings {
                if p.settings().camera_gain == -1 {
                    p.settings_mut().camera_gain = 75;
                }
            }
        }
        if config.camera_quirks.has_quirk(CameraQuirk::AwbRedBlueGain) {
            for p in &mut config.pipeline_settings {
                if p.settings().camera_red_gain == -1 {
                    p.settings_mut().camera_red_gain = 11;
                }
                if p.settings().camera_blue_gain == -1 {
                    p.settings_mut().camera_blue_gain = 20;
                }
            }
        }

        let cal = config.calibrations.first().cloned();
        let frame_provider =
            USBFrameProvider::new(config.nickname.clone(), 640, 480, config.fov, cal);
        let settables = BasicSettables::new(config.clone());

        Self {
            config,
            settables,
            frame_provider,
        }
    }
}

impl VisionSource for USBCameraSource {
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
