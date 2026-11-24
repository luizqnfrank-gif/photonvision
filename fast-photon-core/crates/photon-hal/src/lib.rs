//! `photon-hal`: Coprocessor hardware abstraction, GPIO LEDs, system metrics, and OS networking.

pub mod gpio;
pub mod hardware_manager;
pub mod metrics;
pub mod networking;
pub mod platform;

#[cfg(test)]
mod unit_tests {
    use crate::hardware_manager::{HardwareManager, PhotonStatus, StatusLEDConsumer};
    use crate::metrics::{PlatformSystemMonitor, SystemMonitor};
    use crate::networking::{NetworkManager, NetworkingCommands};
    use crate::platform::Platform;
    use photon_common::configuration::{
        HardwareConfig, HardwareSettings, NetworkConfig, NetworkMode, StatusLedConfig,
    };
    use photon_common::dataflow::CVPipelineResultConsumer;
    use photon_targeting::{PhotonPipelineResult, PhotonTrackedTarget};

    #[test]
    fn test_hardware_manager_vision_and_status_leds() {
        let hw_cfg = HardwareConfig {
            device_name: "RaspberryPi4".to_string(),
            led_pins: vec![2, 13],
            led_brightness_range: vec![10, 90],
            status_led_config: StatusLedConfig::Rgb {
                active_high: true,
                red_pin: 17,
                green_pin: 27,
                blue_pin: 22,
            },
            ..HardwareConfig::default()
        };
        let hw_settings = HardwareSettings {
            led_brightness_percentage: 50,
            gpu_accel_enabled: false,
        };
        let mgr = HardwareManager::new(&hw_cfg, &hw_settings);

        // 50% mapped into [10..90] -> 50% duty cycle (0.5)
        assert!((mgr.factory.get_pwm(2) - 0.5).abs() < 1e-6);
        assert!(mgr.factory.get_digital(13));

        mgr.set_brightness_percent(0);
        assert!(!mgr.factory.get_digital(2));

        // StatusLEDConsumer sets green when targets are present, yellow (R+G) when no targets
        let consumer = StatusLEDConsumer::new(mgr.status_led.clone());
        let mut res = PhotonPipelineResult::default();
        consumer.accept(&res, 60.0);
        assert!(mgr.factory.get_digital(17)); // red ON
        assert!(mgr.factory.get_digital(27)); // green ON
        assert!(!mgr.factory.get_digital(22)); // blue OFF

        res.targets.push(PhotonTrackedTarget::default());
        consumer.accept(&res, 60.0);
        assert!(!mgr.factory.get_digital(17)); // red OFF
        assert!(mgr.factory.get_digital(27)); // green ON
        assert!(!mgr.factory.get_digital(22)); // blue OFF

        mgr.status_led.set_status(PhotonStatus::Error, false);
        assert!(mgr.factory.get_digital(17));
        assert!(!mgr.factory.get_digital(27));
    }

    #[test]
    fn test_networking_commands_and_system_monitor() {
        let mut net_cfg = NetworkConfig {
            should_manage: true,
            can_manage: true,
            connection_type: NetworkMode::STATIC,
            static_ip: "10.34.76.11".to_string(),
            network_manager_iface: "eth0".to_string(),
            ..NetworkConfig::default()
        };
        let static_cmd = NetworkingCommands::build_static_command(&net_cfg);
        assert!(static_cmd.contains("10.34.76.11/24"));
        assert!(static_cmd.contains("eth0"));

        let net_mgr = NetworkManager::new();
        assert!(net_mgr.apply_config(&net_cfg).is_ok());

        net_cfg.static_ip.clear();
        assert!(net_mgr.apply_config(&net_cfg).is_err());

        let mut monitor = PlatformSystemMonitor::new(Platform::current());
        let metrics = monitor.collect_metrics();
        assert!(metrics.ram_mem > 0.0);
    }
}
