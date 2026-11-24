use crate::camera::{FileVisionSource, LibcameraGpuSource, TcpCameraSource, USBCameraSource};
use crate::processes::{VisionModule, VisionSource};
use photon_common::configuration::{CameraConfiguration, CameraType, PVCameraInfo};
use photon_common::dataflow::websocket::UICameraConfiguration;
use photon_common::dataflow::DataChangeService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const DEVICE_BLACKLIST: &[&str] = &["bcm2835-isp"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionSourceManagerState {
    pub disabled_configs: Vec<UICameraConfiguration>,
    pub all_connected_cameras: Vec<PVCameraInfo>,
}

/// Holds active `VisionModule` instances (`org.photonvision.vision.processes.VisionModuleManager`).
#[derive(Default)]
pub struct VisionModuleManager {
    modules: Vec<VisionModule>,
}

impl VisionModuleManager {
    pub fn new() -> Self {
        Self {
            modules: Vec::new(),
        }
    }

    pub fn modules(&self) -> &[VisionModule] {
        &self.modules
    }

    pub fn modules_mut(&mut self) -> &mut [VisionModule] {
        &mut self.modules
    }

    pub fn get_module(&self, unique_name: &str) -> Option<&VisionModule> {
        self.modules.iter().find(|m| m.unique_name() == unique_name)
    }

    pub fn get_module_mut(&mut self, unique_name: &str) -> Option<&mut VisionModule> {
        self.modules
            .iter_mut()
            .find(|m| m.unique_name() == unique_name)
    }

    pub fn add_source(
        &mut self,
        mut source: Box<dyn VisionSource>,
        data_change_service: DataChangeService,
        snapshots_dir: PathBuf,
    ) -> &mut VisionModule {
        let stream_index = self.modules.len() as i32;
        source.camera_configuration_mut().stream_index = stream_index;
        let module = VisionModule::new(source, data_change_service, snapshots_dir, Vec::new());
        self.modules.push(module);
        let last = self.modules.len() - 1;
        &mut self.modules[last]
    }

    pub fn remove_module(&mut self, unique_name: &str) -> Option<VisionModule> {
        let pos = self
            .modules
            .iter()
            .position(|m| m.unique_name() == unique_name)?;
        Some(self.modules.remove(pos))
    }
}

/// Manages camera discovery, matching, active `VisionModule` creation, deactivation, and reactivation (`VisionSourceManager`).
pub struct VisionSourceManager {
    pub vmm: VisionModuleManager,
    pub disabled_camera_configs: HashMap<String, CameraConfiguration>,
    pub connected_cameras: Vec<PVCameraInfo>,
    data_change_service: DataChangeService,
    snapshots_dir: PathBuf,
}

impl VisionSourceManager {
    pub fn new(data_change_service: DataChangeService, snapshots_dir: impl Into<PathBuf>) -> Self {
        Self {
            vmm: VisionModuleManager::new(),
            disabled_camera_configs: HashMap::new(),
            connected_cameras: Vec::new(),
            data_change_service,
            snapshots_dir: snapshots_dir.into(),
        }
    }

    pub fn filter_blacklisted(cameras: Vec<PVCameraInfo>) -> Vec<PVCameraInfo> {
        cameras
            .into_iter()
            .filter(|c| {
                !DEVICE_BLACKLIST
                    .iter()
                    .any(|b| c.human_readable_name().contains(b))
            })
            .collect()
    }

    pub fn load_vision_source_from_cam_config(
        config: CameraConfiguration,
    ) -> Box<dyn VisionSource> {
        match config.matched_camera_info.camera_type() {
            CameraType::UsbCamera => Box::new(USBCameraSource::new(config)),
            CameraType::ZeroCopyPicam => Box::new(LibcameraGpuSource::new(config)),
            CameraType::FileCamera => Box::new(FileVisionSource::new(config)),
            CameraType::TcpCamera => Box::new(TcpCameraSource::new(config)),
        }
    }

    pub fn register_loaded_configs(
        &mut self,
        configs: impl IntoIterator<Item = CameraConfiguration>,
    ) {
        let mut deduped: HashMap<String, CameraConfiguration> = HashMap::new();
        for cfg in configs {
            if deduped.contains_key(&cfg.unique_name) {
                continue;
            }
            let dup_path = deduped.values().any(|other| {
                other.matched_camera_info.camera_type() == cfg.matched_camera_info.camera_type()
                    && other.matched_camera_info.unique_path()
                        == cfg.matched_camera_info.unique_path()
            });
            if !dup_path {
                deduped.insert(cfg.unique_name.clone(), cfg);
            }
        }

        for (_, cfg) in deduped {
            if cfg.deactivated {
                self.disabled_camera_configs
                    .insert(cfg.unique_name.clone(), cfg);
            } else {
                let src = Self::load_vision_source_from_cam_config(cfg);
                self.vmm.add_source(
                    src,
                    self.data_change_service.clone(),
                    self.snapshots_dir.clone(),
                );
            }
        }
    }

    pub fn assign_unmatched_camera(&mut self, camera_info: PVCameraInfo) -> String {
        let cfg = CameraConfiguration::new(camera_info);
        let uname = cfg.unique_name.clone();
        let src = Self::load_vision_source_from_cam_config(cfg);
        self.vmm.add_source(
            src,
            self.data_change_service.clone(),
            self.snapshots_dir.clone(),
        );
        uname
    }

    pub fn deactivate_vision_source(&mut self, unique_name: &str) -> bool {
        let Some(module) = self.vmm.remove_module(unique_name) else {
            return false;
        };
        let mut cfg = module.to_camera_config();
        cfg.deactivated = true;
        self.disabled_camera_configs
            .insert(cfg.unique_name.clone(), cfg);
        true
    }

    pub fn reactivate_disabled_camera_config(&mut self, unique_name: &str) -> bool {
        let Some(mut cfg) = self.disabled_camera_configs.remove(unique_name) else {
            return false;
        };
        cfg.deactivated = false;
        let src = Self::load_vision_source_from_cam_config(cfg);
        self.vmm.add_source(
            src,
            self.data_change_service.clone(),
            self.snapshots_dir.clone(),
        );
        true
    }

    pub fn delete_camera(&mut self, unique_name: &str) -> bool {
        if self.vmm.remove_module(unique_name).is_some() {
            return true;
        }
        self.disabled_camera_configs.remove(unique_name).is_some()
    }

    pub fn get_state(&self) -> VisionSourceManagerState {
        let disabled_configs = self
            .disabled_camera_configs
            .values()
            .map(UICameraConfiguration::from)
            .collect();
        VisionSourceManagerState {
            disabled_configs,
            all_connected_cameras: self.connected_cameras.clone(),
        }
    }
}
