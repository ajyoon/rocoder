use crate::hotswapper;
use crossbeam_channel::Receiver;
use libloading::{Library, Symbol};
use rand::Rng;
use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::f32;
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const TWO_PI: f32 = f32::consts::PI;

pub struct ReFFT {
    forward_fft: Arc<dyn Fft<f32>>,
    inverse_fft: Arc<dyn Fft<f32>>,
    window_len: usize,
    window: Vec<f32>,
    kernel_recv: Option<Receiver<Library>>,
    kernels: Vec<Library>,
}

impl ReFFT {
    pub fn new(window: Vec<f32>, kernel_src: Option<PathBuf>) -> ReFFT {
        let window_len = window.len();
        let mut planner = FftPlanner::new();
        let forward_fft = planner.plan_fft_forward(window_len);
        let inverse_fft = planner.plan_fft_inverse(window_len);
        let kernel_recv = kernel_src.map(|src| hotswapper::hotswap(src).unwrap());
        ReFFT {
            forward_fft,
            inverse_fft,
            window_len,
            window,
            kernel_recv,
            kernels: vec![],
        }
    }

    pub fn resynth(&mut self, samples: &[f32]) -> Vec<f32> {
        let mut fft_result = self.forward_fft(samples);
        if self.kernel_recv.is_some() {
            fft_result = self.apply_kernel_to_fft_result(fft_result);
        }
        self.resynth_from_fft_result(fft_result)
    }

    fn forward_fft(&self, samples: &[f32]) -> Vec<Complex32> {
        let mut buf: Vec<Complex32> = samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex32::new(s * w, 0.0))
            .collect();
        if buf.len() < self.window_len {
            buf.extend(vec![Complex32::new(0.0, 0.0); self.window_len - buf.len()]);
        }
        self.forward_fft.process(&mut buf);
        buf
    }

    fn resynth_from_fft_result(&self, fft_result: Vec<Complex32>) -> Vec<f32> {
        let mut rng = rand::thread_rng();
        let mut buf: Vec<Complex32> = fft_result
            .iter()
            .map(|c| Complex32::new(0.0, rng.gen_range(0.0..TWO_PI)).exp() * c.norm())
            .collect();
        self.inverse_fft.process(&mut buf);
        buf.iter()
            .zip(&self.window)
            .map(|(c, w)| (c.re / self.window_len as f32) * w)
            .collect()
    }

    fn apply_kernel_to_fft_result(&mut self, fft_result: Vec<Complex32>) -> Vec<Complex32> {
        // use catch_unwind to make sure we dont use the new lib if its call panics
        if let Ok(lib) = self.kernel_recv.as_ref().unwrap().try_recv() {
            info!("Got new kernel");
            self.kernels.push(lib);
        }
        let maybe_lib = self.kernels.last();
        if maybe_lib.is_none() {
            return fft_result;
        }
        let kernel_input = fft_result.iter().map(|c| (c.re, c.im)).collect();
        let lib = maybe_lib.unwrap();
        match panic::catch_unwind(move || {
            let time_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as usize;
            let symbol: Symbol<fn(usize, Vec<(f32, f32)>) -> Vec<(f32, f32)>> =
                unsafe { lib.get(b"apply\0").unwrap() };
            let kernel_output = symbol(time_ms, kernel_input);
            kernel_output
                .iter()
                .map(|c| Complex32 { re: c.0, im: c.1 })
                .collect()
        }) {
            Ok(applied) => applied,
            Err(_) => {
                warn!("kernel panicked, retrying with last or noop.");
                self.kernels.pop();
                self.apply_kernel_to_fft_result(fft_result)
            }
        }
    }
}
