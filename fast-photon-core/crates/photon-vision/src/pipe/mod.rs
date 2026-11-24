pub mod impls;
pub use impls::*;

use std::time::Instant;

/// Timed result of a `CVPipe` execution (`org.photonvision.vision.pipe.CVPipe.CVPipeResult`).
#[derive(Debug, Clone)]
pub struct CVPipeResult<O> {
    pub output: O,
    pub elapsed_nanos: u64,
}

/// Composable vision processing pipe (`org.photonvision.vision.pipe.CVPipe<I, O, P>`).
pub trait CVPipe<I, O, P> {
    fn params(&self) -> &P;
    fn set_params(&mut self, params: P);
    fn process(&mut self, input: I) -> O;

    fn run(&mut self, input: I) -> CVPipeResult<O> {
        let start = Instant::now();
        let output = self.process(input);
        CVPipeResult {
            output,
            elapsed_nanos: start.elapsed().as_nanos() as u64,
        }
    }
}

/// In-place mutating pipe (`org.photonvision.vision.pipe.MutatingPipe<I, P>`).
pub trait MutatingPipe<I, P> {
    fn params(&self) -> &P;
    fn set_params(&mut self, params: P);
    fn process_in_place(&mut self, input: &mut I);

    fn run_in_place(&mut self, input: &mut I) -> u64 {
        let start = Instant::now();
        self.process_in_place(input);
        start.elapsed().as_nanos() as u64
    }
}
