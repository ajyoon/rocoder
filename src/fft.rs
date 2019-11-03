use rand::Rng;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::{FFTplanner, FFT};
use std::f32;
use std::sync::Arc;

use hound;

const TWO_PI: f32 = f32::consts::PI;

pub struct ReFFT {
    forward_fft: Arc<dyn FFT<f32>>,
    inverse_fft: Arc<dyn FFT<f32>>,
    window_len: usize,
    window: Vec<f32>,
}

impl ReFFT {
    pub fn new(window: Vec<f32>) -> ReFFT {
        let window_len = window.len();
        let mut forward_planner = FFTplanner::new(false);
        let forward_fft = forward_planner.plan_fft(window_len);
        let mut inverse_planner: FFTplanner<f32> = FFTplanner::new(true);
        let inverse_fft = inverse_planner.plan_fft(window_len);
        ReFFT {
            forward_fft,
            inverse_fft,
            window_len,
            window,
        }
    }

    pub fn resynth(&self, samples: &[f32]) -> Vec<f32> {
        let fft_result = self.forward_fft(samples);
        self.resynth_from_fft_result(fft_result)
    }

    fn forward_fft(&self, samples: &[f32]) -> Vec<Complex<f32>> {
        let mut input: Vec<Complex<f32>> = samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();
        if input.len() < self.window_len {
            input.extend(vec![Complex::new(0.0, 0.0); self.window_len - input.len()]);
        }
        let mut output: Vec<Complex<f32>> = vec![Complex::zero(); self.window_len];
        self.forward_fft.process(&mut input, &mut output);
        output
    }

    fn resynth_from_fft_result(&self, fft_result: Vec<Complex<f32>>) -> Vec<f32> {
        let mut rng = rand::thread_rng();
        let mut input: Vec<Complex<f32>> = fft_result
            .iter()
            .map(|c| Complex::new(0.0, rng.gen_range(0.0, TWO_PI)).exp() * c.norm())
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
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::windows;
    use stopwatch::Stopwatch;
    extern crate test;
    use crate::wav;

    #[test]
    #[ignore]
    fn t() {
        const SAMPLES: usize = 44000;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let input_samples: Vec<f32> =
            wav::read("/home/ayoon/tools/paulstretch_python/test_short.wav")
                .into_iter()
                .take(SAMPLES)
                .collect();

        let synth = ReFFT::new(windows::hanning(SAMPLES));
        let sw = Stopwatch::start_new();
        let output = synth.resynth(&input_samples);
        println!(
            "resynthesized {}s of sound in {}s",
            SAMPLES as f32 / 44100.0,
            sw.elapsed_ms() as f32 / 1000.0
        );

        wav::write("out.wav", &output);
        assert!(false);
    }
}
