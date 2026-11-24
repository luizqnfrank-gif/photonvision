use crate::pipeline::{
    AprilTagPipeline, ArucoPipeline, CVPipeline, Calibrate3dPipeline, ColoredShapePipeline,
    DriverModePipeline, FocusPipeline, ObjectDetectionPipeline, ReflectivePipeline,
};
use photon_common::configuration::{
    AprilTagFieldLayout, CVPipelineSettings, DriverModePipelineSettings, PipelineSettingsVariant,
    PipelineType,
};

pub const DRIVERMODE_INDEX: i32 = -2;
pub const CALIBRATE_3D_INDEX: i32 = -3;
pub const FOCUS_INDEX: i32 = -1;

/// Manages user pipelines, active pipeline index, driver mode, and calibration mode (`PipelineManager`).
pub struct PipelineManager {
    pub user_pipeline_settings: Vec<PipelineSettingsVariant>,
    pub driver_mode_settings: DriverModePipelineSettings,
    requested_index: i32,
    is_driver_mode: bool,
    is_calibrating: bool,
    atfl: AprilTagFieldLayout,
    active_pipeline: Box<dyn CVPipeline>,
}

impl PipelineManager {
    pub fn new(
        mut user_pipeline_settings: Vec<PipelineSettingsVariant>,
        driver_mode_settings: DriverModePipelineSettings,
        initial_index: i32,
    ) -> Self {
        if user_pipeline_settings.is_empty() {
            user_pipeline_settings.push(PipelineSettingsVariant::default());
        }
        let idx = initial_index.clamp(0, (user_pipeline_settings.len() as i32) - 1);
        let active_pipeline = Self::instantiate_pipeline(&user_pipeline_settings[idx as usize]);

        Self {
            user_pipeline_settings,
            driver_mode_settings,
            requested_index: idx,
            is_driver_mode: false,
            is_calibrating: false,
            atfl: AprilTagFieldLayout::default(),
            active_pipeline,
        }
    }

    fn instantiate_pipeline(variant: &PipelineSettingsVariant) -> Box<dyn CVPipeline> {
        let s = variant.settings().clone();
        match variant.pipeline_type() {
            PipelineType::Reflective => Box::new(ReflectivePipeline::new(s)),
            PipelineType::ColoredShape => Box::new(ColoredShapePipeline::new(s)),
            PipelineType::AprilTag => Box::new(AprilTagPipeline::new(s)),
            PipelineType::Aruco => Box::new(ArucoPipeline::new(s)),
            PipelineType::ObjectDetection => Box::new(ObjectDetectionPipeline::new(s)),
            PipelineType::Calibrate3d => Box::new(Calibrate3dPipeline::new(s)),
            PipelineType::DriverMode => Box::new(DriverModePipeline::new(s)),
            PipelineType::Focus => Box::new(FocusPipeline::new(s)),
        }
    }

    pub fn get_requested_index(&self) -> i32 {
        if self.is_calibrating {
            CALIBRATE_3D_INDEX
        } else if self.is_driver_mode {
            DRIVERMODE_INDEX
        } else {
            self.requested_index
        }
    }

    pub fn set_index(&mut self, index: i32) -> bool {
        if index == DRIVERMODE_INDEX {
            self.set_driver_mode(true);
            return true;
        }
        if index == CALIBRATE_3D_INDEX {
            self.set_calibration_mode(true);
            return true;
        }
        if index == FOCUS_INDEX {
            self.active_pipeline = Box::new(FocusPipeline::new(CVPipelineSettings::default()));
            return true;
        }
        if index < 0 || (index as usize) >= self.user_pipeline_settings.len() {
            return false;
        }
        self.is_driver_mode = false;
        self.is_calibrating = false;
        self.requested_index = index;
        let mut p = Self::instantiate_pipeline(&self.user_pipeline_settings[index as usize]);
        p.set_atfl(self.atfl.clone());
        self.active_pipeline = p;
        true
    }

    pub fn set_driver_mode(&mut self, driver_mode: bool) {
        self.is_driver_mode = driver_mode;
        if driver_mode {
            let s = CVPipelineSettings {
                pipeline_type: PipelineType::DriverMode,
                pipeline_nickname: self.driver_mode_settings.pipeline_nickname.clone(),
                ..CVPipelineSettings::default()
            };
            self.active_pipeline = Box::new(DriverModePipeline::new(s));
        } else {
            let idx = self.requested_index;
            let _ = self.set_index(idx);
        }
    }

    pub fn is_driver_mode(&self) -> bool {
        self.is_driver_mode
    }

    pub fn set_calibration_mode(&mut self, calibrating: bool) {
        self.is_calibrating = calibrating;
        if calibrating {
            let s = CVPipelineSettings {
                pipeline_type: PipelineType::Calibrate3d,
                ..CVPipelineSettings::default()
            };
            self.active_pipeline = Box::new(Calibrate3dPipeline::new(s));
        } else {
            let idx = self.requested_index;
            let _ = self.set_index(idx);
        }
    }

    pub fn set_atfl(&mut self, atfl: AprilTagFieldLayout) {
        self.atfl = atfl.clone();
        self.active_pipeline.set_atfl(atfl);
    }

    pub fn current_pipeline(&self) -> &dyn CVPipeline {
        self.active_pipeline.as_ref()
    }

    pub fn current_pipeline_mut(&mut self) -> &mut dyn CVPipeline {
        self.active_pipeline.as_mut()
    }

    pub fn add_pipeline(&mut self, ty: PipelineType, nickname: impl Into<String>) -> i32 {
        let s = CVPipelineSettings {
            pipeline_type: ty,
            pipeline_nickname: nickname.into(),
            pipeline_index: self.user_pipeline_settings.len() as i32,
            ..CVPipelineSettings::default()
        };
        let variant = match ty {
            PipelineType::Reflective => PipelineSettingsVariant::ReflectivePipelineSettings(s),
            PipelineType::AprilTag => PipelineSettingsVariant::AprilTagPipelineSettings(s),
            PipelineType::Aruco => PipelineSettingsVariant::ArucoPipelineSettings(s),
            PipelineType::ColoredShape => PipelineSettingsVariant::ColoredShapePipelineSettings(s),
            PipelineType::ObjectDetection => {
                PipelineSettingsVariant::ObjectDetectionPipelineSettings(s)
            }
            PipelineType::Calibrate3d => PipelineSettingsVariant::Calibration3dPipelineSettings(s),
            PipelineType::DriverMode => PipelineSettingsVariant::DriverModePipelineSettings(s),
            PipelineType::Focus => PipelineSettingsVariant::FocusPipelineSettings(s),
        };
        self.user_pipeline_settings.push(variant);
        let new_idx = (self.user_pipeline_settings.len() as i32) - 1;
        let _ = self.set_index(new_idx);
        new_idx
    }

    pub fn duplicate_pipeline(&mut self, index: i32) -> i32 {
        let idx = index.clamp(0, (self.user_pipeline_settings.len() as i32) - 1) as usize;
        let mut cloned = self.user_pipeline_settings[idx].clone();
        cloned.settings_mut().pipeline_nickname.push_str("_1");
        cloned.settings_mut().pipeline_index = self.user_pipeline_settings.len() as i32;
        self.user_pipeline_settings.push(cloned);
        let new_idx = (self.user_pipeline_settings.len() as i32) - 1;
        let _ = self.set_index(new_idx);
        new_idx
    }

    pub fn remove_pipeline(&mut self, index: i32) -> bool {
        if self.user_pipeline_settings.len() <= 1
            || index < 0
            || (index as usize) >= self.user_pipeline_settings.len()
        {
            return false;
        }
        self.user_pipeline_settings.remove(index as usize);
        for (i, p) in self.user_pipeline_settings.iter_mut().enumerate() {
            p.settings_mut().pipeline_index = i as i32;
        }
        let next = self
            .requested_index
            .min((self.user_pipeline_settings.len() as i32) - 1);
        let _ = self.set_index(next);
        true
    }
}
