use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NetworkMode {
    #[default]
    DHCP,
    STATIC,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    #[serde(default = "default_nt_server")]
    pub nt_server_address: String,
    #[serde(default)]
    pub connection_type: NetworkMode,
    #[serde(default)]
    pub static_ip: String,
    #[serde(default = "default_hostname")]
    pub hostname: String,
    #[serde(default, rename = "runNTServer")]
    pub run_nt_server: bool,
    #[serde(default)]
    pub should_manage: bool,
    #[serde(default = "default_true")]
    pub should_publish_proto: bool,
    #[serde(default)]
    pub can_manage: bool,
    #[serde(default)]
    pub network_manager_iface: String,
    #[serde(default)]
    pub set_static_command: String,
    #[serde(default)]
    pub set_dhcp_command: String,
}

fn default_nt_server() -> String {
    "0".to_string()
}

fn default_hostname() -> String {
    "photonvision".to_string()
}

const fn default_true() -> bool {
    true
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            nt_server_address: default_nt_server(),
            connection_type: NetworkMode::DHCP,
            static_ip: String::new(),
            hostname: default_hostname(),
            run_nt_server: false,
            should_manage: false,
            should_publish_proto: true,
            can_manage: true,
            network_manager_iface: String::new(),
            set_static_command: String::new(),
            set_dhcp_command: String::new(),
        }
    }
}
