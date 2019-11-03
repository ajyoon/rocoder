use crate::fft::ReFFT;
use std::cmp;

pub fn stretch(samples: &[f32], factor: f32, window: Vec<f32>) -> Vec<f32> {
    let window_size = window.len();
    let half_window_size = window_size / 2;
    let re_fft = ReFFT::new(window);
    let sample_step_size = (window_size as f32 / (factor * 2.0)) as usize;
    let mut previous_fft_result = vec![0.0; window_size];
    let mut output = vec![];

    for start_pos in (0..samples.len()).step_by(sample_step_size) {
        let samples_end_idx = cmp::min(samples.len(), start_pos + window_size);
        let fft_result = re_fft.resynth(&samples[start_pos..samples_end_idx]);
        let iter_output: Vec<f32> = (0..half_window_size)
            .map(|i| {
                previous_fft_result.get(half_window_size + i).unwrap() + fft_result.get(i).unwrap()
            })
            .collect();
        output.extend(iter_output);
        previous_fft_result = fft_result;
    }

    output
}
