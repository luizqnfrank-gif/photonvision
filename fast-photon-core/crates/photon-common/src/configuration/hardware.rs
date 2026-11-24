use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StatusLedConfig {
    #[serde(rename = "RGB")]
    Rgb {
        #[serde(rename = "activeHigh", default)]
        active_high: bool,
        #[serde(rename = "redPin", default = "default_pin")]
        red_pin: i32,
        #[serde(rename = "greenPin", default = "default_pin")]
        green_pin: i32,
        #[serde(rename = "bluePin", default = "default_pin")]
        blue_pin: i32,
    },
    #[serde(rename = "GreenYellow")]
    GreenYellow {
        #[serde(rename = "activeHigh", default)]
        active_high: bool,
        #[serde(rename = "greenPin", default = "default_pin")]
        green_pin: i32,
        #[serde(rename = "yellowPin", default = "default_pin")]
        yellow_pin: i32,
    },
    #[serde(other)]
    Unconfigured,
}

const fn default_pin() -> i32 {
    -1
}

impl Default for StatusLedConfig {
    fn default() -> Self {
        Self::Rgb {
            active_high: false,
            red_pin: -1,
            green_pin: -1,
            blue_pin: -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareConfig {
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub device_logo_path: String,
    #[serde(default)]
    pub support_url: String,
    #[serde(default)]
    pub led_pins: Vec<i32>,
    #[serde(default)]
    pub leds_can_dim: bool,
    #[serde(default)]
    pub led_brightness_range: Vec<i32>,
    #[serde(default, rename = "ledPWMFrequency")]
    pub led_pwm_frequency: i32,
    #[serde(default, rename = "statusLEDConfig")]
    pub status_led_config: StatusLedConfig,
    #[serde(default)]
    pub cpu_temp_command: String,
    #[serde(default)]
    pub cpu_memory_command: String,
    #[serde(default)]
    pub cpu_utilization_command: String,
    #[serde(default)]
    pub cpu_throttle_reason_command: String,
    #[serde(default)]
    pub cpu_uptime_command: String,
    #[serde(default)]
    pub gpu_memory_command: String,
    #[serde(default)]
    pub ram_utilization_command: String,
    #[serde(default)]
    pub restart_hardware_command: String,
    #[serde(default)]
    pub disk_usage_command: String,
    #[serde(default)]
    pub vendor_fov: f64,
    #[serde(default)]
    pub blacklisted_i2c_devices: Vec<i32>,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            device_logo_path: String::new(),
            support_url: String::new(),
            led_pins: Vec::new(),
            leds_can_dim: true,
            led_brightness_range: vec![0, 100],
            led_pwm_frequency: 0,
            status_led_config: StatusLedConfig::default(),
            cpu_temp_command: String::new(),
            cpu_memory_command: String::new(),
            cpu_utilization_command: String::new(),
            cpu_throttle_reason_command: String::new(),
            cpu_uptime_command: String::new(),
            gpu_memory_command: String::new(),
            ram_utilization_command: String::new(),
            restart_hardware_command: String::new(),
            disk_usage_command: String::new(),
            vendor_fov: -1.0,
            blacklisted_i2c_devices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSettings {
    #[serde(default = "default_led_brightness", rename = "ledBrightnessPercentage")]
    pub led_brightness_percentage: i32,
    #[serde(default)]
    pub gpu_accel_enabled: bool,
}

const fn default_led_brightness() -> i32 {
    100
}

impl Default for HardwareSettings {
    fn default() -> Self {
        Self {
            led_brightness_percentage: 100,
            gpu_accel_enabled: false,
        }
    }
}
