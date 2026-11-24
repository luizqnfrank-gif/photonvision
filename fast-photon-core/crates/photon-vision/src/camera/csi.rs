use crate::camera::BasicSettables;
use crate::frame::{FrameProvider, LibcameraGpuFrameProvider};
use crate::processes::{VisionSource, VisionSourceSettables};
use photon_common::configuration::{CameraConfiguration, CameraQuirk};

pub type LibcameraGpuSettables = BasicSettables;

/// CSI Libcamera GPU source (`org.photonvision.vision.camera.csi.LibcameraGpuSource`).
pub struct LibcameraGpuSource {
    config: CameraConfiguration,
    settables: LibcameraGpuSettables,
    frame_provider: LibcameraGpuFrameProvider,
}

impl LibcameraGpuSource {
    pub fn new(mut config: CameraConfiguration) -> Self {
        config.camera_quirks.quirks.insert(CameraQuirk::PiCam, true);
        config.camera_quirks.quirks.insert(CameraQuirk::Gain, true);
        config
            .camera_quirks
            .quirks
            .insert(CameraQuirk::AwbRedBlueGain, true);

        let cal = config.calibrations.first().cloned();
        let frame_provider = LibcameraGpuFrameProvider::new(
            config.matched_camera_info.unique_path(),
            640,
            480,
            config.fov,
            cal,
        );
        let settables = BasicSettables::new(config.clone());
        Self {
            config,
            settables,
            frame_provider,
        }
    }
}

impl VisionSource for LibcameraGpuSource {
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
        true
    }
    fn has_leds(&self) -> bool {
        true
    }
}
