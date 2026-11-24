pub mod camera;
pub mod hardware;
pub mod migrations;
pub mod network;
pub mod neural_network;
pub mod sql;

pub use camera::*;
pub use hardware::*;
pub use migrations::*;
pub use network::*;
pub use neural_network::*;
pub use sql::*;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Root configuration aggregate (`org.photonvision.common.configuration.PhotonConfiguration`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotonConfiguration {
    pub hardware_config: HardwareConfig,
    pub hardware_settings: HardwareSettings,
    pub network_config: NetworkConfig,
    pub atfl: AprilTagFieldLayout,
    pub neural_network_properties: NeuralNetworkModelsSettings,
    pub camera_configurations: HashMap<String, CameraConfiguration>,
}

impl PhotonConfiguration {
    pub fn add_camera_config(&mut self, config: CameraConfiguration) {
        self.camera_configurations
            .insert(config.unique_name.clone(), config);
    }

    pub fn remove_camera_config(&mut self, unique_name: &str) -> Option<CameraConfiguration> {
        self.camera_configurations.remove(unique_name)
    }
}

/// Manages filesystem paths for configuration, logs, snapshots, and neural network models (`PathManager`).
#[derive(Debug, Clone)]
pub struct PathManager {
    root_dir: PathBuf,
}

impl Default for PathManager {
    fn default() -> Self {
        Self::new(
            std::env::var("PHOTONVISION_CONFIG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("photonvision_config")),
        )
    }
}

impl PathManager {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    pub fn config_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root_dir.join("logs")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.root_dir.join("models")
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.root_dir.join("imgSaves")
    }

    pub fn calib_imgs_dir(&self) -> PathBuf {
        self.root_dir.join("calibImgs")
    }
}
