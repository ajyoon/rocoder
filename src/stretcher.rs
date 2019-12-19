use crate::crossfade;
use crate::fft::ReFFT;
use crate::resampler;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::cmp;
use std::path::PathBuf;
use std::time::Duration;
use stopwatch::Stopwatch;

pub async fn stretch(
    sample_rate: u32,
    samples: Vec<f32>,
    factor: f32,
    amplitude: f32,
    pitch_multiple: i8,
    window: Vec<f32>,
    channel_name: String,
    frequency_kernel_src: Option<PathBuf>,
) -> Vec<f32> {
    debug_assert!(pitch_multiple != 0);
    let pitch_shifted_factor = if pitch_multiple < 0 {
        factor / pitch_multiple.abs() as f32
    } else {
        factor * pitch_multiple.abs() as f32
    };

    let stretched = stretch_without_pitch_shift(
        sample_rate,
        samples,
        pitch_shifted_factor,
        amplitude,
        window,
        channel_name,
        frequency_kernel_src,
    )
    .await;
    if pitch_multiple == 1 {
        stretched
    } else {
        resampler::resample(stretched, pitch_multiple)
    }
}

async fn stretch_without_pitch_shift(
    sample_rate: u32,
    samples: Vec<f32>,
    factor: f32,
    amplitude: f32,
    window: Vec<f32>,
    channel_name: String,
    frequency_kernel_src: Option<PathBuf>,
) -> Vec<f32> {
    let window_size = window.len();
    let half_window_size = window_size / 2;
    let amp_correction_envelope = crossfade::hanning_crossfade_compensation(half_window_size);
    let mut re_fft = ReFFT::new(sample_rate as usize, window, frequency_kernel_src);
    let sample_step_size = (window_size as f32 / (factor * 2.0)) as usize;
    let mut previous_fft_result = vec![0.0; window_size];
    let mut output = vec![];
    let mut stats = Stats::new(
        sample_rate,
        Duration::from_secs(1),
        Some((samples.len() as f32 * factor) as usize),
        channel_name,
    );

    // correct for power lost in resynth - correction curve approx by trial and error
    let amp_factor = (4f32).max(factor / 4.0) * amplitude;

    for start_pos in (0..samples.len()).step_by(sample_step_size) {
        let samples_end_idx = cmp::min(samples.len(), start_pos + window_size);
        // todo output.len() here is not actually the dest sample pos due to pitch shift
        let fft_result = re_fft.resynth(output.len(), &samples[start_pos..samples_end_idx]);
        let step_output: Vec<f32> = (0..half_window_size)
            .map(|i| {
                (previous_fft_result.get(half_window_size + i).unwrap()
                    + fft_result.get(i).unwrap())
                    * amp_correction_envelope[i]
                    * amp_factor
            })
            .collect();
        stats.collect(step_output.len());
        output.extend(step_output);
        previous_fft_result = fft_result;
    }

    output
}

struct Stats {
    sample_rate: u32,
    report_interval: Duration,
    total_samples: Option<usize>,
    interval_timer: Stopwatch,
    samples_generated_in_interval: usize,
    total_samples_generated: usize,
    channel_name: String,
}

impl Stats {
    fn new(
        sample_rate: u32,
        report_interval: Duration,
        total_samples: Option<usize>,
        channel_name: String,
    ) -> Stats {
        Stats {
            sample_rate,
            report_interval,
            total_samples,
            channel_name,
            interval_timer: Stopwatch::new(),
            samples_generated_in_interval: 0,
            total_samples_generated: 0,
        }
    }

    fn collect(&mut self, samples_generated: usize) {
        if !self.interval_timer.is_running() {
            self.start_interval();
        }
        self.total_samples_generated += samples_generated;
        self.samples_generated_in_interval += samples_generated;
        if self.interval_timer.elapsed() > self.report_interval {
            self.report();
            self.start_interval();
        }
    }

    fn start_interval(&mut self) {
        self.interval_timer = Stopwatch::start_new();
        self.samples_generated_in_interval = 0;
    }

    fn report(&mut self) {
        let elapsed_ms = self.interval_timer.elapsed_ms();
        let samples_per_second =
            (self.samples_generated_in_interval as f32 / (elapsed_ms as f32 / 1000.0)) as u32;
        let realtime_factor = samples_per_second / self.sample_rate;
        let progress_percent = self
            .total_samples
            .map(|t| (self.total_samples_generated as f32 / t as f32) * 100.0);
        let approx_secs_remaining = self
            .total_samples
            .map(|t| (t - self.total_samples_generated) / samples_per_second as usize);

        let speed_msg = format!(
            "{}k samples / sec ({:.1}x)",
            samples_per_second / 1000,
            realtime_factor
        );
        let progress_msg = self.total_samples.map_or("".to_owned(), |_| {
            format!(
                "; {:.1}%, ~{}s remaining",
                progress_percent.unwrap(),
                approx_secs_remaining.unwrap()
            )
        });

        println!(
            "Channel {}: {}{}",
            self.channel_name, speed_msg, progress_msg
        );
    }
}
