use fast_photon_core::common::configuration::{CVPipelineSettings, QuirkyCamera};
use fast_photon_core::vision::frame::{FileFrameProvider, FrameProvider};
use fast_photon_core::vision::opencv_ext::CVMat;
use fast_photon_core::vision::pipeline::{CVPipeline, ReflectivePipeline};
use std::time::Instant;

fn main() {
    let mut provider = FileFrameProvider::from_mat(CVMat::new_rgb(640, 480), 70.0, None);
    let mut pipeline = ReflectivePipeline::new(CVPipelineSettings::default());
    let quirks = QuirkyCamera::default();

    let iterations = 100u32;
    let start = Instant::now();
    for _ in 0..iterations {
        let frame = provider.get_frame();
        let _ = pipeline.run(frame, &quirks);
    }
    let elapsed = start.elapsed();
    println!(
        "Executed {iterations} ReflectivePipeline frames in {:?} ({:.2} ms/frame)",
        elapsed,
        elapsed.as_secs_f64() * 1000.0 / f64::from(iterations)
    );
}
