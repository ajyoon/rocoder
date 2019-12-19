use crate::audio::{Audio, AudioSpec, Sample};
use crate::math;
use std::cmp::{Ord, Ordering};
use std::time::Duration;

use crossbeam_channel::Receiver;

#[derive(Debug, Copy, Clone)]
pub struct Keyframe {
    sample_pos: usize,
    val: f32,
}

impl PartialEq for Keyframe {
    fn eq(&self, other: &Self) -> bool {
        self.sample_pos == other.sample_pos && (self.val - other.val).abs() < 0.001
    }
}

impl Eq for Keyframe {}

impl PartialOrd for Keyframe {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Keyframe {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sample_pos.cmp(&other.sample_pos)
    }
}

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
    audio_spec: AudioSpec,
    buffer: Audio<T>,
    buffer_pos: usize,
    total_samples_played: usize,
    expected_total_samples: Option<usize>,
    amp_keyframes: Vec<Keyframe>,
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
            audio_spec: *spec,
            buffer_pos: 0,
            total_samples_played: 0,
            amp_keyframes: vec![],
        }
    }

    pub fn fill_buffer(&mut self, out_buf: &mut [f32]) {
        for buffer_interleaved_samples in out_buf.chunks_mut(self.buffer.spec.channels as usize) {
            self.prune_keyframes();
            if self.buffer_pos >= self.buffer.data[0].len() {
                match self.chunk_receiver.recv() {
                    Ok(new_audio) => {
                        self.buffer = new_audio;
                        self.buffer_pos = 0;
                    }
                    Err(_) => {
                        self.state = MixerState::DONE;
                    }
                }
            }
            let amp = self.current_amp();
            for (dest, src_channel) in buffer_interleaved_samples.iter_mut().zip(&self.buffer.data)
            {
                match src_channel.get(self.buffer_pos) {
                    Some(sample) => *dest = (*sample).into_f32() * amp,
                    None => {
                        *dest = 0.0;
                    }
                }
            }
            self.buffer_pos += 1;
            self.total_samples_played += 1;
        }
    }

    /// If known, return total playback progress as a percent float
    pub fn progress(&self) -> Option<f32> {
        self.expected_total_samples
            .map(|total| (self.total_samples_played as f32 / total as f32) * 100.0)
    }

    #[inline]
    fn prune_keyframes(&mut self) {
        loop {
            if self.amp_keyframes.len() > 1 {
                if self.amp_keyframes[self.amp_keyframes.len() - 2].sample_pos
                    < self.total_samples_played
                {
                    self.amp_keyframes.pop();
                    continue;
                }
            }
            break;
        }
    }

    fn sort_keyframes(&mut self) {
        self.amp_keyframes.sort();
        self.amp_keyframes.reverse();
    }

    #[inline]
    fn current_amp(&mut self) -> f32 {
        let keyframe_len = self.amp_keyframes.len();
        if keyframe_len == 0 {
            1.0
        } else if keyframe_len == 1 {
            self.amp_keyframes[0].val
        } else {
            let prev = self.amp_keyframes[keyframe_len - 1];
            let next = self.amp_keyframes[keyframe_len - 2];
            let progress = (self.total_samples_played - prev.sample_pos) as f32
                / (next.sample_pos - prev.sample_pos) as f32;
            math::sqrt_interp(prev.val, next.val, progress)
        }
    }

    pub fn fade_from_now(&mut self, to: f32, dur: Duration) {
        // assumes that no keyframes exist in the modified window
        let current_amp = self.current_amp();
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.total_samples_played,
            val: current_amp,
        });
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.total_samples_played
                + (dur.as_secs_f32() * self.audio_spec.sample_rate as f32) as usize,
            val: to,
        });
        self.sort_keyframes();
    }

    // pub fn fade(&mut self, start: Duration, start_val: f32, dur: Duration, end_val: f32) {
    //     self.amp_keyframes.push(Keyframe {
    //         sample_pos: (start.as_secs_f32() * self.audio_spec.sample_rate as f32) as usize,
    //         val: start_val,
    //     });
    //     self.amp_keyframes.push(Keyframe {
    //         sample_pos: ((start + dur).as_secs_f32() * self.audio_spec.sample_rate as f32) as usize,
    //         val: end_val,
    //     });
    //     self.sort_keyframes();
    // }
}
