use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait DigitalOutputDevice: Send + Sync {
    fn pin(&self) -> i32;
    fn set_state(&mut self, active: bool);
    fn get_state(&self) -> bool;
}

pub trait PwmOutputDevice: Send + Sync {
    fn pin(&self) -> i32;
    fn set_value(&mut self, duty_cycle: f64);
    fn get_value(&self) -> f64;
}

#[derive(Debug, Clone)]
pub struct SimulatedDigitalOutput {
    pin: i32,
    state: bool,
}

impl SimulatedDigitalOutput {
    pub const fn new(pin: i32) -> Self {
        Self { pin, state: false }
    }
}

impl DigitalOutputDevice for SimulatedDigitalOutput {
    fn pin(&self) -> i32 {
        self.pin
    }

    fn set_state(&mut self, active: bool) {
        self.state = active;
    }

    fn get_state(&self) -> bool {
        self.state
    }
}

#[derive(Debug, Clone)]
pub struct SimulatedPwmOutput {
    pin: i32,
    duty_cycle: f64,
}

impl SimulatedPwmOutput {
    pub const fn new(pin: i32) -> Self {
        Self {
            pin,
            duty_cycle: 0.0,
        }
    }
}

impl PwmOutputDevice for SimulatedPwmOutput {
    fn pin(&self) -> i32 {
        self.pin
    }

    fn set_value(&mut self, duty_cycle: f64) {
        self.duty_cycle = duty_cycle.clamp(0.0, 1.0);
    }

    fn get_value(&self) -> f64 {
        self.duty_cycle
    }
}

/// Custom GPIO adapter factory (`org.photonvision.common.hardware.gpio.CustomDeviceFactory`).
#[derive(Clone, Default)]
pub struct CustomDeviceFactory {
    digital_states: Arc<Mutex<HashMap<i32, bool>>>,
    pwm_values: Arc<Mutex<HashMap<i32, f64>>>,
}

impl CustomDeviceFactory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_digital(&self, pin: i32, state: bool) {
        if pin >= 0 {
            if let Ok(mut guard) = self.digital_states.lock() {
                guard.insert(pin, state);
            }
        }
    }

    pub fn get_digital(&self, pin: i32) -> bool {
        self.digital_states
            .lock()
            .ok()
            .and_then(|g| g.get(&pin).copied())
            .unwrap_or(false)
    }

    pub fn set_pwm(&self, pin: i32, duty_cycle: f64) {
        if pin >= 0 {
            if let Ok(mut guard) = self.pwm_values.lock() {
                guard.insert(pin, duty_cycle.clamp(0.0, 1.0));
            }
        }
    }

    pub fn get_pwm(&self, pin: i32) -> f64 {
        self.pwm_values
            .lock()
            .ok()
            .and_then(|g| g.get(&pin).copied())
            .unwrap_or(0.0)
    }
}
