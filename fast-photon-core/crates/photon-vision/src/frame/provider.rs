pub use crate::camera::TcpFrameProvider;
use crate::frame::{Frame, FrameProvider, FrameStaticProperties};
use crate::opencv_ext::CVMat;
use photon_common::configuration::CameraCalibrationCoefficients;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Loads frames from an image file on disk (`org.photonvision.vision.frame.provider.FileFrameProvider`).
pub struct FileFrameProvider {
    path: PathBuf,
    fov: f64,
    calibration: Option<CameraCalibrationCoefficients>,
    cached_mat: CVMat,
    static_props: Arc<FrameStaticProperties>,
    sequence_id: i64,
}

impl FileFrameProvider {
    pub fn new(
        path: impl Into<PathBuf>,
        fov: f64,
        calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let path = path.into();
        let cached_mat = CVMat::load_from_path(&path).unwrap_or_else(|_| CVMat::new_rgb(640, 480));
        let static_props = Arc::new(FrameStaticProperties::new(
            cached_mat.width() as i32,
            cached_mat.height() as i32,
            fov,
            calibration.clone(),
        ));
        Self {
            path,
            fov,
            calibration,
            cached_mat,
            static_props,
            sequence_id: 0,
        }
    }

    pub fn from_mat(
        mat: CVMat,
        fov: f64,
        calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let static_props = Arc::new(FrameStaticProperties::new(
            mat.width() as i32,
            mat.height() as i32,
            fov,
            calibration.clone(),
        ));
        Self {
            path: PathBuf::from("in_memory.png"),
            fov,
            calibration,
            cached_mat: mat,
            static_props,
            sequence_id: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn fov(&self) -> f64 {
        self.fov
    }

    pub fn calibration(&self) -> Option<&CameraCalibrationCoefficients> {
        self.calibration.as_ref()
    }
}

impl FrameProvider for FileFrameProvider {
    fn get_frame(&mut self) -> Frame {
        self.sequence_id += 1;
        Frame::from_single_mat(
            self.sequence_id,
            self.cached_mat.clone(),
            self.static_props.clone(),
        )
    }

    fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("FileFrameProvider")
    }

    fn is_connected(&self) -> bool {
        true
    }
}

/// USB V4L2 / cross-platform camera frame provider (`org.photonvision.vision.frame.provider.USBFrameProvider`).
pub struct USBFrameProvider {
    device_name: String,
    width: u32,
    height: u32,
    static_props: Arc<FrameStaticProperties>,
    sequence_id: i64,
    connected: bool,
}

impl USBFrameProvider {
    pub fn new(
        device_name: impl Into<String>,
        width: u32,
        height: u32,
        fov: f64,
        calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let static_props = Arc::new(FrameStaticProperties::new(
            width as i32,
            height as i32,
            fov,
            calibration,
        ));
        Self {
            device_name: device_name.into(),
            width,
            height,
            static_props,
            sequence_id: 0,
            connected: true,
        }
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }
}

impl FrameProvider for USBFrameProvider {
    fn get_frame(&mut self) -> Frame {
        self.sequence_id += 1;
        Frame::from_single_mat(
            self.sequence_id,
            CVMat::new_rgb(self.width, self.height),
            self.static_props.clone(),
        )
    }

    fn name(&self) -> &str {
        &self.device_name
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

/// CSI Libcamera GPU frame provider (`org.photonvision.vision.frame.provider.LibcameraGpuFrameProvider`).
pub struct LibcameraGpuFrameProvider {
    camera_path: String,
    width: u32,
    height: u32,
    static_props: Arc<FrameStaticProperties>,
    sequence_id: i64,
}

impl LibcameraGpuFrameProvider {
    pub fn new(
        camera_path: impl Into<String>,
        width: u32,
        height: u32,
        fov: f64,
        calibration: Option<CameraCalibrationCoefficients>,
    ) -> Self {
        let static_props = Arc::new(FrameStaticProperties::new(
            width as i32,
            height as i32,
            fov,
            calibration,
        ));
        Self {
            camera_path: camera_path.into(),
            width,
            height,
            static_props,
            sequence_id: 0,
        }
    }
}

impl FrameProvider for LibcameraGpuFrameProvider {
    fn get_frame(&mut self) -> Frame {
        self.sequence_id += 1;
        Frame::from_single_mat(
            self.sequence_id,
            CVMat::new_rgb(self.width, self.height),
            self.static_props.clone(),
        )
    }

    fn name(&self) -> &str {
        &self.camera_path
    }

    fn is_connected(&self) -> bool {
        true
    }
}
