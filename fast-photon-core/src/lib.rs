//! `fast-photon-core`: Pure-Rust migration of PhotonVision `photon-core`.

pub use photon_common as common;
pub use photon_hal as hal;
pub use photon_targeting as targeting;
pub use photon_vision as vision;
pub use photon_web as web;

pub const PHOTON_VERSION: &str = "2027.0.0-rust";

use anyhow::Result;
use clap::Parser;
use common::configuration::{ConfigProvider, PVCameraInfo, PathManager};
use common::dataflow::DataChangeService;
use common::logging::{LogGroup, Logger};
use hal::hardware_manager::HardwareManager;
use hal::networking::NetworkManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use vision::processes::VisionSourceManager;
use web::handlers::AppState;
use web::server::build_app_router;

#[derive(Debug, Parser)]
#[command(
    name = "fast-phontonvision-core",
    version,
    about = "Pure-Rust PhotonVision core engine"
)]
pub struct CliArgs {
    #[arg(long, default_value = "photonvision_config")]
    pub config_dir: PathBuf,

    #[arg(long, default_value_t = 5800)]
    pub port: u16,

    /// Register one or more TCP camera stream endpoints (e.g. tcp://192.168.1.50:8888)
    #[arg(long)]
    pub tcp_camera: Vec<String>,

    #[arg(long, default_value_t = false)]
    pub test_mode: bool,
}

pub async fn run_cli() -> Result<()> {
    Logger::init_global();
    let logger = Logger::new("Main", LogGroup::General);
    let args = CliArgs::parse();

    let path_manager = PathManager::new(&args.config_dir);
    let config_provider = ConfigProvider::new(path_manager.config_dir())?;
    let loaded_cfg = config_provider.get_config();

    let dc_service = DataChangeService::new();
    let hw_manager =
        HardwareManager::new(&loaded_cfg.hardware_config, &loaded_cfg.hardware_settings);
    let net_manager = NetworkManager::new();

    let mut vsm = VisionSourceManager::new(dc_service.clone(), path_manager.snapshots_dir());
    vsm.register_loaded_configs(loaded_cfg.camera_configurations.into_values());

    for (idx, uri) in args.tcp_camera.iter().enumerate() {
        let already_registered = vsm.vmm.modules().iter().any(|m| {
            m.vision_source
                .camera_configuration()
                .matched_camera_info
                .unique_path()
                == uri
        });
        if !already_registered {
            let name = format!("rpicam-tcp-{idx}");
            let cam_info = PVCameraInfo::tcp(uri.clone(), name, 1280, 720, 30);
            let uname = vsm.assign_unmatched_camera(cam_info);
            if let Some(m) = vsm.vmm.get_module(&uname) {
                let c = m.to_camera_config();
                config_provider.update_config(|cfg| {
                    cfg.add_camera_config(c);
                });
            }
            let _ = config_provider.save_to_disk();
        }
    }

    let state = AppState {
        config_provider,
        data_change_service: dc_service,
        vision_source_manager: Arc::new(Mutex::new(vsm)),
        hardware_manager: hw_manager,
        network_manager: net_manager,
    };

    if args.test_mode {
        logger.info("Running in test-mode (single startup validation pass).");
        return Ok(());
    }

    let app = build_app_router(state);
    let addr = format!("0.0.0.0:{}", args.port);
    logger.info(format!("Starting fast-photon-core server on http://{addr}"));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
