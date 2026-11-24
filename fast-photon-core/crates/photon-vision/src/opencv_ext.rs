use image::{DynamicImage, GrayImage, ImageBuffer, Luma, Rgb, RgbImage};
pub use photon_common::configuration::{
    ContourGroupingMode, ContourIntersectionDirection, ContourShape, ContourSortMode,
    ImageFlipMode, ImageRotationMode,
};
use photon_targeting::TargetCorner;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE_MAT_COUNT: AtomicUsize = AtomicUsize::new(0);

/// RAII-managed image matrix (`org.photonvision.vision.opencv.CVMat`).
/// Eliminates manual `Releasable` tracking by automatically decrementing live matrix count on `Drop`.
#[derive(Debug)]
pub struct CVMat {
    image: DynamicImage,
}

impl Clone for CVMat {
    fn clone(&self) -> Self {
        LIVE_MAT_COUNT.fetch_add(1, Ordering::Relaxed);
        Self {
            image: self.image.clone(),
        }
    }
}

impl Default for CVMat {
    fn default() -> Self {
        Self::new_rgb(1, 1)
    }
}

impl Drop for CVMat {
    fn drop(&mut self) {
        LIVE_MAT_COUNT.fetch_sub(1, Ordering::Relaxed);
    }
}

impl CVMat {
    pub fn get_mat_count() -> usize {
        LIVE_MAT_COUNT.load(Ordering::Relaxed)
    }

    pub fn from_dynamic(image: DynamicImage) -> Self {
        LIVE_MAT_COUNT.fetch_add(1, Ordering::Relaxed);
        Self { image }
    }

    pub fn new_rgb(width: u32, height: u32) -> Self {
        Self::from_dynamic(DynamicImage::ImageRgb8(RgbImage::new(
            width.max(1),
            height.max(1),
        )))
    }

    pub fn new_gray(width: u32, height: u32) -> Self {
        Self::from_dynamic(DynamicImage::ImageLuma8(GrayImage::new(
            width.max(1),
            height.max(1),
        )))
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let img = image::open(path).map_err(|e| format!("failed to open {:?}: {e}", path))?;
        Ok(Self::from_dynamic(img))
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn as_dynamic(&self) -> &DynamicImage {
        &self.image
    }

    pub fn as_dynamic_mut(&mut self) -> &mut DynamicImage {
        &mut self.image
    }

    pub fn to_rgb8(&self) -> RgbImage {
        self.image.to_rgb8()
    }

    pub fn to_luma8(&self) -> GrayImage {
        self.image.to_luma8()
    }

    pub fn encode_jpeg(&self) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let _ = self.image.write_to(&mut buf, image::ImageFormat::Jpeg);
        buf.into_inner()
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.image
            .save(path)
            .map_err(|e| format!("failed to save {:?}: {e}", path))
    }

    /// Apply rotation and flip in-place.
    pub fn rotate_and_flip(&mut self, rot: ImageRotationMode, flip: ImageFlipMode) {
        self.image = match rot {
            ImageRotationMode::DEG_0 => self.image.clone(),
            ImageRotationMode::DEG_90_CCW => self.image.rotate270(),
            ImageRotationMode::DEG_180_CCW => self.image.rotate180(),
            ImageRotationMode::DEG_270_CCW => self.image.rotate90(),
        };
        self.image = match flip {
            ImageFlipMode::NONE => self.image.clone(),
            ImageFlipMode::HORIZONTAL => self.image.fliph(),
            ImageFlipMode::VERTICAL => self.image.flipv(),
            ImageFlipMode::BOTH => self.image.fliph().flipv(),
        };
    }

    /// Resize image by integer divisor.
    pub fn resize_by_divisor(&self, divisor: u32) -> Self {
        let d = divisor.max(1);
        if d == 1 {
            return self.clone();
        }
        let nw = (self.width() / d).max(1);
        let nh = (self.height() / d).max(1);
        Self::from_dynamic(
            self.image
                .resize_exact(nw, nh, image::imageops::FilterType::Nearest),
        )
    }

    /// HSV threshold into a binary `CVMat` (`Luma8`, 0 or 255).
    /// Hue in `[0..=180]`, Saturation in `[0..=255]`, Value in `[0..=255]`.
    pub fn threshold_hsv(
        &self,
        hue: (i32, i32),
        sat: (i32, i32),
        val: (i32, i32),
        invert_hue: bool,
    ) -> Self {
        let rgb = self.image.to_rgb8();
        let (w, h) = rgb.dimensions();
        let mut mask = ImageBuffer::from_pixel(w, h, Luma([0u8]));

        let (h_min, h_max) = (hue.0.clamp(0, 180) as u8, hue.1.clamp(0, 180) as u8);
        let (s_min, s_max) = (sat.0.clamp(0, 255) as u8, sat.1.clamp(0, 255) as u8);
        let (v_min, v_max) = (val.0.clamp(0, 255) as u8, val.1.clamp(0, 255) as u8);

        for (x, y, Rgb([r, g, b])) in rgb.enumerate_pixels() {
            let (ph, ps, pv) = photon_common::util::ColorHelper::rgb_to_opencv_hsv(*r, *g, *b);
            let h_ok = if !invert_hue {
                ph >= h_min && ph <= h_max
            } else {
                ph >= h_max || ph <= h_min
            };
            let s_ok = ps >= s_min && ps <= s_max;
            let v_ok = pv >= v_min && pv <= v_max;
            if h_ok && s_ok && v_ok {
                mask.put_pixel(x, y, Luma([255u8]));
            }
        }

        Self::from_dynamic(DynamicImage::ImageLuma8(mask))
    }
}

/// 2D contour representation (`org.photonvision.vision.opencv.Contour`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contour {
    pub points: Vec<TargetCorner>,
    pub area: f64,
    pub perimeter: f64,
    pub center: TargetCorner,
    pub min_area_rect_corners: Vec<TargetCorner>,
    pub bounding_width: f64,
    pub bounding_height: f64,
    pub convex_hull: Vec<TargetCorner>,
}

impl Contour {
    pub fn from_points(points: Vec<TargetCorner>) -> Self {
        if points.is_empty() {
            return Self {
                points: Vec::new(),
                area: 0.0,
                perimeter: 0.0,
                center: TargetCorner::default(),
                min_area_rect_corners: vec![TargetCorner::default(); 4],
                bounding_width: 0.0,
                bounding_height: 0.0,
                convex_hull: Vec::new(),
            };
        }

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for p in &points {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }

        let w = (max_x - min_x).max(1.0);
        let h = (max_y - min_y).max(1.0);

        // Compute shoelace polygon area if >2 points, else bounding area
        let mut shoelace = 0.0;
        let mut perim = 0.0;
        for i in 0..points.len() {
            let p1 = points[i];
            let p2 = points[(i + 1) % points.len()];
            shoelace += p1.x * p2.y - p2.x * p1.y;
            perim += (p2.x - p1.x).hypot(p2.y - p1.y);
        }
        let area = (shoelace.abs() * 0.5).max(w * h);

        let center = TargetCorner::new((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        let rect_corners = vec![
            TargetCorner::new(min_x, max_y),
            TargetCorner::new(max_x, max_y),
            TargetCorner::new(max_x, min_y),
            TargetCorner::new(min_x, min_y),
        ];

        Self {
            convex_hull: rect_corners.clone(),
            points,
            area,
            perimeter: perim,
            center,
            min_area_rect_corners: rect_corners,
            bounding_width: w,
            bounding_height: h,
        }
    }

    pub fn combine(contours: &[Contour]) -> Option<Self> {
        let mut all_pts = Vec::new();
        for c in contours {
            all_pts.extend_from_slice(&c.min_area_rect_corners);
        }
        if all_pts.is_empty() {
            None
        } else {
            Some(Self::from_points(all_pts))
        }
    }
}

/// Fitted geometric shape (`org.photonvision.vision.opencv.CVShape`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CVShape {
    pub contour: Contour,
    pub shape: ContourShape,
    pub corners: Vec<TargetCorner>,
    pub center: TargetCorner,
    pub radius: f64,
}

impl CVShape {
    pub fn from_polygon(contour: Contour, corners: Vec<TargetCorner>) -> Self {
        let shape = match corners.len() {
            3 => ContourShape::Triangle,
            4 => ContourShape::Quadrilateral,
            _ => ContourShape::Polygon,
        };
        let center = contour.center;
        Self {
            contour,
            shape,
            corners,
            center,
            radius: 0.0,
        }
    }

    pub fn from_circle(contour: Contour, center: TargetCorner, radius: f64) -> Self {
        let corners = contour.min_area_rect_corners.clone();
        Self {
            contour,
            shape: ContourShape::Circle,
            corners,
            center,
            radius,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DualOffsetValues {
    pub first_point: TargetCorner,
    pub first_point_area: f64,
    pub second_point: TargetCorner,
    pub second_point_area: f64,
}
