use crate::opencv_ext::{CVMat, ImageFlipMode, ImageRotationMode};
use crate::pipe::{CVPipe, MutatingPipe};
use image::DynamicImage;
use photon_common::configuration::FrameDivisor;
use photon_common::util::IntegerCouple;
use std::time::Instant;

// 1. RotateImagePipe
#[derive(Debug, Clone, Copy, Default)]
pub struct RotateImageParams {
    pub rotation: ImageRotationMode,
    pub flip: ImageFlipMode,
}

#[derive(Debug, Default)]
pub struct RotateImagePipe {
    params: RotateImageParams,
}

impl MutatingPipe<CVMat, RotateImageParams> for RotateImagePipe {
    fn params(&self) -> &RotateImageParams {
        &self.params
    }
    fn set_params(&mut self, params: RotateImageParams) {
        self.params = params;
    }
    fn process_in_place(&mut self, input: &mut CVMat) {
        input.rotate_and_flip(self.params.rotation, self.params.flip);
    }
}

// 2. ResizeImagePipe
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeImageParams {
    pub divisor: FrameDivisor,
}

#[derive(Debug, Default)]
pub struct ResizeImagePipe {
    params: ResizeImageParams,
}

impl CVPipe<&CVMat, CVMat, ResizeImageParams> for ResizeImagePipe {
    fn params(&self) -> &ResizeImageParams {
        &self.params
    }
    fn set_params(&mut self, params: ResizeImageParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> CVMat {
        input.resize_by_divisor(self.params.divisor as u32)
    }
}

// 3. GrayscalePipe
#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleParams;

#[derive(Debug, Default)]
pub struct GrayscalePipe {
    params: GrayscaleParams,
}

impl CVPipe<&CVMat, CVMat, GrayscaleParams> for GrayscalePipe {
    fn params(&self) -> &GrayscaleParams {
        &self.params
    }
    fn set_params(&mut self, params: GrayscaleParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> CVMat {
        CVMat::from_dynamic(DynamicImage::ImageLuma8(input.to_luma8()))
    }
}

// 4. HSVPipe
#[derive(Debug, Clone, Copy)]
pub struct HSVParams {
    pub hue: IntegerCouple,
    pub saturation: IntegerCouple,
    pub value: IntegerCouple,
    pub hue_inverted: bool,
}

impl Default for HSVParams {
    fn default() -> Self {
        Self {
            hue: IntegerCouple::new(50, 180),
            saturation: IntegerCouple::new(50, 255),
            value: IntegerCouple::new(50, 255),
            hue_inverted: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct HSVPipe {
    params: HSVParams,
}

impl CVPipe<&CVMat, CVMat, HSVParams> for HSVPipe {
    fn params(&self) -> &HSVParams {
        &self.params
    }
    fn set_params(&mut self, params: HSVParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> CVMat {
        input.threshold_hsv(
            (self.params.hue.first, self.params.hue.second),
            (self.params.saturation.first, self.params.saturation.second),
            (self.params.value.first, self.params.value.second),
            self.params.hue_inverted,
        )
    }
}

// 5. OutputMatPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputMatParams;

#[derive(Debug, Default)]
pub struct OutputMatPipe {
    params: OutputMatParams,
}

impl CVPipe<&CVMat, CVMat, OutputMatParams> for OutputMatPipe {
    fn params(&self) -> &OutputMatParams {
        &self.params
    }
    fn set_params(&mut self, params: OutputMatParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> CVMat {
        CVMat::from_dynamic(DynamicImage::ImageRgb8(input.to_rgb8()))
    }
}

// 6. CalculateFPSPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct CalculateFPSParams;

#[derive(Debug)]
pub struct CalculateFPSPipe {
    params: CalculateFPSParams,
    last_time: Option<Instant>,
    rolling_fps: f64,
}

impl Default for CalculateFPSPipe {
    fn default() -> Self {
        Self {
            params: CalculateFPSParams,
            last_time: None,
            rolling_fps: 0.0,
        }
    }
}

impl CVPipe<(), i32, CalculateFPSParams> for CalculateFPSPipe {
    fn params(&self) -> &CalculateFPSParams {
        &self.params
    }
    fn set_params(&mut self, params: CalculateFPSParams) {
        self.params = params;
    }
    fn process(&mut self, _input: ()) -> i32 {
        let now = Instant::now();
        if let Some(prev) = self.last_time {
            let dt = now.duration_since(prev).as_secs_f64();
            if dt > 1e-6 {
                let inst = 1.0 / dt;
                self.rolling_fps = if self.rolling_fps == 0.0 {
                    inst
                } else {
                    self.rolling_fps * 0.9 + inst * 0.1
                };
            }
        }
        self.last_time = Some(now);
        self.rolling_fps.round() as i32
    }
}

// 7. FocusPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusParams;

#[derive(Debug, Default)]
pub struct FocusPipe {
    params: FocusParams,
}

impl CVPipe<&CVMat, (CVMat, f64), FocusParams> for FocusPipe {
    fn params(&self) -> &FocusParams {
        &self.params
    }
    fn set_params(&mut self, params: FocusParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> (CVMat, f64) {
        let gray = input.to_luma8();
        let (w, h) = gray.dimensions();
        if w < 3 || h < 3 {
            return (input.clone(), 0.0);
        }
        // Compute variance of 3x3 Laplacian
        let mut sum: f64 = 0.0;
        let mut sum_sq: f64 = 0.0;
        let mut count: f64 = 0.0;
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let c = f64::from(gray.get_pixel(x, y).0[0]);
                let n = f64::from(gray.get_pixel(x, y - 1).0[0]);
                let s = f64::from(gray.get_pixel(x, y + 1).0[0]);
                let e = f64::from(gray.get_pixel(x + 1, y).0[0]);
                let w_px = f64::from(gray.get_pixel(x - 1, y).0[0]);
                let lap = n + s + e + w_px - 4.0 * c;
                sum += lap;
                sum_sq += lap * lap;
                count += 1.0;
            }
        }
        let mean = sum / count.max(1.0);
        let variance = (sum_sq / count.max(1.0)) - (mean * mean);
        (input.clone(), variance.max(0.0))
    }
}
