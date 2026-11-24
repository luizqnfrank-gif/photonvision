use crate::platform::Platform;
use photon_targeting::proto::ProtobufDeviceMetrics;
use serde::{Deserialize, Serialize};
use sysinfo::System;

/// Snapshot of coprocessor hardware metrics (`org.photonvision.common.hardware.metrics.DeviceMetrics`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMetrics {
    pub cpu_temp: f64,
    pub cpu_util: f64,
    pub cpu_thr: String,
    pub ram_mem: f64,
    pub ram_util: f64,
    pub gpu_mem: f64,
    pub gpu_mem_util: f64,
    pub disk_util_pct: f64,
    pub npu_usage: Vec<f64>,
    pub ip_address: String,
    pub uptime: f64,
    pub sent_bit_rate: f64,
    pub recv_bit_rate: f64,
    pub disk_usable_space: f64,
}

impl From<&DeviceMetrics> for ProtobufDeviceMetrics {
    fn from(m: &DeviceMetrics) -> Self {
        Self {
            cpu_temp: m.cpu_temp,
            cpu_util: m.cpu_util,
            cpu_thr: m.cpu_thr.clone(),
            ram_mem: m.ram_mem,
            ram_util: m.ram_util,
            gpu_mem: m.gpu_mem,
            gpu_mem_util: m.gpu_mem_util,
            disk_util_pct: m.disk_util_pct,
            npu_usage: m.npu_usage.clone(),
            ip_address: m.ip_address.clone(),
            uptime: m.uptime,
            sent_bit_rate: m.sent_bit_rate,
            recv_bit_rate: m.recv_bit_rate,
            disk_usable_space: m.disk_usable_space,
        }
    }
}

pub trait SystemMonitor: Send + Sync {
    fn collect_metrics(&mut self) -> DeviceMetrics;
}

/// Platform-aware system monitor (`SystemMonitorRaspberryPi`, `SystemMonitorRK3588`, `SystemMonitorQCS6490`, `SystemMonitorWindows`).
pub struct PlatformSystemMonitor {
    platform: Platform,
    sys: System,
}

impl Default for PlatformSystemMonitor {
    fn default() -> Self {
        Self::new(Platform::current())
    }
}

impl PlatformSystemMonitor {
    pub fn new(platform: Platform) -> Self {
        let mut sys = System::new_all();
        sys.refresh_memory();
        sys.refresh_cpu_usage();
        Self { platform, sys }
    }

    fn read_thermal_zone0_celsius() -> f64 {
        std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
            .ok()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .map(|millideg| millideg / 1000.0)
            .unwrap_or(-1.0)
    }

    fn read_rk3588_npu_usage() -> Vec<f64> {
        if let Ok(content) = std::fs::read_to_string("/sys/kernel/debug/rknpu/load") {
            let mut out = Vec::new();
            for part in content.split('%') {
                if let Some(num_str) = part.split(':').next_back() {
                    if let Ok(v) = num_str.trim().parse::<f64>() {
                        out.push(v);
                    }
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        Vec::new()
    }

    fn read_jetson_gpu_utilization() -> f64 {
        // NVIDIA Tegra Orin sysfs GPU load is reported in tenths of a percent (0..1000)
        for path in [
            "/sys/devices/platform/bus@0/17000000.ga10b/devfreq/17000000.ga10b/cur_freq",
            "/sys/devices/gpu.0/load",
            "/sys/devices/platform/17000000.ga10b/load",
        ] {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(val) = s.trim().parse::<f64>() {
                    if path.ends_with("load") {
                        return (val / 10.0).clamp(0.0, 100.0);
                    }
                }
            }
        }
        0.0
    }
}

impl SystemMonitor for PlatformSystemMonitor {
    fn collect_metrics(&mut self) -> DeviceMetrics {
        self.sys.refresh_memory();
        self.sys.refresh_cpu_usage();

        let total_mem_mb = self.sys.total_memory() as f64 / (1024.0 * 1024.0);
        let used_mem_mb = self.sys.used_memory() as f64 / (1024.0 * 1024.0);
        let cpu_util = f64::from(self.sys.global_cpu_usage());
        let uptime = System::uptime() as f64;
        let cpu_temp = Self::read_thermal_zone0_celsius();

        let npu_usage = if self.platform.is_rk3588() {
            Self::read_rk3588_npu_usage()
        } else {
            Vec::new()
        };

        let gpu_mem_util = if self.platform.is_jetson() {
            Self::read_jetson_gpu_utilization()
        } else {
            0.0
        };

        DeviceMetrics {
            cpu_temp,
            cpu_util,
            cpu_thr: "None".to_string(),
            ram_mem: total_mem_mb,
            ram_util: used_mem_mb,
            gpu_mem: 0.0,
            gpu_mem_util,
            disk_util_pct: 0.0,
            npu_usage,
            ip_address: "127.0.0.1".to_string(),
            uptime,
            sent_bit_rate: 0.0,
            recv_bit_rate: 0.0,
            disk_usable_space: 0.0,
        }
    }
}
