use crate::gpio::CustomDeviceFactory;
use photon_common::configuration::{HardwareConfig, HardwareSettings, StatusLedConfig};
use photon_common::dataflow::CVPipelineResultConsumer;
use photon_common::util::MathUtils;
use photon_targeting::PhotonPipelineResult;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisionLEDMode {
    Default = -1,
    Off = 0,
    On = 1,
    Blink = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhotonStatus {
    Booting,
    RunningNoTargets,
    RunningWithTargets,
    Error,
}

/// Controls vision illumination LEDs (`org.photonvision.common.hardware.VisionLED`).
#[derive(Clone)]
pub struct VisionLED {
    pins: Vec<i32>,
    brightness_min: i32,
    brightness_max: i32,
    factory: CustomDeviceFactory,
    current_percentage: Arc<Mutex<i32>>,
}

impl VisionLED {
    pub fn new(
        pins: Vec<i32>,
        brightness_min: i32,
        brightness_max: i32,
        factory: CustomDeviceFactory,
    ) -> Self {
        Self {
            pins,
            brightness_min,
            brightness_max,
            factory,
            current_percentage: Arc::new(Mutex::new(0)),
        }
    }

    pub fn set_brightness_percentage(&self, percent: i32) {
        let clamped = percent.clamp(0, 100);
        if let Ok(mut guard) = self.current_percentage.lock() {
            *guard = clamped;
        }
        let mapped = MathUtils::map(
            f64::from(clamped),
            0.0,
            100.0,
            f64::from(self.brightness_min),
            f64::from(self.brightness_max),
        ) / 100.0;
        for &pin in &self.pins {
            self.factory.set_pwm(pin, mapped);
            self.factory.set_digital(pin, clamped > 0);
        }
    }

    pub fn get_brightness_percentage(&self) -> i32 {
        self.current_percentage.lock().map(|g| *g).unwrap_or(0)
    }

    pub fn set_state(&self, on: bool) {
        self.set_brightness_percentage(if on { 100 } else { 0 });
    }
}

/// Controls RGB or Green/Yellow status indicator LEDs (`org.photonvision.common.hardware.statusLED.StatusLED`).
#[derive(Clone)]
pub struct StatusLED {
    config: StatusLedConfig,
    factory: CustomDeviceFactory,
}

impl StatusLED {
    pub fn new(config: StatusLedConfig, factory: CustomDeviceFactory) -> Self {
        Self { config, factory }
    }

    pub fn set_rgb(&self, r: bool, g: bool, b: bool) {
        match &self.config {
            StatusLedConfig::Rgb {
                active_high,
                red_pin,
                green_pin,
                blue_pin,
            } => {
                let norm = |v: bool| if *active_high { v } else { !v };
                self.factory.set_digital(*red_pin, norm(r));
                self.factory.set_digital(*green_pin, norm(g));
                self.factory.set_digital(*blue_pin, norm(b));
            }
            StatusLedConfig::GreenYellow {
                active_high,
                green_pin,
                yellow_pin,
            } => {
                let norm = |v: bool| if *active_high { v } else { !v };
                self.factory.set_digital(*green_pin, norm(g));
                self.factory.set_digital(*yellow_pin, norm(r || b));
            }
            StatusLedConfig::Unconfigured => {}
        }
    }

    pub fn set_status(&self, status: PhotonStatus, has_targets: bool) {
        match status {
            PhotonStatus::Booting => self.set_rgb(false, false, true),
            PhotonStatus::RunningWithTargets => self.set_rgb(false, true, false),
            PhotonStatus::RunningNoTargets => {
                if has_targets {
                    self.set_rgb(false, true, false);
                } else {
                    self.set_rgb(true, true, false);
                }
            }
            PhotonStatus::Error => self.set_rgb(true, false, false),
        }
    }
}

/// Pipeline result consumer updating status LEDs (`org.photonvision.common.dataflow.statusLEDs.StatusLEDConsumer`).
#[derive(Clone)]
pub struct StatusLEDConsumer {
    status_led: StatusLED,
}

impl StatusLEDConsumer {
    pub fn new(status_led: StatusLED) -> Self {
        Self { status_led }
    }
}

impl CVPipelineResultConsumer for StatusLEDConsumer {
    fn accept(&self, result: &PhotonPipelineResult, _fps: f64) {
        let status = if result.has_targets() {
            PhotonStatus::RunningWithTargets
        } else {
            PhotonStatus::RunningNoTargets
        };
        self.status_led.set_status(status, result.has_targets());
    }
}

/// Hardware manager singleton/facade (`org.photonvision.common.hardware.HardwareManager`).
#[derive(Clone)]
pub struct HardwareManager {
    pub vision_led: Option<VisionLED>,
    pub status_led: StatusLED,
    pub factory: CustomDeviceFactory,
}

impl Default for HardwareManager {
    fn default() -> Self {
        Self::new(&HardwareConfig::default(), &HardwareSettings::default())
    }
}

impl HardwareManager {
    pub fn new(config: &HardwareConfig, settings: &HardwareSettings) -> Self {
        let factory = CustomDeviceFactory::new();
        let (min_b, max_b) = if config.led_brightness_range.len() >= 2 {
            (
                config.led_brightness_range[0],
                config.led_brightness_range[1],
            )
        } else {
            (0, 100)
        };
        let vision_led = if config.led_pins.is_empty() {
            None
        } else {
            let led = VisionLED::new(config.led_pins.clone(), min_b, max_b, factory.clone());
            led.set_brightness_percentage(settings.led_brightness_percentage);
            Some(led)
        };
        let status_led = StatusLED::new(config.status_led_config.clone(), factory.clone());

        Self {
            vision_led,
            status_led,
            factory,
        }
    }

    pub fn set_brightness_percent(&self, percent: i32) {
        if let Some(led) = &self.vision_led {
            led.set_brightness_percentage(percent);
        }
    }

    pub fn enable_vision_led(&self, enable: bool) {
        if let Some(led) = &self.vision_led {
            led.set_state(enable);
        }
    }
}
