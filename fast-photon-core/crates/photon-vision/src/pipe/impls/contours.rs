use crate::frame::FrameStaticProperties;
use crate::opencv_ext::{
    CVMat, CVShape, Contour, ContourGroupingMode, ContourIntersectionDirection, ContourShape,
    ContourSortMode, DualOffsetValues,
};
use crate::pipe::CVPipe;
use crate::target::{
    PotentialTarget, RobotOffsetPointMode, TargetCalculationParameters, TargetCalculations,
    TargetOffsetPointEdge, TrackedTarget,
};
use photon_common::util::DoubleCouple;
use photon_targeting::TargetCorner;

// 8. FindContoursPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct FindContoursParams;

#[derive(Debug, Default)]
pub struct FindContoursPipe {
    params: FindContoursParams,
}

impl CVPipe<&CVMat, Vec<Contour>, FindContoursParams> for FindContoursPipe {
    fn params(&self) -> &FindContoursParams {
        &self.params
    }
    fn set_params(&mut self, params: FindContoursParams) {
        self.params = params;
    }
    fn process(&mut self, input: &CVMat) -> Vec<Contour> {
        let gray = input.to_luma8();
        let raw_contours = imageproc::contours::find_contours::<i32>(&gray);
        let mut out = Vec::new();
        for c in raw_contours {
            // Keep external contours (no parent) with at least 3 points
            if c.parent.is_none() && c.points.len() >= 3 {
                let pts: Vec<TargetCorner> = c
                    .points
                    .into_iter()
                    .map(|p| TargetCorner::new(f64::from(p.x), f64::from(p.y)))
                    .collect();
                let contour = Contour::from_points(pts);
                if contour.area >= 2.0 {
                    out.push(contour);
                }
            }
        }
        out
    }
}

// 9. SpeckleRejectPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct SpeckleRejectParams {
    pub min_percent_of_avg: f64,
}

#[derive(Debug, Default)]
pub struct SpeckleRejectPipe {
    params: SpeckleRejectParams,
}

impl CVPipe<Vec<Contour>, Vec<Contour>, SpeckleRejectParams> for SpeckleRejectPipe {
    fn params(&self) -> &SpeckleRejectParams {
        &self.params
    }
    fn set_params(&mut self, params: SpeckleRejectParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<Contour>) -> Vec<Contour> {
        if input.is_empty() {
            return input;
        }
        let avg_area: f64 = input.iter().map(|c| c.area).sum::<f64>() / (input.len() as f64);
        let min_allowed = (self.params.min_percent_of_avg / 100.0) * avg_area;
        input
            .into_iter()
            .filter(|c| c.area >= min_allowed)
            .collect()
    }
}

// 10. FilterContoursPipe
#[derive(Debug, Clone)]
pub struct FilterContoursParams {
    pub area: DoubleCouple,
    pub ratio: DoubleCouple,
    pub fullness: DoubleCouple,
    pub frame_properties: FrameStaticProperties,
    pub is_landscape: bool,
}

impl Default for FilterContoursParams {
    fn default() -> Self {
        Self {
            area: DoubleCouple::new(0.0, 100.0),
            ratio: DoubleCouple::new(0.0, 20.0),
            fullness: DoubleCouple::new(0.0, 100.0),
            frame_properties: FrameStaticProperties::default(),
            is_landscape: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct FilterContoursPipe {
    params: FilterContoursParams,
}

impl CVPipe<Vec<Contour>, Vec<Contour>, FilterContoursParams> for FilterContoursPipe {
    fn params(&self) -> &FilterContoursParams {
        &self.params
    }
    fn set_params(&mut self, params: FilterContoursParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<Contour>) -> Vec<Contour> {
        let img_area = self.params.frame_properties.image_area.max(1.0);
        input
            .into_iter()
            .filter(|c| {
                let area_pct = (c.area / img_area) * 100.0;
                if area_pct < self.params.area.first || area_pct > self.params.area.second {
                    return false;
                }
                let aspect = TargetCalculations::calculate_aspect_ratio(
                    c.bounding_width,
                    c.bounding_height,
                    self.params.is_landscape,
                );
                if aspect < self.params.ratio.first || aspect > self.params.ratio.second {
                    return false;
                }
                let bbox_area = (c.bounding_width * c.bounding_height).max(1.0);
                let fullness_pct = ((c.area / bbox_area) * 100.0).clamp(0.0, 100.0);
                fullness_pct >= self.params.fullness.first
                    && fullness_pct <= self.params.fullness.second
            })
            .collect()
    }
}

// 11. GroupContoursPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct GroupContoursParams {
    pub group: ContourGroupingMode,
    pub intersection: ContourIntersectionDirection,
}

#[derive(Debug, Default)]
pub struct GroupContoursPipe {
    params: GroupContoursParams,
}

impl CVPipe<Vec<Contour>, Vec<PotentialTarget>, GroupContoursParams> for GroupContoursPipe {
    fn params(&self) -> &GroupContoursParams {
        &self.params
    }
    fn set_params(&mut self, params: GroupContoursParams) {
        self.params = params;
    }
    fn process(&mut self, mut input: Vec<Contour>) -> Vec<PotentialTarget> {
        input.sort_by(|a, b| {
            b.area
                .partial_cmp(&a.area)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        match self.params.group {
            ContourGroupingMode::Single => input
                .into_iter()
                .map(PotentialTarget::from_contour)
                .collect(),
            ContourGroupingMode::Two => {
                if input.len() < 2 {
                    return Vec::new();
                }
                let mut out = Vec::new();
                for i in 0..input.len() - 1 {
                    if let Some(grouped) = PotentialTarget::from_contour_group(vec![
                        input[i].clone(),
                        input[i + 1].clone(),
                    ]) {
                        out.push(grouped);
                    }
                }
                out
            }
            ContourGroupingMode::TwoOrMore => {
                if input.len() < 2 {
                    return Vec::new();
                }
                PotentialTarget::from_contour_group(input)
                    .into_iter()
                    .collect()
            }
        }
    }
}

// 12. SortContoursPipe
#[derive(Debug, Clone)]
pub struct SortContoursParams {
    pub sort_mode: ContourSortMode,
    pub max_targets: usize,
    pub frame_properties: FrameStaticProperties,
}

impl Default for SortContoursParams {
    fn default() -> Self {
        Self {
            sort_mode: ContourSortMode::Largest,
            max_targets: 20,
            frame_properties: FrameStaticProperties::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SortContoursPipe {
    params: SortContoursParams,
}

impl CVPipe<Vec<PotentialTarget>, Vec<PotentialTarget>, SortContoursParams> for SortContoursPipe {
    fn params(&self) -> &SortContoursParams {
        &self.params
    }
    fn set_params(&mut self, params: SortContoursParams) {
        self.params = params;
    }
    fn process(&mut self, mut input: Vec<PotentialTarget>) -> Vec<PotentialTarget> {
        let cx = self.params.frame_properties.center_x;
        let cy = self.params.frame_properties.center_y;

        input.sort_by(|a, b| {
            let ca = &a.main_contour;
            let cb = &b.main_contour;
            let ord = match self.params.sort_mode {
                ContourSortMode::Largest => cb.area.partial_cmp(&ca.area),
                ContourSortMode::Smallest => ca.area.partial_cmp(&cb.area),
                ContourSortMode::Highest => ca.center.y.partial_cmp(&cb.center.y),
                ContourSortMode::Lowest => cb.center.y.partial_cmp(&ca.center.y),
                ContourSortMode::Rightmost => cb.center.x.partial_cmp(&ca.center.x),
                ContourSortMode::Leftmost => ca.center.x.partial_cmp(&cb.center.x),
                ContourSortMode::Centermost => {
                    let da = (ca.center.x - cx).hypot(ca.center.y - cy);
                    let db = (cb.center.x - cx).hypot(cb.center.y - cy);
                    da.partial_cmp(&db)
                }
            };
            ord.unwrap_or(std::cmp::Ordering::Equal)
        });
        input.truncate(self.params.max_targets.max(1));
        input
    }
}

// 13. FindPolygonPipe
#[derive(Debug, Clone, Copy)]
pub struct FindPolygonParams {
    pub accuracy_percentage: f64,
}

impl Default for FindPolygonParams {
    fn default() -> Self {
        Self {
            accuracy_percentage: 10.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct FindPolygonPipe {
    params: FindPolygonParams,
}

impl CVPipe<Vec<Contour>, Vec<CVShape>, FindPolygonParams> for FindPolygonPipe {
    fn params(&self) -> &FindPolygonParams {
        &self.params
    }
    fn set_params(&mut self, params: FindPolygonParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<Contour>) -> Vec<CVShape> {
        input
            .into_iter()
            .map(|c| {
                let corners = c.min_area_rect_corners.clone();
                CVShape::from_polygon(c, corners)
            })
            .collect()
    }
}

// 14. FindCirclesPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct FindCirclesParams {
    pub min_radius: f64,
    pub max_radius: f64,
}

#[derive(Debug, Default)]
pub struct FindCirclesPipe {
    params: FindCirclesParams,
}

impl CVPipe<(&CVMat, Vec<Contour>), Vec<CVShape>, FindCirclesParams> for FindCirclesPipe {
    fn params(&self) -> &FindCirclesParams {
        &self.params
    }
    fn set_params(&mut self, params: FindCirclesParams) {
        self.params = params;
    }
    fn process(&mut self, (_mat, contours): (&CVMat, Vec<Contour>)) -> Vec<CVShape> {
        contours
            .into_iter()
            .filter_map(|c| {
                let r = (c.bounding_width + c.bounding_height) * 0.25;
                if self.params.max_radius > 0.0
                    && (r < self.params.min_radius || r > self.params.max_radius)
                {
                    return None;
                }
                let center = c.center;
                Some(CVShape::from_circle(c, center, r))
            })
            .collect()
    }
}

// 15. FilterShapesPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterShapesParams {
    pub desired_shape: ContourShape,
    pub min_area: f64,
    pub max_area: f64,
}

#[derive(Debug, Default)]
pub struct FilterShapesPipe {
    params: FilterShapesParams,
}

impl CVPipe<Vec<CVShape>, Vec<CVShape>, FilterShapesParams> for FilterShapesPipe {
    fn params(&self) -> &FilterShapesParams {
        &self.params
    }
    fn set_params(&mut self, params: FilterShapesParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<CVShape>) -> Vec<CVShape> {
        input
            .into_iter()
            .filter(|s| {
                self.params.desired_shape == ContourShape::Polygon
                    || s.shape == self.params.desired_shape
            })
            .collect()
    }
}

// 16. CornerDetectionPipe
#[derive(Debug, Clone, Copy, Default)]
pub struct CornerDetectionParams {
    pub exact_side_count: Option<usize>,
}

#[derive(Debug, Default)]
pub struct CornerDetectionPipe {
    params: CornerDetectionParams,
}

impl CVPipe<Vec<PotentialTarget>, Vec<PotentialTarget>, CornerDetectionParams>
    for CornerDetectionPipe
{
    fn params(&self) -> &CornerDetectionParams {
        &self.params
    }
    fn set_params(&mut self, params: CornerDetectionParams) {
        self.params = params;
    }
    fn process(&mut self, mut input: Vec<PotentialTarget>) -> Vec<PotentialTarget> {
        for target in &mut input {
            if target.detected_corners.is_empty() {
                target.detected_corners = target.main_contour.min_area_rect_corners.clone();
            }
        }
        input
    }
}

// 17. Collect2dTargetsPipe
#[derive(Debug, Clone)]
pub struct Collect2dTargetsParams {
    pub calc_params: TargetCalculationParameters,
}

impl Default for Collect2dTargetsParams {
    fn default() -> Self {
        Self {
            calc_params: TargetCalculationParameters {
                is_landscape: true,
                target_offset_point_edge: TargetOffsetPointEdge::Center,
                robot_offset_point_mode: RobotOffsetPointMode::None,
                robot_offset_single_point: TargetCorner::default(),
                dual_offset_values: DualOffsetValues::default(),
                camera_center_point: TargetCorner::new(319.5, 239.5),
                horizontal_focal_length: 500.0,
                vertical_focal_length: 500.0,
                image_area: 640.0 * 480.0,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct Collect2dTargetsPipe {
    params: Collect2dTargetsParams,
}

impl CVPipe<Vec<PotentialTarget>, Vec<TrackedTarget>, Collect2dTargetsParams>
    for Collect2dTargetsPipe
{
    fn params(&self) -> &Collect2dTargetsParams {
        &self.params
    }
    fn set_params(&mut self, params: Collect2dTargetsParams) {
        self.params = params;
    }
    fn process(&mut self, input: Vec<PotentialTarget>) -> Vec<TrackedTarget> {
        input
            .into_iter()
            .map(|pt| TrackedTarget::from_potential_target(pt, &self.params.calc_params))
            .collect()
    }
}
