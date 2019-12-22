use crate::audio::AudioSpec;
use crate::crossfade;
use crate::fft::ReFFT;
use crate::resampler;
use crossbeam_channel::{bounded, Receiver};
use slice_deque::SliceDeque;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// concurrent vocoder for one channel of audio
pub struct Stretcher {
    spec: AudioSpec,
    input: Receiver<Vec<f32>>,
    input_buf: SliceDeque<f32>,
    output_buf: SliceDeque<f32>,
    corrected_amp_factor: f32,
    pitch_multiple: i8,
    amp_correction_envelope: Vec<f32>,
    re_fft: ReFFT,
    window_len: usize,
    half_window_len: usize,
    samples_needed_per_window: usize,
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
        let window_len = window.len();
        let pitch_shifted_factor = if pitch_multiple < 0 {
            factor / pitch_multiple.abs() as f32
        } else {
            factor * pitch_multiple.abs() as f32
        };
        let samples_needed_per_window = if pitch_multiple < 0 {
            (window_len as f32 / pitch_multiple.abs() as f32).ceil() as usize
        } else {
            window_len * pitch_multiple.abs() as usize
        };
        // correct for power lost in resynth - correction curve approx by trial and error
        let corrected_amp_factor = (4f32).max(pitch_shifted_factor / 4.0) * amplitude;
        let half_window_len = window_len / 2;
        let sample_step_len = (window_len as f32 / (pitch_shifted_factor * 2.0)) as usize;
        let amp_correction_envelope = crossfade::hanning_crossfade_compensation(window.len() / 2);
        let re_fft = ReFFT::new(window, frequency_kernel_src);
        let mut output_buf = SliceDeque::with_capacity(samples_needed_per_window + half_window_len);
        output_buf.extend(vec![0.0; half_window_len]);
        Stretcher {
            spec,
            input,
            corrected_amp_factor,
            pitch_multiple,
            amp_correction_envelope,
            re_fft,
            window_len,
            half_window_len,
            samples_needed_per_window,
            sample_step_len,
            buffer_dur,
            output_buf,
            input_buf: SliceDeque::new(),
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
        debug_assert!(self.output_buf.len() == self.half_window_len);
        let mut iter_output_buf_pos = 0;
        while self.output_buf.len() < self.samples_needed_per_window + self.half_window_len {
            // Generate output one half-window at a time, with each step leaving a half window
            // from the fade-out half of the window function for the next iteration to pick up.
            self.ensure_input_samples_available(self.window_len);
            let fft_result = self.re_fft.resynth(&self.input_buf[..self.window_len]);
            for i in 0..self.half_window_len {
                self.output_buf[iter_output_buf_pos + i] = (fft_result[i]
                    + self.output_buf[iter_output_buf_pos + i])
                    * self.amp_correction_envelope[i]
                    * self.corrected_amp_factor;
            }
            self.output_buf
                .extend_from_slice(&fft_result[self.half_window_len..]);
            iter_output_buf_pos += self.half_window_len;
            self.input_buf
                .truncate_front(self.input_buf.len() - self.sample_step_len);
        }
        let result = resampler::resample(
            &self.output_buf[..self.samples_needed_per_window],
            self.pitch_multiple,
        );
        self.output_buf.truncate_front(self.half_window_len);
        debug_assert!(result.len() == self.window_len);
        result
    }

    pub fn ensure_input_samples_available(&mut self, n: usize) {
        while self.input_buf.len() < n {
            match self.input.recv() {
                Ok(chunk) => {
                    self.input_buf.extend(chunk);
                }
                Err(_) => {
                    self.input_buf.resize(n, 0.0);
                    self.done = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;
    use crossbeam_channel::{unbounded, Sender};

    #[test]
    fn ensure_input_samples_available_when_channel_closed_fills_with_zeros() {
        let (mut stretcher, tx) = basic_stretcher(1000);
        drop(tx);
        stretcher.ensure_input_samples_available(4);
        assert_eq!(stretcher.done, true);
        assert_almost_eq_by_element(stretcher.input_buf.to_vec(), vec![0.0; 4]);
    }

    #[test]
    fn ensure_input_samples_available_loading_multiple_chunks() {
        let (mut stretcher, tx) = basic_stretcher(1000);
        tx.send(vec![1.0, 2.0, 3.0]).unwrap();
        tx.send(vec![4.0, 5.0]).unwrap();
        stretcher.ensure_input_samples_available(4);
        assert_eq!(stretcher.done, false);
        assert_almost_eq_by_element(stretcher.input_buf.to_vec(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    fn basic_stretcher(window_len: usize) -> (Stretcher, Sender<Vec<f32>>) {
        let (tx, rx) = unbounded();
        let stretcher = Stretcher::new(
            AudioSpec {
                channels: 2,
                sample_rate: 44100,
            },
            rx,
            1.0,
            1.0,
            1,
            vec![1.0; window_len],
            Duration::from_secs(1),
            None,
        );
        (stretcher, tx)
    }
}
