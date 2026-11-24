use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Alliance {
    #[default]
    Unknown,
    Red,
    Blue,
}

/// Tracks FMS/DriverStation state published over NetworkTables (`org.photonvision.common.dataflow.networktables.NTDriverStation`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NTDriverStation {
    pub is_red_alliance: bool,
    pub is_fms_attached: bool,
    pub is_ds_attached: bool,
    pub is_enabled: bool,
    pub is_autonomous: bool,
    pub match_number: i32,
    pub event_name: String,
}

impl NTDriverStation {
    pub fn alliance(&self) -> Alliance {
        if self.is_red_alliance {
            Alliance::Red
        } else {
            Alliance::Blue
        }
    }
}
