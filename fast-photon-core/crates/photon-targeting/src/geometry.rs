use crate::packet::{Packet, PacketResult};
use nalgebra::{Isometry3, Translation3, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

/// 3D translation in meters (`org.wpilib.math.geometry.Translation3d`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Translation3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Translation3d {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn to_nalgebra(self) -> Translation3<f64> {
        Translation3::new(self.x, self.y, self.z)
    }

    pub fn from_nalgebra(t: Translation3<f64>) -> Self {
        Self {
            x: t.x,
            y: t.y,
            z: t.z,
        }
    }

    pub fn pack(&self, packet: &mut Packet) {
        packet.encode_f64(self.x);
        packet.encode_f64(self.y);
        packet.encode_f64(self.z);
    }

    pub fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            x: packet.decode_f64()?,
            y: packet.decode_f64()?,
            z: packet.decode_f64()?,
        })
    }
}

/// Unit quaternion (`org.wpilib.math.geometry.Quaternion`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quaternion {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Quaternion {
    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    pub fn to_nalgebra(self) -> UnitQuaternion<f64> {
        UnitQuaternion::from_quaternion(nalgebra::Quaternion::new(self.w, self.x, self.y, self.z))
    }

    pub fn from_nalgebra(uq: UnitQuaternion<f64>) -> Self {
        let q = uq.into_inner();
        Self {
            w: q.w,
            x: q.i,
            y: q.j,
            z: q.k,
        }
    }

    pub fn pack(&self, packet: &mut Packet) {
        packet.encode_f64(self.w);
        packet.encode_f64(self.x);
        packet.encode_f64(self.y);
        packet.encode_f64(self.z);
    }

    pub fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            w: packet.decode_f64()?,
            x: packet.decode_f64()?,
            y: packet.decode_f64()?,
            z: packet.decode_f64()?,
        })
    }
}

/// 3D rotation (`org.wpilib.math.geometry.Rotation3d`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rotation3d {
    pub quaternion: Quaternion,
}

impl Rotation3d {
    pub const fn new(quaternion: Quaternion) -> Self {
        Self { quaternion }
    }

    pub fn from_euler(roll: f64, pitch: f64, yaw: f64) -> Self {
        let uq = UnitQuaternion::from_euler_angles(roll, pitch, yaw);
        Self {
            quaternion: Quaternion::from_nalgebra(uq),
        }
    }

    pub fn from_rodrigues(rvec: Vector3<f64>) -> Self {
        let uq = UnitQuaternion::from_scaled_axis(rvec);
        Self {
            quaternion: Quaternion::from_nalgebra(uq),
        }
    }

    pub fn euler_angles(&self) -> (f64, f64, f64) {
        self.quaternion.to_nalgebra().euler_angles()
    }

    pub fn pack(&self, packet: &mut Packet) {
        self.quaternion.pack(packet);
    }

    pub fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            quaternion: Quaternion::unpack(packet)?,
        })
    }
}

/// 3D rigid transform (`org.wpilib.math.geometry.Transform3d`).
/// WPILib Struct layout is 56 bytes little-endian: `Translation3d (24B) + Rotation3d::Quaternion (32B)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Transform3d {
    pub translation: Translation3d,
    pub rotation: Rotation3d,
}

impl Transform3d {
    pub const STRUCT_SIZE: usize = 56;
    pub const STRUCT_SCHEMA: &'static str = "Translation3d translation;Rotation3d rotation";

    pub const fn new(translation: Translation3d, rotation: Rotation3d) -> Self {
        Self {
            translation,
            rotation,
        }
    }

    pub fn from_translation_euler(x: f64, y: f64, z: f64, roll: f64, pitch: f64, yaw: f64) -> Self {
        Self {
            translation: Translation3d::new(x, y, z),
            rotation: Rotation3d::from_euler(roll, pitch, yaw),
        }
    }

    pub fn to_isometry(self) -> Isometry3<f64> {
        Isometry3::from_parts(
            self.translation.to_nalgebra(),
            self.rotation.quaternion.to_nalgebra(),
        )
    }

    pub fn from_isometry(iso: Isometry3<f64>) -> Self {
        Self {
            translation: Translation3d::from_nalgebra(iso.translation),
            rotation: Rotation3d::new(Quaternion::from_nalgebra(iso.rotation)),
        }
    }

    pub fn inverse(&self) -> Self {
        Self::from_isometry(self.to_isometry().inverse())
    }

    pub fn plus(&self, other: &Self) -> Self {
        Self::from_isometry(self.to_isometry() * other.to_isometry())
    }

    pub fn pack(&self, packet: &mut Packet) {
        self.translation.pack(packet);
        self.rotation.pack(packet);
    }

    pub fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            translation: Translation3d::unpack(packet)?,
            rotation: Rotation3d::unpack(packet)?,
        })
    }

    pub fn to_struct_bytes(&self) -> [u8; Self::STRUCT_SIZE] {
        let mut pkt = Packet::with_capacity(Self::STRUCT_SIZE);
        self.pack(&mut pkt);
        let mut out = [0u8; Self::STRUCT_SIZE];
        out.copy_from_slice(pkt.as_slice());
        out
    }

    pub fn from_struct_bytes(bytes: &[u8]) -> PacketResult<Self> {
        let mut pkt = Packet::from_bytes(bytes);
        Self::unpack(&mut pkt)
    }
}

/// 3D pose (`org.wpilib.math.geometry.Pose3d`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Pose3d {
    pub translation: Translation3d,
    pub rotation: Rotation3d,
}

impl Pose3d {
    pub const fn new(translation: Translation3d, rotation: Rotation3d) -> Self {
        Self {
            translation,
            rotation,
        }
    }

    pub fn transform_by(&self, transform: &Transform3d) -> Self {
        let iso = Isometry3::from_parts(
            self.translation.to_nalgebra(),
            self.rotation.quaternion.to_nalgebra(),
        ) * transform.to_isometry();
        Self {
            translation: Translation3d::from_nalgebra(iso.translation),
            rotation: Rotation3d::new(Quaternion::from_nalgebra(iso.rotation)),
        }
    }
}
