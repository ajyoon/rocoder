use anyhow::Result;
use rand::Rng;
use rustfft::num_complex::Complex32;
use rustfft::num_traits::Zero;
use rustfft::{FFTplanner, FFT};
use std::f32;
use std::sync::Arc;

use crate::opencl::OpenClProgram;

const TWO_PI: f32 = f32::consts::PI;

pub struct ReFFT {
    sample_rate: u32,
    forward_fft: Arc<dyn FFT<f32>>,
    inverse_fft: Arc<dyn FFT<f32>>,
    window_len: usize,
    window: Vec<f32>,
    kernel_program: Option<OpenClProgram>,
}

impl ReFFT {
    pub fn new(sample_rate: u32, window: Vec<f32>, kernel_src: Option<String>) -> ReFFT {
        let window_len = window.len();
        let mut forward_planner = FFTplanner::new(false);
        let forward_fft = forward_planner.plan_fft(window_len);
        let mut inverse_planner: FFTplanner<f32> = FFTplanner::new(true);
        let inverse_fft = inverse_planner.plan_fft(window_len);
        let kernel_program = kernel_src.map(|s| OpenClProgram::new(s, window_len));
        ReFFT {
            sample_rate,
            forward_fft,
            inverse_fft,
            window_len,
            window,
            kernel_program,
        }
    }

    pub fn resynth(&self, dest_sample_pos: usize, samples: &[f32]) -> Vec<f32> {
        let mut fft_result = self.forward_fft(samples);
        if self.kernel_program.is_some() {
            self.apply_opencl_kernel_to_fft_result(dest_sample_pos, &mut fft_result)
                .unwrap();
        }
        self.resynth_from_fft_result(fft_result)
    }

    fn forward_fft(&self, samples: &[f32]) -> Vec<Complex32> {
        let mut input: Vec<Complex32> = samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex32::new(s * w, 0.0))
            .collect();
        if input.len() < self.window_len {
            input.extend(vec![
                Complex32::new(0.0, 0.0);
                self.window_len - input.len()
            ]);
        }
        let mut output: Vec<Complex32> = vec![Complex32::zero(); self.window_len];
        self.forward_fft.process(input.as_mut_slice(), &mut output);
        output
    }

    fn resynth_from_fft_result(&self, fft_result: Vec<Complex32>) -> Vec<f32> {
        let mut rng = rand::thread_rng();
        let mut input: Vec<Complex32> = fft_result
            .iter()
            .map(|c| Complex32::new(0.0, rng.gen_range(0.0, TWO_PI)).exp() * c.norm())
            .collect();
        // reuse fft_result for output to skip another allocation
        let mut output = fft_result;
        self.inverse_fft.process(&mut input, &mut output);
        output
            .iter()
            .zip(&self.window)
            .map(|(c, w)| (c.re / self.window_len as f32) * w)
            .collect()
    }

    fn apply_opencl_kernel_to_fft_result(
        &self,
        dest_sample_pos: usize,
        fft_result: &mut Vec<Complex32>,
    ) -> Result<()> {
        self.kernel_program
            .as_ref()
            .unwrap()
            .apply_fft_transform(
                fft_result,
                (dest_sample_pos as u32 * 1000) / self.sample_rate,
            )
            .unwrap();
        Ok(())
    }
}
