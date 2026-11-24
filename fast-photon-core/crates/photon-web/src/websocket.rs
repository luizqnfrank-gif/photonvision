#![allow(non_camel_case_types)]

use photon_common::configuration::PipelineType;
use photon_common::dataflow::{DataChangeDestination, DataChangeEvent, DataChangeService};
use photon_hal::hardware_manager::HardwareManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// All 16 WebSocket message types (`org.photonvision.server.DataSocketMessageType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataSocketMessageType {
    SMT_DRIVERMODE,
    SMT_CHANGECAMERANAME,
    SMT_CHANGEPIPELINENAME,
    SMT_ADDNEWPIPELINE,
    SMT_CHANGEBRIGHTNESS,
    SMT_DUPLICATEPIPELINE,
    SMT_DELETECURRENTPIPELINE,
    SMT_ROBOTOFFSETPOINT,
    SMT_CURRENTCAMERA,
    SMT_CURRENTPIPELINE,
    SMT_STARTPNPCALIBRATION,
    SMT_SAVEINPUTSNAPSHOT,
    SMT_SAVEOUTPUTSNAPSHOT,
    SMT_TAKECALIBRATIONSNAPSHOT,
    SMT_PIPELINESETTINGCHANGE,
    SMT_CHANGEPIPELINETYPE,
}

impl DataSocketMessageType {
    pub fn from_entry_key(key: &str) -> Option<Self> {
        match key {
            "driverMode" => Some(Self::SMT_DRIVERMODE),
            "cameraNickname" => Some(Self::SMT_CHANGECAMERANAME),
            "pipelineName" => Some(Self::SMT_CHANGEPIPELINENAME),
            "newPipelineInfo" => Some(Self::SMT_ADDNEWPIPELINE),
            "changeBrightness" => Some(Self::SMT_CHANGEBRIGHTNESS),
            "duplicatePipeline" => Some(Self::SMT_DUPLICATEPIPELINE),
            "deleteCurrPipeline" => Some(Self::SMT_DELETECURRENTPIPELINE),
            "robotOffsetPoint" => Some(Self::SMT_ROBOTOFFSETPOINT),
            "changeUICamera" => Some(Self::SMT_CURRENTCAMERA),
            "changePipeline" => Some(Self::SMT_CURRENTPIPELINE),
            "startCalibration" => Some(Self::SMT_STARTPNPCALIBRATION),
            "saveInputSnapshot" => Some(Self::SMT_SAVEINPUTSNAPSHOT),
            "saveOutputSnapshot" => Some(Self::SMT_SAVEOUTPUTSNAPSHOT),
            "takeCalSnapshot" => Some(Self::SMT_TAKECALIBRATIONSNAPSHOT),
            "changePipelineSetting" => Some(Self::SMT_PIPELINESETTINGCHANGE),
            "changePipelineType" => Some(Self::SMT_CHANGEPIPELINETYPE),
            _ => None,
        }
    }
}

/// Decoded inbound MessagePack frame (`org.photonvision.server.DataSocketHandler.WSMessage`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WSMessage {
    #[serde(rename = "cameraUniqueName", default)]
    pub camera_unique_name: Option<String>,
    #[serde(flatten)]
    pub properties: HashMap<String, Value>,
}

/// Processes binary MessagePack WebSocket frames and publishes `DataChangeEvent`s (`DataSocketHandler`).
pub struct DataSocketHandler;

impl DataSocketHandler {
    pub fn decode_msgpack_message(bytes: &[u8]) -> Result<WSMessage, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }

    pub fn encode_msgpack_value(val: &Value) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(val)
    }

    pub fn handle_binary_message(
        bytes: &[u8],
        dc_service: &DataChangeService,
        hardware_manager: Option<&HardwareManager>,
        origin_client_id: Option<u64>,
    ) -> Result<usize, rmp_serde::decode::Error> {
        let msg = Self::decode_msgpack_message(bytes)?;
        let mut dispatched = 0usize;

        for (entry_key, entry_val) in msg.properties {
            let Some(msg_type) = DataSocketMessageType::from_entry_key(&entry_key) else {
                continue;
            };
            match msg_type {
                DataSocketMessageType::SMT_DRIVERMODE => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "isDriverMode",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_CHANGECAMERANAME => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "cameraNickname",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_CHANGEPIPELINENAME => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "pipelineName",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_ADDNEWPIPELINE => {
                    let payload = if let Some(arr) = entry_val.as_array() {
                        if arr.len() >= 2 {
                            let name = arr[0].as_str().unwrap_or("New Pipeline");
                            let ord = arr[1].as_i64().unwrap_or(0);
                            let p_type = match ord {
                                -3 => PipelineType::Calibrate3d,
                                -2 => PipelineType::DriverMode,
                                -1 => PipelineType::Focus,
                                0 => PipelineType::Reflective,
                                1 => PipelineType::ColoredShape,
                                2 => PipelineType::AprilTag,
                                3 => PipelineType::Aruco,
                                4 => PipelineType::ObjectDetection,
                                _ => PipelineType::Reflective,
                            };
                            json!([name, p_type])
                        } else {
                            entry_val
                        }
                    } else {
                        entry_val
                    };
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "newPipelineInfo",
                        payload,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_CHANGEBRIGHTNESS => {
                    let pct = entry_val
                        .as_i64()
                        .or_else(|| entry_val.as_str().and_then(|s| s.parse::<i64>().ok()))
                        .unwrap_or(100) as i32;
                    if let Some(hw) = hardware_manager {
                        hw.set_brightness_percent(pct);
                    }
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_DUPLICATEPIPELINE => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "duplicatePipeline",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_DELETECURRENTPIPELINE => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "deleteCurrPipeline",
                        json!(0),
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_ROBOTOFFSETPOINT => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "robotOffsetPoint",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        None,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_CURRENTCAMERA => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_OTHER,
                        "changeUICamera",
                        entry_val,
                        None,
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_CURRENTPIPELINE => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "changePipeline",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_STARTPNPCALIBRATION => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "startCalibration",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_SAVEINPUTSNAPSHOT => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "saveInputSnapshot",
                        json!(0),
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_SAVEOUTPUTSNAPSHOT => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "saveOutputSnapshot",
                        json!(0),
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_TAKECALIBRATIONSNAPSHOT => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "takeCalSnapshot",
                        json!(0),
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
                DataSocketMessageType::SMT_PIPELINESETTINGCHANGE => {
                    if let Some(obj) = entry_val.as_object() {
                        let cam_uname = obj
                            .get("cameraUniqueName")
                            .and_then(Value::as_str)
                            .map(String::from)
                            .or_else(|| msg.camera_unique_name.clone());
                        for (k, v) in obj {
                            if k == "cameraUniqueName" {
                                continue;
                            }
                            dc_service.publish_event(DataChangeEvent::incoming_ws(
                                DataChangeDestination::DCD_ACTIVEPIPELINESETTINGS,
                                k.clone(),
                                v.clone(),
                                cam_uname.clone(),
                                origin_client_id,
                            ));
                            dispatched += 1;
                        }
                    }
                }
                DataSocketMessageType::SMT_CHANGEPIPELINETYPE => {
                    dc_service.publish_event(DataChangeEvent::incoming_ws(
                        DataChangeDestination::DCD_ACTIVEMODULE,
                        "changePipelineType",
                        entry_val,
                        msg.camera_unique_name.clone(),
                        origin_client_id,
                    ));
                    dispatched += 1;
                }
            }
        }

        Ok(dispatched)
    }
}
