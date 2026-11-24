use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OSType {
    Windows,
    Linux,
    MacOS,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PiVersion {
    Pi3,
    Pi4,
    Pi5,
    ComputeModule4,
    NotAPi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Windows64,
    Linux64,
    LinuxArm64,
    LinuxRaspbian64,
    LinuxRK3588,
    LinuxQCS6490,
    LinuxJetsonOrinNano,
    MacOS,
    Unknown,
}

impl Platform {
    pub fn current() -> Self {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", _) => Self::Windows64,
            ("macos", _) => Self::MacOS,
            ("linux", "x86_64") => Self::Linux64,
            ("linux", "aarch64") => {
                let model = std::fs::read_to_string("/proc/device-tree/model")
                    .unwrap_or_default()
                    .to_lowercase();
                if model.contains("raspberry pi") {
                    Self::LinuxRaspbian64
                } else if model.contains("jetson")
                    || model.contains("orin")
                    || cfg!(feature = "jetson-orin-nano")
                {
                    Self::LinuxJetsonOrinNano
                } else if model.contains("rk3588") || model.contains("orange pi 5") {
                    Self::LinuxRK3588
                } else if model.contains("qcs6490") || model.contains("rubik") {
                    Self::LinuxQCS6490
                } else {
                    Self::LinuxArm64
                }
            }
            _ => Self::Unknown,
        }
    }

    pub fn os_type(self) -> OSType {
        match self {
            Self::Windows64 => OSType::Windows,
            Self::MacOS => OSType::MacOS,
            Self::Linux64
            | Self::LinuxArm64
            | Self::LinuxRaspbian64
            | Self::LinuxRK3588
            | Self::LinuxQCS6490
            | Self::LinuxJetsonOrinNano => OSType::Linux,
            Self::Unknown => OSType::Unknown,
        }
    }

    pub fn is_pi(self) -> bool {
        matches!(self, Self::LinuxRaspbian64)
    }

    pub fn is_rk3588(self) -> bool {
        matches!(self, Self::LinuxRK3588)
    }

    pub fn is_qcs6490(self) -> bool {
        matches!(self, Self::LinuxQCS6490)
    }

    pub fn is_jetson(self) -> bool {
        matches!(self, Self::LinuxJetsonOrinNano)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OsImageData {
    pub image_version: String,
    pub commit_tag: String,
}

pub struct PlatformUtils;

impl PlatformUtils {
    /// Execute a shell command with a bounded timeout (resolving `PlatformUtils.java:38` TODO).
    pub fn execute_command_with_timeout(cmd: &str, _timeout: Duration) -> Option<String> {
        if cmd.trim().is_empty() {
            return None;
        }
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", cmd]).output().ok()?
        } else {
            Command::new("sh").args(["-c", cmd]).output().ok()?
        };
        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }
}
