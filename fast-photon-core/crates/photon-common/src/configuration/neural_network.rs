use photon_targeting::Pose3d;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct FieldDimensions {
    #[serde(default = "default_field_length")]
    pub length: f64,
    #[serde(default = "default_field_width")]
    pub width: f64,
}

const fn default_field_length() -> f64 {
    16.54175
}

const fn default_field_width() -> f64 {
    8.0137
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AprilTagFieldEntry {
    #[serde(rename = "ID")]
    pub id: i32,
    pub pose: Pose3d,
}

/// AprilTag field layout (`org.wpilib.fields.Field` / 2027 `fieldLayout` format).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AprilTagFieldLayout {
    #[serde(default, rename = "field-dimensions", alias = "field")]
    pub field_dimensions: FieldDimensions,
    #[serde(default, rename = "field-tags", alias = "tags")]
    pub field_tags: Vec<AprilTagFieldEntry>,
}

impl AprilTagFieldLayout {
    pub fn get_tag_pose(&self, id: i32) -> Option<Pose3d> {
        self.field_tags.iter().find(|t| t.id == id).map(|t| t.pose)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProperties {
    pub model_path: PathBuf,
    pub nickname: String,
    pub labels: Vec<String>,
    pub resolution_width: i32,
    pub resolution_height: i32,
    pub family: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeuralNetworkModelsSettings {
    #[serde(default)]
    pub models: HashMap<String, Vec<ModelProperties>>,
}

/// Manages `.rknn` and `.tflite` object detection models (`org.photonvision.common.configuration.NeuralNetworkModelManager`).
#[derive(Debug, Clone, Default)]
pub struct NeuralNetworkModelManager {
    pub settings: NeuralNetworkModelsSettings,
    pub models_dir: PathBuf,
}

impl NeuralNetworkModelManager {
    pub fn new(models_dir: impl Into<PathBuf>) -> Self {
        Self {
            settings: NeuralNetworkModelsSettings::default(),
            models_dir: models_dir.into(),
        }
    }

    pub fn discover_models(&mut self) -> std::io::Result<usize> {
        if !self.models_dir.exists() {
            return Ok(0);
        }
        let mut count = 0usize;
        for entry in std::fs::read_dir(&self.models_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "rknn" || ext == "tflite" {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("model")
                        .to_string();
                    let backend_key = ext.to_uppercase();
                    let list = self.settings.models.entry(backend_key).or_default();
                    if !list.iter().any(|m| m.nickname == stem) {
                        list.push(ModelProperties {
                            model_path: path.clone(),
                            nickname: stem,
                            labels: vec!["target".to_string()],
                            resolution_width: 640,
                            resolution_height: 640,
                            family: "YOLO".to_string(),
                            version: "v8".to_string(),
                        });
                    }
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    pub fn delete_model(&mut self, nickname: &str) -> bool {
        let mut removed = false;
        for list in self.settings.models.values_mut() {
            let before = list.len();
            list.retain(|m| m.nickname != nickname);
            if list.len() != before {
                removed = true;
            }
        }
        removed
    }

    pub fn rename_model(&mut self, old_name: &str, new_name: &str) -> bool {
        let mut renamed = false;
        for list in self.settings.models.values_mut() {
            for model in list.iter_mut() {
                if model.nickname == old_name {
                    model.nickname = new_name.to_string();
                    renamed = true;
                }
            }
        }
        renamed
    }

    pub fn import_model_file(&mut self, src: &Path, nickname: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.models_dir)?;
        let file_name = src
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("model.tflite"));
        let dst = self.models_dir.join(file_name);
        if src != dst {
            std::fs::copy(src, &dst)?;
        }
        let ext = dst
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("tflite")
            .to_uppercase();
        self.settings
            .models
            .entry(ext)
            .or_default()
            .push(ModelProperties {
                model_path: dst,
                nickname: nickname.to_string(),
                labels: vec!["target".to_string()],
                resolution_width: 640,
                resolution_height: 640,
                family: "YOLO".to_string(),
                version: "v8".to_string(),
            });
        Ok(())
    }
}
