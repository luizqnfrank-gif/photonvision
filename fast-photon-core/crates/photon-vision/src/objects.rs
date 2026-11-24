use crate::opencv_ext::CVMat;
use photon_targeting::TargetCorner;
use serde::{Deserialize, Serialize};

/// Result of a single neural network object detection (`org.photonvision.vision.pipe.impl.NeuralNetworkPipeResult`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeuralNetworkPipeResult {
    pub bbox_corners: Vec<TargetCorner>,
    pub class_id: i32,
    pub confidence: f32,
}

impl NeuralNetworkPipeResult {
    pub fn new(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        class_id: i32,
        confidence: f32,
    ) -> Self {
        Self {
            bbox_corners: vec![
                TargetCorner::new(x_min, y_max),
                TargetCorner::new(x_max, y_max),
                TargetCorner::new(x_max, y_min),
                TargetCorner::new(x_min, y_min),
            ],
            class_id,
            confidence,
        }
    }
}

/// Letterbox scaling parameters (`org.photonvision.vision.objects.Letterbox`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Letterbox {
    pub scale: f64,
    pub dx: f64,
    pub dy: f64,
    pub target_width: u32,
    pub target_height: u32,
}

impl Letterbox {
    pub fn compute(src_width: u32, src_height: u32, target_width: u32, target_height: u32) -> Self {
        let sw = f64::from(src_width.max(1));
        let sh = f64::from(src_height.max(1));
        let tw = f64::from(target_width.max(1));
        let th = f64::from(target_height.max(1));

        let scale = (tw / sw).min(th / sh);
        let new_unpad_w = (sw * scale).round();
        let new_unpad_h = (sh * scale).round();
        let dx = (tw - new_unpad_w) * 0.5;
        let dy = (th - new_unpad_h) * 0.5;

        Self {
            scale,
            dx,
            dy,
            target_width,
            target_height,
        }
    }

    pub fn unletterbox_point(&self, x: f64, y: f64) -> TargetCorner {
        let s = self.scale.max(1e-6);
        TargetCorner::new((x - self.dx) / s, (y - self.dy) / s)
    }

    pub fn unletterbox_detections(
        &self,
        detections: Vec<NeuralNetworkPipeResult>,
    ) -> Vec<NeuralNetworkPipeResult> {
        detections
            .into_iter()
            .map(|mut d| {
                d.bbox_corners = d
                    .bbox_corners
                    .into_iter()
                    .map(|c| self.unletterbox_point(c.x, c.y))
                    .collect();
                d
            })
            .collect()
    }
}

pub trait ObjectDetector: Send + Sync {
    fn detect(
        &mut self,
        frame: &CVMat,
        nms_thresh: f64,
        box_thresh: f64,
    ) -> Vec<NeuralNetworkPipeResult>;
    fn classes(&self) -> &[String];
}

pub trait Model: Send + Sync {
    fn nickname(&self) -> &str;
    fn load_detector(&self) -> Box<dyn ObjectDetector>;
}

/// Safe fallback object detector (`org.photonvision.vision.objects.NullModel`).
#[derive(Debug, Clone, Default)]
pub struct NullModel;

impl ObjectDetector for NullModel {
    fn detect(
        &mut self,
        _frame: &CVMat,
        _nms_thresh: f64,
        _box_thresh: f64,
    ) -> Vec<NeuralNetworkPipeResult> {
        Vec::new()
    }

    fn classes(&self) -> &[String] {
        &[]
    }
}

impl Model for NullModel {
    fn nickname(&self) -> &str {
        "NullModel"
    }

    fn load_detector(&self) -> Box<dyn ObjectDetector> {
        Box::new(Self)
    }
}

/// Rockchip RKNN NPU object detector (`org.photonvision.vision.objects.RknnObjectDetector`).
#[derive(Debug, Clone)]
pub struct RknnModel {
    pub nickname: String,
    pub labels: Vec<String>,
}

impl RknnModel {
    pub fn new(nickname: impl Into<String>, labels: Vec<String>) -> Self {
        Self {
            nickname: nickname.into(),
            labels,
        }
    }
}

pub struct RknnObjectDetector {
    labels: Vec<String>,
}

impl ObjectDetector for RknnObjectDetector {
    fn detect(
        &mut self,
        _frame: &CVMat,
        _nms_thresh: f64,
        _box_thresh: f64,
    ) -> Vec<NeuralNetworkPipeResult> {
        Vec::new()
    }

    fn classes(&self) -> &[String] {
        &self.labels
    }
}

impl Model for RknnModel {
    fn nickname(&self) -> &str {
        &self.nickname
    }

    fn load_detector(&self) -> Box<dyn ObjectDetector> {
        Box::new(RknnObjectDetector {
            labels: self.labels.clone(),
        })
    }
}

/// TensorFlow Lite object detector (`org.photonvision.vision.objects.TFLiteObjectDetector`).
#[derive(Debug, Clone)]
pub struct TFLiteModel {
    pub nickname: String,
    pub labels: Vec<String>,
}

impl TFLiteModel {
    pub fn new(nickname: impl Into<String>, labels: Vec<String>) -> Self {
        Self {
            nickname: nickname.into(),
            labels,
        }
    }
}

pub struct TFLiteObjectDetector {
    labels: Vec<String>,
}

impl ObjectDetector for TFLiteObjectDetector {
    fn detect(
        &mut self,
        _frame: &CVMat,
        _nms_thresh: f64,
        _box_thresh: f64,
    ) -> Vec<NeuralNetworkPipeResult> {
        Vec::new()
    }

    fn classes(&self) -> &[String] {
        &self.labels
    }
}

impl Model for TFLiteModel {
    fn nickname(&self) -> &str {
        &self.nickname
    }

    fn load_detector(&self) -> Box<dyn ObjectDetector> {
        Box::new(TFLiteObjectDetector {
            labels: self.labels.clone(),
        })
    }
}
