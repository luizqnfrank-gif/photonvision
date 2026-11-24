use crate::platform::{Platform, PlatformUtils};
use photon_common::configuration::{NetworkConfig, NetworkMode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NMDeviceInfo {
    pub dev_name: String,
    pub conn_name: String,
    pub ip_address: String,
}

/// Command generator for Linux `nmcli` networking (`org.photonvision.common.networking.NetworkingCommands`).
pub struct NetworkingCommands;

impl NetworkingCommands {
    pub fn build_dhcp_command(config: &NetworkConfig) -> String {
        if !config.set_dhcp_command.is_empty() {
            return config
                .set_dhcp_command
                .replace("${interface}", &config.network_manager_iface);
        }
        format!(
            "nmcli con mod \"{}\" ipv4.method \"auto\" ipv4.addresses \"\" && nmcli con up \"{}\"",
            config.network_manager_iface, config.network_manager_iface
        )
    }

    pub fn build_static_command(config: &NetworkConfig) -> String {
        if !config.set_static_command.is_empty() {
            return config
                .set_static_command
                .replace("${interface}", &config.network_manager_iface)
                .replace("${ip}", &config.static_ip);
        }
        format!(
            "nmcli con mod \"{}\" ipv4.addresses \"{}/24\" ipv4.method \"manual\" && nmcli con up \"{}\"",
            config.network_manager_iface, config.static_ip, config.network_manager_iface
        )
    }

    pub fn build_hostname_command(hostname: &str) -> String {
        format!("hostnamectl set-hostname \"{hostname}\"")
    }
}

/// Manages OS network interfaces and applies `NetworkConfig` (`org.photonvision.common.networking.NetworkManager`).
#[derive(Debug, Clone, Default)]
pub struct NetworkManager {
    pub is_disabled: bool,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self { is_disabled: false }
    }

    pub fn apply_config(&self, config: &NetworkConfig) -> Result<String, String> {
        if self.is_disabled || !config.should_manage || !config.can_manage {
            return Ok("Network management disabled or unmanaged".to_string());
        }

        let cmd = match config.connection_type {
            NetworkMode::DHCP => NetworkingCommands::build_dhcp_command(config),
            NetworkMode::STATIC => {
                if config.static_ip.trim().is_empty() {
                    return Err("Static IP cannot be empty in STATIC mode".to_string());
                }
                NetworkingCommands::build_static_command(config)
            }
        };

        if matches!(
            Platform::current(),
            Platform::LinuxRaspbian64
                | Platform::LinuxRK3588
                | Platform::LinuxQCS6490
                | Platform::LinuxJetsonOrinNano
        ) {
            let _ = PlatformUtils::execute_command_with_timeout(&cmd, Duration::from_secs(5));
        }

        Ok(cmd)
    }
}
