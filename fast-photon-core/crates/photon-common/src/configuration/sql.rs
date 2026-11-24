use crate::configuration::camera::CameraConfiguration;
use crate::configuration::hardware::{HardwareConfig, HardwareSettings};
use crate::configuration::migrations::{MigrationManager, LATEST_SCHEMA_VERSION};
use crate::configuration::network::NetworkConfig;
use crate::configuration::neural_network::{AprilTagFieldLayout, NeuralNetworkModelsSettings};
use crate::configuration::PhotonConfiguration;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct GlobalKeys;

impl GlobalKeys {
    pub const NETWORK_CONFIG: &'static str = "networkConfig";
    pub const HARDWARE_CONFIG: &'static str = "hardwareConfig";
    pub const HARDWARE_SETTINGS: &'static str = "hardwareSettings";
    pub const FIELD_CONFIG: &'static str = "fieldLayout";
    pub const NEURAL_NETWORK_PROPERTIES: &'static str = "neuralNetworkProperties";
}

/// SQLite-backed configuration provider (`org.photonvision.common.configuration.ConfigProvider`).
#[derive(Clone)]
pub struct ConfigProvider {
    root_dir: PathBuf,
    db_path: PathBuf,
    config: Arc<Mutex<PhotonConfiguration>>,
}

impl ConfigProvider {
    pub const DEFAULT_DB_NAME: &'static str = "photon.sqlite";

    pub fn new(root_dir: impl Into<PathBuf>) -> Result<Self> {
        Self::with_db_name(root_dir, Self::DEFAULT_DB_NAME)
    }

    pub fn with_db_name(root_dir: impl Into<PathBuf>, db_name: &str) -> Result<Self> {
        let root_dir = root_dir.into();
        std::fs::create_dir_all(&root_dir)
            .with_context(|| format!("failed to create config root dir {:?}", root_dir))?;
        let db_path = root_dir.join(db_name);

        let mut provider = Self {
            root_dir,
            db_path,
            config: Arc::new(Mutex::new(PhotonConfiguration::default())),
        };
        provider.init_database()?;
        provider.load()?;
        Ok(provider)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn get_config(&self) -> PhotonConfiguration {
        self.config.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn update_config<F>(&self, f: F)
    where
        F: FnOnce(&mut PhotonConfiguration),
    {
        if let Ok(mut guard) = self.config.lock() {
            f(&mut guard);
        }
    }

    pub fn init_database(&mut self) -> Result<i32> {
        let mut conn = Connection::open(&self.db_path)
            .with_context(|| format!("failed to open sqlite db at {:?}", self.db_path))?;
        let ver = MigrationManager::migrate_to_latest(&mut conn)?;
        debug_assert_eq!(ver, LATEST_SCHEMA_VERSION);
        Ok(ver)
    }

    pub fn load(&mut self) -> Result<PhotonConfiguration> {
        let mut conn = Connection::open(&self.db_path)?;
        MigrationManager::migrate_to_latest(&mut conn)?;

        let read_global = |key: &str| -> Option<String> {
            conn.query_row(
                "SELECT contents FROM global WHERE config_name = ?1;",
                [key],
                |row| row.get(0),
            )
            .ok()
        };

        let hardware_config: HardwareConfig = read_global(GlobalKeys::HARDWARE_CONFIG)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let hardware_settings: HardwareSettings = read_global(GlobalKeys::HARDWARE_SETTINGS)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let network_config: NetworkConfig = read_global(GlobalKeys::NETWORK_CONFIG)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let atfl: AprilTagFieldLayout = read_global(GlobalKeys::FIELD_CONFIG)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        let neural_network_properties: NeuralNetworkModelsSettings =
            read_global(GlobalKeys::NEURAL_NETWORK_PROPERTIES)
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

        let mut camera_configurations = HashMap::new();
        let mut stmt = conn.prepare("SELECT unique_name, contents FROM cameras;")?;
        let cam_rows = stmt.query_map([], |row| {
            let uname: String = row.get(0)?;
            let contents: String = row.get(1)?;
            Ok((uname, contents))
        })?;

        for row in cam_rows.flatten() {
            let (uname, json_str) = row;
            if let Ok(mut cam_cfg) = serde_json::from_str::<CameraConfiguration>(&json_str) {
                if cam_cfg.unique_name.is_empty() {
                    cam_cfg.unique_name = uname.clone();
                }
                camera_configurations.insert(uname, cam_cfg);
            }
        }

        let loaded = PhotonConfiguration {
            hardware_config,
            hardware_settings,
            network_config,
            atfl,
            neural_network_properties,
            camera_configurations,
        };

        if let Ok(mut guard) = self.config.lock() {
            *guard = loaded.clone();
        }
        Ok(loaded)
    }

    pub fn save_to_disk(&self) -> Result<()> {
        let cfg = self.get_config();
        let mut conn = Connection::open(&self.db_path)?;
        let tx = conn.transaction()?;

        let write_global = |key: &str, json_str: String| -> rusqlite::Result<()> {
            tx.execute(
                "REPLACE INTO global (config_name, contents) VALUES (?1, ?2);",
                (key, json_str),
            )?;
            Ok(())
        };

        write_global(
            GlobalKeys::HARDWARE_CONFIG,
            serde_json::to_string(&cfg.hardware_config)?,
        )?;
        write_global(
            GlobalKeys::HARDWARE_SETTINGS,
            serde_json::to_string(&cfg.hardware_settings)?,
        )?;
        write_global(
            GlobalKeys::NETWORK_CONFIG,
            serde_json::to_string(&cfg.network_config)?,
        )?;
        write_global(GlobalKeys::FIELD_CONFIG, serde_json::to_string(&cfg.atfl)?)?;
        write_global(
            GlobalKeys::NEURAL_NETWORK_PROPERTIES,
            serde_json::to_string(&cfg.neural_network_properties)?,
        )?;

        tx.execute("DELETE FROM cameras;", [])?;
        for (uname, cam_cfg) in &cfg.camera_configurations {
            let cam_json = serde_json::to_string(cam_cfg)?;
            tx.execute(
                "REPLACE INTO cameras (unique_name, contents) VALUES (?1, ?2);",
                (uname, cam_json),
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn export_settings_zip(&self, out_zip_path: &Path) -> Result<()> {
        self.save_to_disk()?;
        let file = File::create(out_zip_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        if self.db_path.exists() {
            let file_name = self
                .db_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(Self::DEFAULT_DB_NAME);
            zip.start_file(file_name, options)?;
            let mut db_file = File::open(&self.db_path)?;
            let mut buf = Vec::new();
            db_file.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
        zip.finish()?;
        Ok(())
    }

    pub fn save_uploaded_settings_zip(&mut self, upload_zip_path: &Path) -> Result<()> {
        let file = File::open(upload_zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        std::fs::create_dir_all(&self.root_dir)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let Some(enclosed) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let out_path = self.root_dir.join(enclosed);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out_file = File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out_file)?;
            }
        }
        self.load()?;
        Ok(())
    }
}
