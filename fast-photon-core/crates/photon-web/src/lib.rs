//! `photon-web`: Embedded HTTP REST API, MessagePack WebSocket handler, and MJPEG stream server.

pub mod handlers;
pub mod mjpeg;
pub mod server;
pub mod websocket;

#[cfg(test)]
mod unit_tests {
    use crate::mjpeg::format_mjpeg_frame;
    use crate::websocket::{DataSocketHandler, DataSocketMessageType};
    use serde_json::json;

    #[test]
    fn test_all_16_datasocket_message_types_mapping() {
        let keys = [
            ("driverMode", DataSocketMessageType::SMT_DRIVERMODE),
            (
                "cameraNickname",
                DataSocketMessageType::SMT_CHANGECAMERANAME,
            ),
            (
                "pipelineName",
                DataSocketMessageType::SMT_CHANGEPIPELINENAME,
            ),
            ("newPipelineInfo", DataSocketMessageType::SMT_ADDNEWPIPELINE),
            (
                "changeBrightness",
                DataSocketMessageType::SMT_CHANGEBRIGHTNESS,
            ),
            (
                "duplicatePipeline",
                DataSocketMessageType::SMT_DUPLICATEPIPELINE,
            ),
            (
                "deleteCurrPipeline",
                DataSocketMessageType::SMT_DELETECURRENTPIPELINE,
            ),
            (
                "robotOffsetPoint",
                DataSocketMessageType::SMT_ROBOTOFFSETPOINT,
            ),
            ("changeUICamera", DataSocketMessageType::SMT_CURRENTCAMERA),
            ("changePipeline", DataSocketMessageType::SMT_CURRENTPIPELINE),
            (
                "startCalibration",
                DataSocketMessageType::SMT_STARTPNPCALIBRATION,
            ),
            (
                "saveInputSnapshot",
                DataSocketMessageType::SMT_SAVEINPUTSNAPSHOT,
            ),
            (
                "saveOutputSnapshot",
                DataSocketMessageType::SMT_SAVEOUTPUTSNAPSHOT,
            ),
            (
                "takeCalSnapshot",
                DataSocketMessageType::SMT_TAKECALIBRATIONSNAPSHOT,
            ),
            (
                "changePipelineSetting",
                DataSocketMessageType::SMT_PIPELINESETTINGCHANGE,
            ),
            (
                "changePipelineType",
                DataSocketMessageType::SMT_CHANGEPIPELINETYPE,
            ),
        ];
        for (k, expected) in keys {
            assert_eq!(DataSocketMessageType::from_entry_key(k), Some(expected));
        }
        assert_eq!(DataSocketMessageType::from_entry_key("unknownKey"), None);

        let val = json!({"cameraUniqueName": "c1", "driverMode": false});
        let encoded = DataSocketHandler::encode_msgpack_value(&val).expect("encode");
        let decoded = DataSocketHandler::decode_msgpack_message(&encoded).expect("decode");
        assert_eq!(decoded.camera_unique_name.as_deref(), Some("c1"));

        let frame = format_mjpeg_frame(&[0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(frame.ends_with(b"\r\n"));
    }
}
