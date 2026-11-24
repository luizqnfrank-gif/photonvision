use fast_photon_core::targeting::proto::{ProtobufDeviceMetrics, ProtobufPhotonPipelineResult};
use fast_photon_core::targeting::{
    MultiTargetPNPResult, Packet, PacketSerde, PhotonPipelineMetadata, PhotonPipelineResult,
    PhotonTrackedTarget, PnpResult, Quaternion, Rotation3d, TargetCorner, Transform3d,
    Translation3d,
};
use prost::Message;

#[test]
fn test_transform3d_struct_56_bytes_roundtrip() {
    let c = std::f64::consts::FRAC_1_SQRT_2;
    let tf = Transform3d::new(
        Translation3d::new(1.25, -0.5, 0.75),
        Rotation3d::new(Quaternion::new(c, 0.0, c, 0.0)),
    );
    let bytes = tf.to_struct_bytes();
    assert_eq!(bytes.len(), Transform3d::STRUCT_SIZE);

    let decoded = Transform3d::from_struct_bytes(&bytes).expect("decode Transform3d struct");
    assert_eq!(tf, decoded);
}

#[test]
fn test_photon_pipeline_result_packet_uuid_and_roundtrip() {
    assert_eq!(
        PhotonPipelineResult::INTERFACE_UUID,
        "fd353b11f3a818e9c5b457c7dc5a8f5c"
    );
    assert_eq!(
        PhotonTrackedTarget::INTERFACE_UUID,
        "cc6dbb5c5c1e0fa808108019b20863f1"
    );
    assert_eq!(
        PhotonPipelineMetadata::INTERFACE_UUID,
        "5231116d7f87ff560e6aa5263d0eee02"
    );
    assert_eq!(
        MultiTargetPNPResult::INTERFACE_UUID,
        "541096947e9f3ca2d3f425ff7b04aa7b"
    );

    let target = PhotonTrackedTarget {
        yaw: -12.34,
        pitch: 5.67,
        area: 4.2,
        skew: 0.0,
        fiducial_id: 7,
        obj_detect_id: -1,
        obj_detect_conf: -1.0,
        best_camera_to_target: Transform3d::new(
            Translation3d::new(2.5, -0.3, 0.4),
            Rotation3d::default(),
        ),
        alt_camera_to_target: Transform3d::new(
            Translation3d::new(2.5, 0.3, 0.4),
            Rotation3d::default(),
        ),
        pose_ambiguity: 0.08,
        min_area_rect_corners: vec![
            TargetCorner::new(100.0, 200.0),
            TargetCorner::new(150.0, 200.0),
            TargetCorner::new(150.0, 150.0),
            TargetCorner::new(100.0, 150.0),
        ],
        detected_corners: vec![
            TargetCorner::new(101.0, 199.0),
            TargetCorner::new(149.0, 199.0),
            TargetCorner::new(149.0, 151.0),
            TargetCorner::new(101.0, 151.0),
        ],
    };

    let result = PhotonPipelineResult {
        metadata: PhotonPipelineMetadata {
            sequence_id: 42,
            capture_timestamp_micros: 1_000_000,
            publish_timestamp_micros: 1_005_500,
            time_since_last_pong: 25_000,
        },
        targets: vec![target],
        multitag_result: Some(MultiTargetPNPResult {
            estimated_pose: PnpResult {
                best: Transform3d::new(Translation3d::new(4.1, 2.2, 0.3), Rotation3d::default()),
                best_reproj_err: 0.12,
                alt: Transform3d::default(),
                alt_reproj_err: 0.85,
                ambiguity: 0.14,
            },
            fiducial_ids_used: vec![4, 7],
        }),
    };

    let mut pkt = Packet::new();
    result.pack(&mut pkt).expect("pack PhotonPipelineResult");
    assert!(!pkt.is_empty());

    let unpacked = PhotonPipelineResult::unpack(&mut pkt).expect("unpack PhotonPipelineResult");
    assert_eq!(result, unpacked);
    assert_eq!(pkt.remaining(), 0);
}

#[test]
fn test_protobuf_pipeline_result_and_metrics_roundtrip() {
    let orig = PhotonPipelineResult {
        metadata: PhotonPipelineMetadata {
            sequence_id: 99,
            capture_timestamp_micros: 500_000,
            publish_timestamp_micros: 512_000,
            time_since_last_pong: 10_000,
        },
        targets: vec![PhotonTrackedTarget {
            yaw: 3.5,
            pitch: -1.2,
            area: 8.0,
            fiducial_id: 3,
            ..PhotonTrackedTarget::default()
        }],
        multitag_result: None,
    };

    let proto = ProtobufPhotonPipelineResult::from(&orig);
    let encoded = proto.encode_to_vec();
    let decoded_proto =
        ProtobufPhotonPipelineResult::decode(encoded.as_slice()).expect("decode protobuf");
    let roundtripped = PhotonPipelineResult::from(decoded_proto);
    assert_eq!(orig, roundtripped);

    let metrics = ProtobufDeviceMetrics {
        cpu_temp: 54.2,
        cpu_util: 31.5,
        cpu_thr: "None".to_string(),
        ram_mem: 4096.0,
        ram_util: 1024.0,
        gpu_mem: 0.0,
        gpu_mem_util: 0.0,
        disk_util_pct: 22.0,
        npu_usage: vec![15.0, 20.0, 10.0],
        ip_address: "10.12.34.11".to_string(),
        uptime: 3600.0,
        sent_bit_rate: 1200.0,
        recv_bit_rate: 400.0,
        disk_usable_space: 16_000.0,
    };
    let m_bytes = metrics.encode_to_vec();
    let m_dec = ProtobufDeviceMetrics::decode(m_bytes.as_slice()).expect("decode metrics");
    assert_eq!(metrics, m_dec);
}
