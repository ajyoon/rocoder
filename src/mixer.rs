use crate::audio::{Audio, AudioSpec, Sample};
use crate::math;

use crossbeam_channel::Receiver;
use std::sync::atomic::Ordering;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Ord, PartialOrd, Hash)]
pub enum MixerState {
    PLAYING,
    DONE,
}

pub struct Mixer<T>
where
    T: Sample,
{
    pub state: MixerState,
    chunk_receiver: Receiver<Audio<T>>,
    buffer: Audio<T>,
    buffer_pos: usize,
    total_samples_played: usize,
    expected_total_samples: Option<usize>,
}

impl<T> Mixer<T>
where
    T: Sample,
{
    pub fn new(
        spec: &AudioSpec,
        chunk_receiver: Receiver<Audio<T>>,
        expected_total_samples: Option<usize>,
    ) -> Self {
        Mixer {
            chunk_receiver,
            expected_total_samples,
            state: MixerState::PLAYING,
            buffer: Audio::from_spec(spec),
            buffer_pos: 0,
            total_samples_played: 0,
        }
    }

    pub fn fill_buffer(&mut self, out_buf: &mut [f32]) {
        for buffer_interleaved_samples in out_buf.chunks_mut(self.buffer.spec.channels as usize) {
            if self.buffer_pos >= self.buffer.data[0].len() {
                match self.chunk_receiver.recv() {
                    Ok(new_audio) => {
                        self.buffer = new_audio;
                        self.buffer_pos = 0;
                    }
                    Err(e) => {
                        self.state = MixerState::DONE;
                    }
                }
            }
            for (dest, src_channel) in buffer_interleaved_samples.iter_mut().zip(&self.buffer.data)
            {
                match src_channel.get(self.buffer_pos) {
                    Some(sample) => *dest = (*sample).into_f32(),
                    None => {
                        *dest = 0.0;
                    }
                }
            }
            self.buffer_pos += 1;
        }
        self.total_samples_played += out_buf.len();
    }

    /// If known, return total playback progress as a percent float
    pub fn progress(&self) -> Option<f32> {
        self.expected_total_samples
            .map(|total| (self.total_samples_played as f32 / total as f32) * 100.0)
    }
}
