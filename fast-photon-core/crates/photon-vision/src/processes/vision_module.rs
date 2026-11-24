use crate::calibration::CameraCalibrationCoefficients;
use crate::frame::{FileSaveFrameConsumer, MJPGFrameConsumer};
use crate::pipeline::{CVPipelineResult, OutputStreamPipeline};
use crate::processes::{PipelineManager, VisionRunner, VisionSource};
use photon_common::configuration::{AprilTagFieldLayout, CameraConfiguration, PipelineType};
use photon_common::dataflow::networktables::NTDataPublisher;
use photon_common::dataflow::websocket::{UICameraConfiguration, UIDataPublisher};
use photon_common::dataflow::{CVPipelineResultConsumer, DataChangeEvent, DataChangeService};
use std::path::PathBuf;
use std::sync::Arc;

/// Coordinates a camera's `VisionSource`, `PipelineManager`, `VisionRunner`, MJPEG streamers, and NT/UI publishers (`VisionModule`).
pub struct VisionModule {
    pub pipeline_manager: PipelineManager,
    pub vision_source: Box<dyn VisionSource>,
    pub vision_runner: VisionRunner,
    pub output_stream_pipeline: OutputStreamPipeline,
    pub nt_consumer: NTDataPublisher,
    pub ui_consumer: UIDataPublisher,
    pub extra_consumers: Vec<Arc<dyn CVPipelineResultConsumer>>,
    pub input_frame_saver: FileSaveFrameConsumer,
    pub output_frame_saver: FileSaveFrameConsumer,
    pub input_video_streamer: MJPGFrameConsumer,
    pub output_video_streamer: MJPGFrameConsumer,
    pub pending_calibration: Option<CameraCalibrationCoefficients>,
}

impl VisionModule {
    pub fn new(
        vision_source: Box<dyn VisionSource>,
        data_change_service: DataChangeService,
        snapshots_dir: PathBuf,
        extra_consumers: Vec<Arc<dyn CVPipelineResultConsumer>>,
    ) -> Self {
        let cfg = vision_source.camera_configuration().clone();
        let pipeline_manager = PipelineManager::new(
            cfg.pipeline_settings.clone(),
            cfg.drive_mode_settings.clone(),
            cfg.current_pipeline_index,
        );
        let input_port = (1181 + cfg.stream_index * 2) as u16;
        let output_port = (1182 + cfg.stream_index * 2) as u16;

        let nt_consumer = NTDataPublisher::new(cfg.nickname.clone(), true);
        let ui_consumer = UIDataPublisher::new(cfg.unique_name.clone(), data_change_service);
        let input_frame_saver =
            FileSaveFrameConsumer::new(cfg.nickname.clone(), "input", snapshots_dir.clone());
        let output_frame_saver =
            FileSaveFrameConsumer::new(cfg.nickname.clone(), "output", snapshots_dir);
        let input_video_streamer =
            MJPGFrameConsumer::new(format!("{}_input", cfg.unique_name), input_port);
        let output_video_streamer =
            MJPGFrameConsumer::new(format!("{}_output", cfg.unique_name), output_port);

        Self {
            pipeline_manager,
            vision_source,
            vision_runner: VisionRunner::new(),
            output_stream_pipeline: OutputStreamPipeline::default(),
            nt_consumer,
            ui_consumer,
            extra_consumers,
            input_frame_saver,
            output_frame_saver,
            input_video_streamer,
            output_video_streamer,
            pending_calibration: None,
        }
    }

    pub fn unique_name(&self) -> &str {
        &self.vision_source.camera_configuration().unique_name
    }

    pub fn nickname(&self) -> &str {
        &self.vision_source.camera_configuration().nickname
    }

    pub fn set_camera_nickname(&mut self, new_name: impl Into<String>) {
        let name = new_name.into();
        self.vision_source.camera_configuration_mut().nickname = name.clone();
        self.nt_consumer.update_camera_nickname(name);
    }

    pub fn set_pipeline_index(&mut self, index: i32) -> bool {
        let ok = self.pipeline_manager.set_index(index);
        if ok && index >= 0 {
            self.vision_source
                .camera_configuration_mut()
                .current_pipeline_index = index;
            self.nt_consumer.handle_pipeline_index_request(index);
        }
        ok
    }

    pub fn set_driver_mode(&mut self, driver_mode: bool) {
        self.pipeline_manager.set_driver_mode(driver_mode);
        self.nt_consumer.handle_driver_mode_request(driver_mode);
    }

    pub fn set_atfl(&mut self, atfl: AprilTagFieldLayout) {
        self.pipeline_manager.set_atfl(atfl);
    }

    pub fn save_input_snapshot(&self) {
        self.input_frame_saver.save_one_frame();
    }

    pub fn save_output_snapshot(&self) {
        self.output_frame_saver.save_one_frame();
    }

    pub fn to_camera_config(&self) -> CameraConfiguration {
        let mut cfg = self.vision_source.camera_configuration().clone();
        cfg.pipeline_settings = self.pipeline_manager.user_pipeline_settings.clone();
        cfg.current_pipeline_index = self.pipeline_manager.get_requested_index().max(0);
        cfg
    }

    pub fn to_ui_camera_config(&self) -> UICameraConfiguration {
        UICameraConfiguration::from(&self.to_camera_config())
    }

    /// Run one frame cycle: capture -> pipeline -> NT/UI/extra consumers -> stream overlays & MJPEG/file savers.
    pub fn tick_once(&mut self) -> Option<CVPipelineResult> {
        let quirks = self
            .vision_source
            .camera_configuration()
            .camera_quirks
            .clone();
        let res = self.vision_runner.step_once(
            self.vision_source.frame_provider(),
            self.pipeline_manager.current_pipeline_mut(),
            &quirks,
        )?;

        let wire_res = res.to_photon_pipeline_result(0);
        let fps = f64::from(res.fps);

        // 1. Publish to NetworkTables, WebSocket UI, and extra consumers (e.g. StatusLEDConsumer)
        self.nt_consumer.accept(&wire_res, fps);
        self.ui_consumer.accept(&wire_res, fps);
        for consumer in &self.extra_consumers {
            consumer.accept(&wire_res, fps);
        }

        // 2. Save raw input snapshot if requested, then render overlays for output stream
        let _ = self
            .input_frame_saver
            .accept(&res.input_and_output_frame.color_image);
        let settings = self.pipeline_manager.current_pipeline().settings().clone();
        let rendered_frame = self.output_stream_pipeline.process(
            res.input_and_output_frame.clone(),
            &settings,
            res.targets.clone(),
        );
        let _ = self
            .output_frame_saver
            .accept(&rendered_frame.processed_image);

        if settings.input_should_show {
            self.input_video_streamer
                .accept(&rendered_frame.color_image);
        }
        if settings.output_should_show {
            self.output_video_streamer
                .accept(&rendered_frame.processed_image);
        }

        Some(res)
    }

    /// Apply a WebSocket / DataChangeEvent mutation targeted at this module.
    pub fn apply_data_change_event(&mut self, event: &DataChangeEvent) {
        if let Some(target_cam) = &event.camera_unique_name {
            if target_cam != self.unique_name() {
                return;
            }
        }
        match event.property_name.as_str() {
            "isDriverMode" => {
                if let Some(b) = event.data.as_bool() {
                    self.set_driver_mode(b);
                }
            }
            "cameraNickname" => {
                if let Some(s) = event.data.as_str() {
                    self.set_camera_nickname(s);
                }
            }
            "pipelineName" => {
                if let Some(s) = event.data.as_str() {
                    self.pipeline_manager
                        .current_pipeline_mut()
                        .settings_mut()
                        .pipeline_nickname = s.to_string();
                }
            }
            "changePipeline" => {
                if let Some(idx) = event.data.as_i64() {
                    self.set_pipeline_index(idx as i32);
                }
            }
            "duplicatePipeline" => {
                if let Some(idx) = event.data.as_i64() {
                    self.pipeline_manager.duplicate_pipeline(idx as i32);
                }
            }
            "deleteCurrPipeline" => {
                let curr = self.pipeline_manager.get_requested_index();
                self.pipeline_manager.remove_pipeline(curr);
            }
            "saveInputSnapshot" => self.save_input_snapshot(),
            "saveOutputSnapshot" => self.save_output_snapshot(),
            "newPipelineInfo" => {
                if let Some(arr) = event.data.as_array() {
                    if arr.len() >= 2 {
                        let name = arr[0].as_str().unwrap_or("New Pipeline");
                        let ty = serde_json::from_value::<PipelineType>(arr[1].clone())
                            .unwrap_or(PipelineType::Reflective);
                        self.pipeline_manager.add_pipeline(ty, name);
                    }
                }
            }
            _ => {}
        }
    }
}
