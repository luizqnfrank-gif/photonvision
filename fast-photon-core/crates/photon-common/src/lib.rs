//! `photon-common`: Configuration, SQLite migrations, Event Bus, NetworkTables, and utilities.

pub mod configuration;
pub mod dataflow;
pub mod logging;
pub mod util;

#[cfg(test)]
mod unit_tests {
    use crate::configuration::{CameraConfiguration, CameraQuirk, PVCameraInfo, QuirkyCamera};
    use crate::util::{ColorHelper, MathUtils, NumberCouple};
    use photon_targeting::{Rotation3d, Transform3d, Translation3d};

    #[test]
    fn test_coordinate_conversions_edn_nwu_roundtrip() {
        let orig_nwu = Transform3d::new(
            Translation3d::new(2.5, -0.4, 0.8),
            Rotation3d::from_euler(0.1, -0.2, 0.3),
        );
        let edn = MathUtils::convert_nwu_to_opencv(&orig_nwu);
        // x_edn = -y_nwu = 0.4, y_edn = -z_nwu = -0.8, z_edn = x_nwu = 2.5
        assert!((edn.translation.x - 0.4).abs() < 1e-9);
        assert!((edn.translation.y + 0.8).abs() < 1e-9);
        assert!((edn.translation.z - 2.5).abs() < 1e-9);

        let back_nwu = MathUtils::convert_opencv_to_nwu(&edn);
        assert!((back_nwu.translation.x - orig_nwu.translation.x).abs() < 1e-9);
        assert!((back_nwu.translation.y - orig_nwu.translation.y).abs() < 1e-9);
        assert!((back_nwu.translation.z - orig_nwu.translation.z).abs() < 1e-9);
    }

    #[test]
    fn test_color_helper_and_number_couple() {
        let (h, s, v) = ColorHelper::rgb_to_opencv_hsv(0, 255, 0);
        assert_eq!(h, 60); // Green is 120 deg -> 60 in OpenCV [0..180]
        assert_eq!(s, 255);
        assert_eq!(v, 255);

        let pair = NumberCouple::new(0, 0);
        assert!(pair.is_empty_with(0));
    }

    #[test]
    fn test_quirky_camera_detection_and_duplicate_pipeline_naming() {
        let ov9281 = QuirkyCamera::find(0x0c45, 0x6366, "Arducam OV9281 USB Camera");
        assert!(ov9281.has_quirk(CameraQuirk::ArduOV9281Controls));
        assert!(ov9281.has_quirk(CameraQuirk::Gain));

        let lifecam = QuirkyCamera::find(0x045e, 0x0779, "Microsoft LifeCam HD-3000");
        assert!(lifecam.has_quirk(CameraQuirk::LifeCamControls));

        let mut cam = CameraConfiguration::new(PVCameraInfo::default());
        assert_eq!(cam.pipeline_settings.len(), 1);
        let dup = cam.pipeline_settings[0].clone();
        cam.add_pipeline_setting(dup);
        assert_eq!(cam.pipeline_settings.len(), 2);
        assert_eq!(
            cam.pipeline_settings[1].settings().pipeline_nickname,
            "New Pipeline_1"
        );
        assert_eq!(cam.pipeline_settings[1].settings().pipeline_index, 1);
    }
}
