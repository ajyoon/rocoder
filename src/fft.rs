use ocl::{self, ProQue, SpatialDims};
use rand::Rng;
use rustfft::num_complex::Complex;
use rustfft::num_traits::Zero;
use rustfft::{FFTplanner, FFT};
use std::f32;
use std::sync::Arc;
use stopwatch::Stopwatch;

const TWO_PI: f32 = f32::consts::PI;

pub struct ReFFT {
    sample_rate: u32,
    forward_fft: Arc<dyn FFT<f32>>,
    inverse_fft: Arc<dyn FFT<f32>>,
    window_len: usize,
    window: Vec<f32>,
    kernel_program: Option<ProQue>,
}

impl ReFFT {
    pub fn new(sample_rate: u32, window: Vec<f32>, kernel_src: Option<String>) -> ReFFT {
        let window_len = window.len();
        let mut forward_planner = FFTplanner::new(false);
        let forward_fft = forward_planner.plan_fft(window_len);
        let mut inverse_planner: FFTplanner<f32> = FFTplanner::new(true);
        let inverse_fft = inverse_planner.plan_fft(window_len);
        let kernel_program = kernel_src.map(|s| Self::build_pro_que(s, window_len));
        ReFFT {
            sample_rate,
            forward_fft,
            inverse_fft,
            window_len,
            window,
            kernel_program,
        }
    }

    fn build_pro_que(src: String, window_len: usize) -> ProQue {
        let sw = Stopwatch::start_new();
        let pro_que = ProQue::builder()
            .src(src)
            .dims(SpatialDims::One(window_len))
            .build()
            .unwrap();
        info!("Created ProQue in {:?}", sw.elapsed());
        pro_que
    }

    pub fn resynth(&self, dest_sample_pos: usize, samples: &[f32]) -> Vec<f32> {
        let mut fft_result = self.forward_fft(samples);
        if self.kernel_program.is_some() {
            self.apply_opencl_kernel_to_fft_result(dest_sample_pos, &mut fft_result)
                .unwrap();
        }
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

    fn apply_opencl_kernel_to_fft_result(
        &self,
        dest_sample_pos: usize,
        fft_result: &mut Vec<Complex<f32>>,
    ) -> Result<(), ocl::Error> {
        let mut frequency_bins = fft_result.iter().map(|c| c.re).collect();
        self.apply_opencl_kernel(dest_sample_pos, &mut frequency_bins)?;
        for i in 0..fft_result.len() {
            fft_result[i].re = frequency_bins[i];
        }
        Ok(())
    }

    fn apply_opencl_kernel(
        &self,
        dest_sample_pos: usize,
        frequency_bins: &mut Vec<f32>,
    ) -> Result<(), ocl::Error> {
        let pro_que = self.kernel_program.as_ref().unwrap();
        // let sw = Stopwatch::start_new();
        let in_buf = unsafe {
            pro_que
                .buffer_builder::<f32>()
                .use_host_slice(&frequency_bins)
                .build()?
        };
        let out_buf = pro_que.create_buffer().unwrap();
        let kernel = pro_que
            .kernel_builder("transform")
            .arg(&in_buf)
            .arg(&out_buf)
            .arg((&in_buf).len() as u32)
            .arg(self.sample_rate)
            .arg(dest_sample_pos as u32)
            .build()
            .unwrap();
        unsafe {
            kernel.enq()?;
        }

        out_buf.read(frequency_bins).enq()?;
        // info!("applied kernel in {:?}", sw.elapsed());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[ignore]
    fn opencl_experiment() {
        let mut fft_result = vec![Complex::new(1.0, 2.0); 10];
        let kernel_src = r#"
            __kernel void transform(__global float const* const in_buf, __global float* const out_buf, 
                                    __private uint len, __private uint sample_rate, __private uint dest_sample_pos) {
              uint idx = get_global_id(0);
              uint dest_elapsed_s = dest_sample_pos / sample_rate;
              
              out_buf[idx] = in_buf[(idx - (dest_elapsed_s * 4)) % len];
            }
        "#;
        let re_fft = ReFFT::new(44100, vec![0.0; 10], Some(kernel_src.to_string()));
        re_fft
            .apply_opencl_kernel_to_fft_result(0, &mut fft_result)
            .unwrap();
        println!("{:?}", &fft_result[0]);
        assert_eq!(fft_result[0], Complex::new(5.0, 2.0));
        assert!(false);
    }
}
