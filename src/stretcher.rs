use crate::audio::{Audio, AudioSpec, Sample};
use crate::crossfade;
use crate::fft::ReFFT;
use crate::resampler;
use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use std::cmp;
use std::default::Default;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use stopwatch::Stopwatch;

const BUFFER_DUR: Duration = Duration::from_millis(500);

/// concurrent vocoder for one channel of audio
pub struct Stretcher {
    spec: AudioSpec,
    input: Receiver<Vec<f32>>,
    input_buf: Vec<f32>,
    input_buf_pos: usize,
    output_buf: Vec<f32>,
    output_buf_pos: usize,
    pitch_shifted_factor: f32,
    corrected_amp_factor: f32,
    pitch_multiple: i8,
    amp_correction_envelope: Vec<f32>,
    re_fft: ReFFT,
    window_len: usize,
    half_window_len: usize,
    sample_step_len: usize,
    done: bool,
    buffer_dur: Duration,
}

impl Stretcher {
    pub fn new(
        spec: AudioSpec,
        input: Receiver<Vec<f32>>,
        factor: f32,
        amplitude: f32,
        pitch_multiple: i8,
        window: Vec<f32>,
        buffer_dur: Duration,
        frequency_kernel_src: Option<PathBuf>,
    ) -> Stretcher {
        assert!(pitch_multiple != 0);
        let pitch_shifted_factor = if pitch_multiple < 0 {
            factor / pitch_multiple.abs() as f32
        } else {
            factor * pitch_multiple.abs() as f32
        };
        // correct for power lost in resynth - correction curve approx by trial and error
        let corrected_amp_factor = (4f32).max(pitch_shifted_factor / 4.0) * amplitude;
        let window_len = window.len();
        let half_window_len = window_len / 2;
        let sample_step_len = (window_len as f32 / (pitch_shifted_factor * 2.0)) as usize;
        let amp_correction_envelope = crossfade::hanning_crossfade_compensation(window.len() / 2);
        let re_fft = ReFFT::new(spec.sample_rate as usize, window, frequency_kernel_src);
        Stretcher {
            spec,
            input,
            pitch_shifted_factor,
            corrected_amp_factor,
            pitch_multiple,
            amp_correction_envelope,
            re_fft,
            window_len,
            half_window_len,
            sample_step_len,
            buffer_dur,
            input_buf: vec![],
            input_buf_pos: 0,
            output_buf: vec![0.0; half_window_len],
            output_buf_pos: 0,
            done: false,
        }
    }

    fn channel_bound(&self) -> usize {
        ((self.window_len as f32 / self.spec.sample_rate as f32) / self.buffer_dur.as_secs_f32())
            .ceil() as usize
    }

    pub fn into_thread(mut self) -> Receiver<Vec<f32>> {
        let (tx, rx) = bounded(self.channel_bound());
        thread::spawn(move || {
            while !self.done {
                tx.send(self.next_window()).unwrap();
            }
        });
        rx
    }

    pub fn next_window(&mut self) -> Vec<f32> {
        let samples_needed = if self.pitch_multiple < 0 {
            (self.window_len as f32 / self.pitch_multiple.abs() as f32).ceil() as usize
        } else {
            self.window_len * self.pitch_multiple.abs() as usize
        };
        let mut iter_output_buf_pos = self.output_buf_pos;
        while self.output_buf.len() < self.output_buf_pos + samples_needed + self.half_window_len {
            // Generate output one half-window at a time, with each step leaving a half window
            // from the fade-out half of the window function for the next iteration to pick up.
            self.ensure_input_samples_available(self.window_len);
            let input_end_idx = self.input_buf_pos + self.window_len;
            let fft_result = self
                .re_fft
                .resynth(&self.input_buf[self.input_buf_pos..input_end_idx]);
            for i in 0..self.half_window_len {
                self.output_buf[iter_output_buf_pos + i] = (fft_result[i]
                    + self.output_buf[iter_output_buf_pos + i])
                    * self.amp_correction_envelope[i]
                    * self.corrected_amp_factor;
            }
            self.output_buf
                .extend_from_slice(&fft_result[self.half_window_len..]);
            iter_output_buf_pos += self.half_window_len;
            self.input_buf_pos += self.sample_step_len;
        }
        let result = resampler::resample(
            &self.output_buf[self.output_buf_pos..self.output_buf_pos + samples_needed],
            self.pitch_multiple,
        );
        self.output_buf_pos += samples_needed;
        debug_assert!(result.len() == self.window_len);
        result
    }

    pub fn ensure_input_samples_available(&mut self, n: usize) {
        while self.input_buf_pos + n > self.input_buf.len() {
            match self.input.recv() {
                Ok(chunk) => {
                    self.input_buf.extend(chunk);
                }
                Err(_) => {
                    self.input_buf.resize(self.input_buf_pos + n, 0.0);
                    self.done = true;
                }
            }
        }
    }
}
