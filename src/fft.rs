use crate::hotswapper;
use anyhow::Result;
use crossbeam_channel::Receiver;
use libloading::{Library, Symbol};
use rand::Rng;
use rustfft::num_complex::Complex32;
use rustfft::num_traits::Zero;
use rustfft::{FFTplanner, FFT};
use std::f32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const TWO_PI: f32 = f32::consts::PI;

pub struct ReFFT {
    sample_rate: usize,
    forward_fft: Arc<dyn FFT<f32>>,
    inverse_fft: Arc<dyn FFT<f32>>,
    window_len: usize,
    window: Vec<f32>,
    kernel_recv: Option<Receiver<Library>>,
    kernel: Option<Library>,
}

impl ReFFT {
    pub fn new(sample_rate: usize, window: Vec<f32>, kernel_src: Option<PathBuf>) -> ReFFT {
        let window_len = window.len();
        let mut forward_planner = FFTplanner::new(false);
        let forward_fft = forward_planner.plan_fft(window_len);
        let mut inverse_planner: FFTplanner<f32> = FFTplanner::new(true);
        let inverse_fft = inverse_planner.plan_fft(window_len);
        let kernel_recv = kernel_src.map(|src| hotswapper::hotswap(src).unwrap());
        ReFFT {
            sample_rate,
            forward_fft,
            inverse_fft,
            window_len,
            window,
            kernel_recv,
            kernel: None,
        }
    }

    pub fn resynth(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut fft_result = self.forward_fft(samples);
        if self.kernel_recv.is_some() {
            self.apply_kernel_to_fft_result(&mut fft_result)
                .unwrap_or_else(|_| {
                    warn!("failed to apply kernel to fft result");
                });
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

    fn apply_kernel_to_fft_result(&mut self, fft_result: &mut Vec<Complex32>) -> Result<()> {
        if let Ok(lib) = self.kernel_recv.as_ref().unwrap().try_recv() {
            self.kernel = Some(lib)
        }
        if let Some(lib) = &self.kernel {
            let time_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as usize;
            let symbol: Symbol<fn(usize, Vec<(f32, f32)>) -> Vec<(f32, f32)>> =
                unsafe { lib.get(b"apply\0").unwrap() };
            let kernel_input = fft_result.iter().map(|c| (c.re, c.im)).collect();
            let kernel_output = symbol(time_ms, kernel_input);
            for i in 0..fft_result.len() {
                let out = kernel_output[i];
                fft_result[i] = Complex32 {
                    re: out.0,
                    im: out.1,
                };
            }
        }
        Ok(())
    }
}
