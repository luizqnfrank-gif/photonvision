use crate::calibration::BoardObservation;
use crate::opencv_ext::CVMat;
use crate::pipe::MutatingPipe;
use crate::target::TrackedTarget;
use image::{DynamicImage, Rgb};
use photon_targeting::TargetCorner;

fn draw_line_rgb(img: &mut image::RgbImage, p1: TargetCorner, p2: TargetCorner, color: Rgb<u8>) {
    imageproc::drawing::draw_line_segment_mut(
        img,
        (p1.x as f32, p1.y as f32),
        (p2.x as f32, p2.y as f32),
        color,
    );
}

fn draw_polygon_outline(img: &mut image::RgbImage, corners: &[TargetCorner], color: Rgb<u8>) {
    if corners.len() < 2 {
        return;
    }
    for i in 0..corners.len() {
        let p1 = corners[i];
        let p2 = corners[(i + 1) % corners.len()];
        draw_line_rgb(img, p1, p2, color);
    }
}

// 28. Draw2dCrosshairPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct Draw2dCrosshairParams {
    pub should_draw: bool,
    pub center: TargetCorner,
}

#[derive(Debug, Default)]
pub struct Draw2dCrosshairPipe {
    params: Draw2dCrosshairParams,
}

impl MutatingPipe<CVMat, Draw2dCrosshairParams> for Draw2dCrosshairPipe {
    fn params(&self) -> &Draw2dCrosshairParams {
        &self.params
    }
    fn set_params(&mut self, params: Draw2dCrosshairParams) {
        self.params = params;
    }
    fn process_in_place(&mut self, input: &mut CVMat) {
        if !self.params.should_draw || input.is_empty() {
            return;
        }
        let mut rgb = input.to_rgb8();
        let cx = if self.params.center.x > 0.0 {
            self.params.center.x
        } else {
            f64::from(rgb.width()) * 0.5
        };
        let cy = if self.params.center.y > 0.0 {
            self.params.center.y
        } else {
            f64::from(rgb.height()) * 0.5
        };
        let green = Rgb([0u8, 255u8, 0u8]);
        draw_line_rgb(
            &mut rgb,
            TargetCorner::new(cx - 12.0, cy),
            TargetCorner::new(cx + 12.0, cy),
            green,
        );
        draw_line_rgb(
            &mut rgb,
            TargetCorner::new(cx, cy - 12.0),
            TargetCorner::new(cx, cy + 12.0),
            green,
        );
        *input.as_dynamic_mut() = DynamicImage::ImageRgb8(rgb);
    }
}

// 29. Draw2dTargetsPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct Draw2dTargetsParams {
    pub should_draw: bool,
}

#[derive(Debug, Default)]
pub struct Draw2dTargetsPipe {
    params: Draw2dTargetsParams,
}

impl MutatingPipe<(CVMat, Vec<TrackedTarget>), Draw2dTargetsParams> for Draw2dTargetsPipe {
    fn params(&self) -> &Draw2dTargetsParams {
        &self.params
    }
    fn set_params(&mut self, params: Draw2dTargetsParams) {
        self.params = params;
    }
    fn process_in_place(&mut self, (mat, targets): &mut (CVMat, Vec<TrackedTarget>)) {
        if !self.params.should_draw || mat.is_empty() || targets.is_empty() {
            return;
        }
        let mut rgb = mat.to_rgb8();
        for (idx, t) in targets.iter().enumerate() {
            let color = if idx == 0 {
                Rgb([0u8, 255u8, 0u8])
            } else {
                Rgb([255u8, 215u8, 0u8])
            };
            draw_polygon_outline(&mut rgb, &t.main_contour.min_area_rect_corners, color);
        }
        *mat.as_dynamic_mut() = DynamicImage::ImageRgb8(rgb);
    }
}

// 30. Draw3dTargetsPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct Draw3dTargetsParams {
    pub should_draw: bool,
}

#[derive(Debug, Default)]
pub struct Draw3dTargetsPipe {
    params: Draw3dTargetsParams,
}

impl MutatingPipe<(CVMat, Vec<TrackedTarget>), Draw3dTargetsParams> for Draw3dTargetsPipe {
    fn params(&self) -> &Draw3dTargetsParams {
        &self.params
    }
    fn set_params(&mut self, params: Draw3dTargetsParams) {
        self.params = params;
    }
    fn process_in_place(&mut self, (mat, targets): &mut (CVMat, Vec<TrackedTarget>)) {
        if !self.params.should_draw || mat.is_empty() || targets.is_empty() {
            return;
        }
        let mut rgb = mat.to_rgb8();
        for t in targets.iter() {
            draw_polygon_outline(&mut rgb, &t.target_corners, Rgb([255u8, 64u8, 64u8]));
        }
        *mat.as_dynamic_mut() = DynamicImage::ImageRgb8(rgb);
    }
}

// 31. Draw2dAprilTagsPipe
pub type Draw2dAprilTagsParams = Draw2dTargetsParams;
pub type Draw2dAprilTagsPipe = Draw2dTargetsPipe;

// 32. Draw3dAprilTagsPipe
pub type Draw3dAprilTagsParams = Draw3dTargetsParams;
pub type Draw3dAprilTagsPipe = Draw3dTargetsPipe;

// 33. Draw2dArucoPipe
pub type Draw2dArucoParams = Draw2dTargetsParams;
pub type Draw2dArucoPipe = Draw2dTargetsPipe;

// 34. Draw3dArucoPipe
pub type Draw3dArucoParams = Draw3dTargetsParams;
pub type Draw3dArucoPipe = Draw3dTargetsPipe;

// 35. DrawCalibrationPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawCalibrationParams {
    pub should_draw: bool,
}

#[derive(Debug, Default)]
pub struct DrawCalibrationPipe {
    params: DrawCalibrationParams,
}

impl MutatingPipe<(CVMat, Option<BoardObservation>), DrawCalibrationParams>
    for DrawCalibrationPipe
{
    fn params(&self) -> &DrawCalibrationParams {
        &self.params
    }
    fn set_params(&mut self, params: DrawCalibrationParams) {
        self.params = params;
    }
    fn process_in_place(&mut self, (mat, obs): &mut (CVMat, Option<BoardObservation>)) {
        if !self.params.should_draw || mat.is_empty() {
            return;
        }
        let Some(board) = obs else {
            return;
        };
        let mut rgb = mat.to_rgb8();
        let cyan = Rgb([0u8, 255u8, 255u8]);
        for i in 0..board.location_in_image_space.len().saturating_sub(1) {
            let p1 = board.location_in_image_space[i];
            let p2 = board.location_in_image_space[i + 1];
            draw_line_rgb(
                &mut rgb,
                TargetCorner::new(p1.0, p1.1),
                TargetCorner::new(p2.0, p2.1),
                cyan,
            );
        }
        *mat.as_dynamic_mut() = DynamicImage::ImageRgb8(rgb);
    }
}
