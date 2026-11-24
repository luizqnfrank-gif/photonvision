//! `photon-targeting`: Wire formats, 3D geometry, and target estimation math.

pub mod estimation;
pub mod geometry;
pub mod packet;
pub mod proto;
pub mod result;
pub mod targets;

pub use estimation::*;
pub use geometry::*;
pub use packet::*;
pub use result::*;
pub use targets::*;

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_packet_underflow_and_invalid_bool_errors() {
        let mut pkt = Packet::from_bytes(vec![2u8]);
        assert_eq!(pkt.decode_bool(), Err(PacketError::InvalidBoolean(2)));

        let mut empty = Packet::new();
        assert_eq!(
            empty.decode_i32(),
            Err(PacketError::Underflow {
                requested: 4,
                remaining: 0
            })
        );
    }

    #[test]
    fn test_camera_target_relation_and_multitag_estimation() {
        let cam = Pose3d::new(Translation3d::new(0.0, 0.0, 0.0), Rotation3d::default());
        let targ = Pose3d::new(Translation3d::new(2.0, 0.0, 0.0), Rotation3d::default());
        let rel = CameraTargetRelation::new(cam, targ);
        assert!((rel.cam_to_targ_dist - 2.0).abs() < 1e-6);
        assert!(rel.cam_to_targ_yaw.abs() < 1e-6);

        let tag1 = Pose3d::new(Translation3d::new(4.0, 1.0, 0.5), Rotation3d::default());
        let tag2 = Pose3d::new(Translation3d::new(4.0, 2.0, 0.5), Rotation3d::default());
        let obs1 = Transform3d::new(Translation3d::new(2.0, 0.0, 0.0), Rotation3d::default());
        let obs2 = Transform3d::new(Translation3d::new(2.0, 1.0, 0.0), Rotation3d::default());

        let pnp = VisionEstimation::estimate_cam_pose_pnp(&[(tag1, obs1), (tag2, obs2)])
            .expect("estimate multitag pose");
        assert!((pnp.best.translation.x - 2.0).abs() < 1e-6);
        assert!((pnp.best.translation.y - 1.0).abs() < 1e-6);
    }
}
