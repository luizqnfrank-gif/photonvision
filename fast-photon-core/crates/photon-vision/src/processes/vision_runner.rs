use crate::frame::FrameProvider;
use crate::pipeline::{CVPipeline, CVPipelineResult};
use photon_common::configuration::QuirkyCamera;

/// Executes frame acquisition and active `CVPipeline` processing (`org.photonvision.vision.processes.VisionRunner`).
pub struct VisionRunner {
    fps_limit: i32,
    enabled: bool,
}

impl Default for VisionRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl VisionRunner {
    pub const fn new() -> Self {
        Self {
            fps_limit: -1,
            enabled: true,
        }
    }

    pub fn set_fps_limit(&mut self, limit: i32) {
        self.fps_limit = limit;
    }

    pub fn fps_limit(&self) -> i32 {
        self.fps_limit
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Execute a single synchronous frame capture and pipeline pass.
    pub fn step_once(
        &mut self,
        provider: &mut dyn FrameProvider,
        pipeline: &mut dyn CVPipeline,
        quirks: &QuirkyCamera,
    ) -> Option<CVPipelineResult> {
        if !self.enabled || !provider.is_connected() {
            return None;
        }
        let frame = provider.get_frame();
        Some(pipeline.run(frame, quirks))
    }
}
