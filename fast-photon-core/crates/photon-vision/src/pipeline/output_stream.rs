use crate::frame::Frame;
use crate::pipe::*;
use crate::target::TrackedTarget;
use photon_common::configuration::{CVPipelineSettings, PipelineType};
use photon_targeting::TargetCorner;

/// Decoupled output stream overlay rendering pipeline (`org.photonvision.vision.pipeline.OutputStreamPipeline`).
#[derive(Debug, Default)]
pub struct OutputStreamPipeline {
    output_mat_pipe: OutputMatPipe,
    resize_pipe: ResizeImagePipe,
    crosshair_pipe: Draw2dCrosshairPipe,
    draw_2d_targets_pipe: Draw2dTargetsPipe,
    draw_3d_targets_pipe: Draw3dTargetsPipe,
    draw_2d_apriltags_pipe: Draw2dAprilTagsPipe,
    draw_3d_apriltags_pipe: Draw3dAprilTagsPipe,
    draw_2d_aruco_pipe: Draw2dArucoPipe,
    draw_3d_aruco_pipe: Draw3dArucoPipe,
}

impl OutputStreamPipeline {
    pub fn process(
        &mut self,
        mut frame: Frame,
        settings: &CVPipelineSettings,
        targets: Vec<TrackedTarget>,
    ) -> Frame {
        self.resize_pipe.set_params(ResizeImageParams {
            divisor: settings.streaming_frame_divisor,
        });
        frame.color_image = self.resize_pipe.run(&frame.color_image).output;
        frame.processed_image = self.output_mat_pipe.run(&frame.processed_image).output;
        frame.processed_image = self.resize_pipe.run(&frame.processed_image).output;

        if settings.output_should_draw {
            self.crosshair_pipe.set_params(Draw2dCrosshairParams {
                should_draw: true,
                center: TargetCorner::new(
                    frame.static_properties.center_x,
                    frame.static_properties.center_y,
                ),
            });
            self.crosshair_pipe.run_in_place(&mut frame.processed_image);

            let mut tuple = (frame.processed_image, targets);
            match settings.pipeline_type {
                PipelineType::AprilTag => {
                    self.draw_2d_apriltags_pipe
                        .set_params(Draw2dTargetsParams { should_draw: true });
                    self.draw_2d_apriltags_pipe.run_in_place(&mut tuple);
                    if settings.solve_pnp_enabled {
                        self.draw_3d_apriltags_pipe
                            .set_params(Draw3dTargetsParams { should_draw: true });
                        self.draw_3d_apriltags_pipe.run_in_place(&mut tuple);
                    }
                }
                PipelineType::Aruco => {
                    self.draw_2d_aruco_pipe
                        .set_params(Draw2dTargetsParams { should_draw: true });
                    self.draw_2d_aruco_pipe.run_in_place(&mut tuple);
                    if settings.solve_pnp_enabled {
                        self.draw_3d_aruco_pipe
                            .set_params(Draw3dTargetsParams { should_draw: true });
                        self.draw_3d_aruco_pipe.run_in_place(&mut tuple);
                    }
                }
                _ => {
                    self.draw_2d_targets_pipe
                        .set_params(Draw2dTargetsParams { should_draw: true });
                    self.draw_2d_targets_pipe.run_in_place(&mut tuple);
                    if settings.solve_pnp_enabled {
                        self.draw_3d_targets_pipe
                            .set_params(Draw3dTargetsParams { should_draw: true });
                        self.draw_3d_targets_pipe.run_in_place(&mut tuple);
                    }
                }
            }
            frame.processed_image = tuple.0;
        }
        frame
    }
}
