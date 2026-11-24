use crate::packet::{Packet, PacketResult, PacketSerde};
use crate::targets::{MultiTargetPNPResult, PhotonTrackedTarget};
use serde::{Deserialize, Serialize};

/// Metadata header for `PhotonPipelineResult` (`org.photonvision.targeting.PhotonPipelineMetadata`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhotonPipelineMetadata {
    pub sequence_id: i64,
    pub capture_timestamp_micros: i64,
    pub publish_timestamp_micros: i64,
    pub time_since_last_pong: i64,
}

impl PhotonPipelineMetadata {
    pub fn latency_millis(&self) -> f64 {
        (self.publish_timestamp_micros - self.capture_timestamp_micros) as f64 / 1000.0
    }
}

impl PacketSerde for PhotonPipelineMetadata {
    const INTERFACE_UUID: &'static str = "5231116d7f87ff560e6aa5263d0eee02";
    const SCHEMA: &'static str = "int64 sequenceID;int64 captureTimestampMicros;int64 publishTimestampMicros;int64 timeSinceLastPong;";
    const TYPE_NAME: &'static str = "PhotonPipelineMetadata";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        packet.encode_i64(self.sequence_id);
        packet.encode_i64(self.capture_timestamp_micros);
        packet.encode_i64(self.publish_timestamp_micros);
        packet.encode_i64(self.time_since_last_pong);
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            sequence_id: packet.decode_i64()?,
            capture_timestamp_micros: packet.decode_i64()?,
            publish_timestamp_micros: packet.decode_i64()?,
            time_since_last_pong: packet.decode_i64()?,
        })
    }
}

/// Complete pipeline result published over NetworkTables (`org.photonvision.targeting.PhotonPipelineResult`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhotonPipelineResult {
    pub metadata: PhotonPipelineMetadata,
    pub targets: Vec<PhotonTrackedTarget>,
    pub multitag_result: Option<MultiTargetPNPResult>,
}

impl PhotonPipelineResult {
    pub fn has_targets(&self) -> bool {
        !self.targets.is_empty()
    }

    pub fn best_target(&self) -> Option<&PhotonTrackedTarget> {
        self.targets.first()
    }
}

impl PacketSerde for PhotonPipelineResult {
    const INTERFACE_UUID: &'static str = "fd353b11f3a818e9c5b457c7dc5a8f5c";
    const SCHEMA: &'static str = "PhotonPipelineMetadata:5231116d7f87ff560e6aa5263d0eee02 metadata;PhotonTrackedTarget:cc6dbb5c5c1e0fa808108019b20863f1 targets[?];optional MultiTargetPNPResult:541096947e9f3ca2d3f425ff7b04aa7b multitagResult;";
    const TYPE_NAME: &'static str = "PhotonPipelineResult";

    fn pack(&self, packet: &mut Packet) -> PacketResult<()> {
        self.metadata.pack(packet)?;
        packet.encode_list(&self.targets)?;
        packet.encode_optional(&self.multitag_result)?;
        Ok(())
    }

    fn unpack(packet: &mut Packet) -> PacketResult<Self> {
        Ok(Self {
            metadata: PhotonPipelineMetadata::unpack(packet)?,
            targets: packet.decode_list()?,
            multitag_result: packet.decode_optional()?,
        })
    }
}
