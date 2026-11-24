use nalgebra::{Matrix3, UnitQuaternion};
use photon_targeting::{Quaternion, Rotation3d, Transform3d, Translation3d};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Generic pair of numbers (`org.photonvision.common.util.numbers.NumberCouple`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberCouple<T> {
    pub first: T,
    pub second: T,
}

impl<T> NumberCouple<T> {
    pub const fn new(first: T, second: T) -> Self {
        Self { first, second }
    }

    pub fn is_empty_with(&self, zero: T) -> bool
    where
        T: PartialEq,
    {
        self.first == zero && self.second == zero
    }
}

pub type IntegerCouple = NumberCouple<i32>;
pub type DoubleCouple = NumberCouple<f64>;

/// Periodic task scheduler (`org.photonvision.common.util.TimedTaskManager`).
#[derive(Default, Clone)]
pub struct TimedTaskManager {
    tasks: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl TimedTaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_task<F>(&self, name: impl Into<String>, mut callback: F, interval_millis: u64)
    where
        F: FnMut() + Send + 'static,
    {
        let name_str = name.into();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(interval_millis.max(1)));
            loop {
                ticker.tick().await;
                callback();
            }
        });
        if let Ok(mut guard) = self.tasks.lock() {
            if let Some(old) = guard.insert(name_str, handle) {
                old.abort();
            }
        }
    }

    pub fn cancel_task(&self, name: &str) -> bool {
        if let Ok(mut guard) = self.tasks.lock() {
            if let Some(handle) = guard.remove(name) {
                handle.abort();
                return true;
            }
        }
        false
    }

    pub fn cancel_all(&self) {
        if let Ok(mut guard) = self.tasks.lock() {
            for (_, handle) in guard.drain() {
                handle.abort();
            }
        }
    }
}

/// Math utilities and coordinate frame conversions (`org.photonvision.common.util.math.MathUtils`).
pub struct MathUtils;

impl MathUtils {
    pub fn clamp<T: PartialOrd>(val: T, min: T, max: T) -> T {
        if val < min {
            min
        } else if val > max {
            max
        } else {
            val
        }
    }

    pub fn map(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
        if (in_max - in_min).abs() < f64::EPSILON {
            return out_min;
        }
        (value - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
    }

    pub fn lerp(start: f64, end: f64, t: f64) -> f64 {
        start + (end - start) * t
    }

    /// Converts OpenCV EDN (East-Down-North: +X right, +Y down, +Z forward)
    /// to WPILib NWU (North-West-Up: +X forward, +Y left, +Z up).
    pub fn convert_opencv_to_nwu(in_edn: &Transform3d) -> Transform3d {
        // Basis matrix C where v_nwu = C * v_edn:
        // x_nwu = z_edn, y_nwu = -x_edn, z_nwu = -y_edn
        let c = Matrix3::new(0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, -1.0, 0.0);
        let c_rot = UnitQuaternion::from_matrix(&c);
        let t_edn = in_edn.translation;
        let t_nwu = Translation3d::new(t_edn.z, -t_edn.x, -t_edn.y);
        let q_edn = in_edn.rotation.quaternion.to_nalgebra();
        let q_nwu = c_rot * q_edn * c_rot.inverse();
        Transform3d::new(t_nwu, Rotation3d::new(Quaternion::from_nalgebra(q_nwu)))
    }

    /// Converts WPILib NWU to OpenCV EDN.
    pub fn convert_nwu_to_opencv(in_nwu: &Transform3d) -> Transform3d {
        // x_edn = -y_nwu, y_edn = -z_nwu, z_edn = x_nwu
        let c_inv = Matrix3::new(0.0, -1.0, 0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0);
        let c_inv_rot = UnitQuaternion::from_matrix(&c_inv);
        let t_nwu = in_nwu.translation;
        let t_edn = Translation3d::new(-t_nwu.y, -t_nwu.z, t_nwu.x);
        let q_nwu = in_nwu.rotation.quaternion.to_nalgebra();
        let q_edn = c_inv_rot * q_nwu * c_inv_rot.inverse();
        Transform3d::new(t_edn, Rotation3d::new(Quaternion::from_nalgebra(q_edn)))
    }

    /// Converts WPILib AprilTag pose estimate (+X right, +Y down, +Z into tag) to PhotonVision NWU camera-to-target.
    pub fn convert_apriltag_to_nwu(apriltag_pose: &Transform3d) -> Transform3d {
        let edn_to_nwu = Self::convert_opencv_to_nwu(apriltag_pose);
        // Rotate target frame by pi around Z/Y to face camera per PhotonVision convention
        edn_to_nwu
    }
}

/// Color conversion helper (`org.photonvision.common.util.ColorHelper`).
pub struct ColorHelper;

impl ColorHelper {
    /// Convert RGB `[0..255]` to OpenCV HSV `[H: 0..180, S: 0..255, V: 0..255]`.
    pub fn rgb_to_opencv_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let rf = f64::from(r) / 255.0;
        let gf = f64::from(g) / 255.0;
        let bf = f64::from(b) / 255.0;
        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);
        let delta = max - min;

        let h_deg = if delta.abs() < 1e-9 {
            0.0
        } else if (max - rf).abs() < 1e-9 {
            60.0 * (((gf - bf) / delta).rem_euclid(6.0))
        } else if (max - gf).abs() < 1e-9 {
            60.0 * (((bf - rf) / delta) + 2.0)
        } else {
            60.0 * (((rf - gf) / delta) + 4.0)
        };

        let s = if max.abs() < 1e-9 { 0.0 } else { delta / max };
        let v = max;

        (
            (h_deg / 2.0).round().clamp(0.0, 180.0) as u8,
            (s * 255.0).round().clamp(0.0, 255.0) as u8,
            (v * 255.0).round().clamp(0.0, 255.0) as u8,
        )
    }
}
