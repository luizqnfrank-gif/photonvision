//! `photon-vision`: Computer vision matrices, frames, camera sources, pipes, pipelines, and runtime processes.

pub mod apriltag_aruco;
pub mod calibration;
pub mod camera;
pub mod frame;
pub mod objects;
pub mod opencv_ext;
pub mod pipe;
pub mod pipeline;
pub mod processes;
pub mod target;

#[cfg(test)]
mod unit_tests {
    use crate::objects::{Letterbox, NeuralNetworkPipeResult};
    use crate::processes::{PipelineManager, VisionSourceManager, DRIVERMODE_INDEX};
    use photon_common::configuration::{
        CameraConfiguration, DriverModePipelineSettings, PVCameraInfo, PipelineSettingsVariant,
        PipelineType,
    };
    use photon_common::dataflow::DataChangeService;

    #[test]
    fn test_letterbox_scaling_and_unletterbox() {
        let lb = Letterbox::compute(1280, 720, 640, 640);
        assert!((lb.scale - 0.5).abs() < 1e-6);
        assert!((lb.dx - 0.0).abs() < 1e-6);
        assert!((lb.dy - 140.0).abs() < 1e-6);

        let det = NeuralNetworkPipeResult::new(100.0, 240.0, 200.0, 340.0, 1, 0.85);
        let unboxed = lb.unletterbox_detections(vec![det]);
        assert!((unboxed[0].bbox_corners[3].x - 200.0).abs() < 1e-6);
        assert!((unboxed[0].bbox_corners[3].y - 200.0).abs() < 1e-6);
    }

    #[test]
    fn test_pipeline_manager_and_vision_source_lifecycle() {
        let mut pm = PipelineManager::new(
            vec![PipelineSettingsVariant::default()],
            DriverModePipelineSettings::default(),
            0,
        );
        assert_eq!(pm.get_requested_index(), 0);
        let new_idx = pm.add_pipeline(PipelineType::AprilTag, "TagPipe");
        assert_eq!(new_idx, 1);
        assert_eq!(
            pm.current_pipeline().pipeline_type(),
            PipelineType::AprilTag
        );

        let dup_idx = pm.duplicate_pipeline(1);
        assert_eq!(dup_idx, 2);
        assert_eq!(
            pm.current_pipeline().settings().pipeline_nickname,
            "TagPipe_1"
        );

        pm.set_driver_mode(true);
        assert_eq!(pm.get_requested_index(), DRIVERMODE_INDEX);
        pm.set_driver_mode(false);
        assert_eq!(pm.get_requested_index(), 2);

        assert!(pm.remove_pipeline(2));
        assert_eq!(pm.user_pipeline_settings.len(), 2);

        // VisionSourceManager blacklisting & activate/deactivate
        let cams = vec![
            PVCameraInfo::Usb {
                dev: 0,
                path: "/dev/video0".to_string(),
                name: "bcm2835-isp".to_string(),
                other_paths: vec![],
                vendor_id: 0,
                product_id: 0,
                unique_path: "/dev/video0".to_string(),
            },
            PVCameraInfo::File {
                path: "valid.png".to_string(),
                name: "ValidCam".to_string(),
                unique_path: "valid.png".to_string(),
            },
        ];
        let filtered = VisionSourceManager::filter_blacklisted(cams);
        assert_eq!(filtered.len(), 1);

        let dcs = DataChangeService::new();
        let mut vsm = VisionSourceManager::new(dcs, std::env::temp_dir().join("pv_snaps"));
        let mut cfg = CameraConfiguration::new(filtered[0].clone());
        cfg.unique_name = "cam_lifecycle".to_string();
        vsm.register_loaded_configs(vec![cfg]);
        assert_eq!(vsm.vmm.modules().len(), 1);

        assert!(vsm.deactivate_vision_source("cam_lifecycle"));
        assert_eq!(vsm.vmm.modules().len(), 0);
        assert_eq!(vsm.disabled_camera_configs.len(), 1);

        assert!(vsm.reactivate_disabled_camera_config("cam_lifecycle"));
        assert_eq!(vsm.vmm.modules().len(), 1);
        assert_eq!(vsm.disabled_camera_configs.len(), 0);
    }
}
