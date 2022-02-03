use crate::math;
use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use num_traits::Num;
use std::ops::MulAssign;
use std::time::Duration;

pub trait Sample: Sized + Num + Copy + MulAssign + Send + 'static {
    fn from_i8(n: i8) -> Self;
    fn from_i16(n: i16) -> Self;
    // Rust doesn't have i24, so this assumes the i24 has been stored in an i32
    fn from_i24(n: i32) -> Self;
    fn from_i32(n: i32) -> Self;
}

impl Sample for f32 {
    fn from_i8(n: i8) -> Self {
        n as f32 / i8::max_value() as f32
    }
    fn from_i16(n: i16) -> Self {
        n as f32 / i16::max_value() as f32
    }
    fn from_i24(n: i32) -> Self {
        n as f32 / 8388608.0
    }
    fn from_i32(n: i32) -> Self {
        n as f32 / i32::max_value() as f32
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AudioSpec {
    /// Number of audio channels (e.g. 2 for stereo)
    pub channels: u16,
    /// Number of samples per second
    pub sample_rate: u32,
}

#[derive(Debug)]
pub struct Audio {
    pub data: Vec<Vec<f32>>,
    pub spec: AudioSpec,
}

impl Audio {
    pub fn from_spec(spec: &AudioSpec) -> Audio {
        let data = (0..spec.channels).map(|_| Vec::new()).collect();
        Audio { data, spec: *spec }
    }

    pub fn duration(&self) -> Duration {
        Duration::from_nanos(
            ((self.data[0].len() as f32 / self.spec.sample_rate as f32) * 1_000_000_000.0) as u64,
        )
    }

    pub fn clip_in_place(&mut self, start_offset: Option<Duration>, duration: Option<Duration>) {
        let start_sample_pos = self.resolve_start_sample_pos(start_offset);
        let end_sample_pos = self.resolve_end_sample_pos(start_sample_pos, duration);
        for channel in self.data.iter_mut() {
            *channel = channel[start_sample_pos..end_sample_pos].to_vec();
        }
    }

    pub fn amplify_in_place(&mut self, factor: f32) {
        for channel in self.data.iter_mut() {
            for i in 0..channel.len() {
                channel[i] *= factor;
            }
        }
    }

    pub fn rotate_channels(&mut self) {
        self.data.rotate_right(1);
    }

    pub fn fade_in(&mut self, start: Duration, dur: Duration) {
        self.fade_in_at_sample(self.duration_to_sample(start), self.duration_to_sample(dur))
    }

    fn fade_in_at_sample(&mut self, start: usize, dur: usize) {
        if start + dur > self.data[0].len() {
            warn!("Fade in parameters out of bounds, ignoring.");
            return;
        }
        for channel in self.data.iter_mut() {
            for i in 0..start {
                channel[i] = 0.0;
            }
            for p in 0..dur {
                channel[start + p] *= math::sqrt_interp(0.0, 1.0, p as f32 / dur as f32)
            }
        }
    }

    pub fn fade_out(&mut self, start: Duration, dur: Duration) {
        self.fade_out_at_sample(self.duration_to_sample(start), self.duration_to_sample(dur))
    }

    fn fade_out_at_sample(&mut self, start: usize, dur: usize) {
        if start + dur > self.data[0].len() {
            warn!("Fade out parameters out of bounds, ignoring.");
            return;
        }
        for channel in self.data.iter_mut() {
            for i in start + dur..channel.len() {
                channel[i] = 0.0;
            }
            for p in 0..dur {
                channel[start + p] *= math::sqrt_interp(1.0, 0.0, p as f32 / dur as f32)
            }
        }
    }

    fn resolve_start_sample_pos(&self, start_offset: Option<Duration>) -> usize {
        match start_offset {
            Some(offset) => (offset.as_secs_f64() * self.spec.sample_rate as f64) as usize,
            None => 0,
        }
    }

    fn resolve_end_sample_pos(&self, start_sample_pos: usize, duration: Option<Duration>) -> usize {
        match duration {
            Some(dur) => {
                let dur_in_samples = (dur.as_secs_f64() * self.spec.sample_rate as f64) as usize;
                start_sample_pos + dur_in_samples
            }
            None => self.data.get(0).unwrap().len(),
        }
    }

    pub fn duration_to_sample(&self, duration: Duration) -> usize {
        (duration.as_secs_f32() * self.spec.sample_rate as f32) as usize
    }

    pub fn sample_to_duration(&self, sample: usize) -> Duration {
        Duration::from_secs_f32(sample as f32 / self.spec.sample_rate as f32)
    }
}

#[derive(Debug)]
pub struct AudioBus {
    pub spec: AudioSpec,
    pub channels: Vec<Receiver<Vec<f32>>>,
    pub expected_total_samples: Option<usize>,
}

const INTO_AUDIO_DRAIN_TIMEOUT: Duration = Duration::from_millis(5);

impl AudioBus {
    /// quick and dirty collapse into audio
    pub fn into_audio(self) -> Audio {
        assert!(self.channels.len() as u16 == self.spec.channels);
        let mut out: Vec<Vec<f32>> = vec![vec![]; self.spec.channels as usize];
        loop {
            let mut disconnected_count = 0;
            for (i, channel) in self.channels.iter().enumerate() {
                match channel.recv_timeout(INTO_AUDIO_DRAIN_TIMEOUT) {
                    Ok(chunk) => out[i].extend(chunk),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => disconnected_count += 1,
                }
            }
            if disconnected_count == self.spec.channels {
                break;
            }
        }
        Audio {
            spec: self.spec,
            data: out,
        }
    }

    pub fn from_audio(audio: Audio) -> Self {
        let spec = audio.spec;
        let expected_total_samples = Some(audio.data[0].len());
        let channels: Vec<Receiver<Vec<f32>>> = audio
            .data
            .into_iter()
            .map(|channel| {
                let (tx, rx) = unbounded();
                tx.send(channel).unwrap();
                rx
            })
            .collect();
        AudioBus {
            spec,
            expected_total_samples,
            channels,
        }
    }

    pub fn from_spec(
        spec: AudioSpec,
        expected_total_samples: Option<usize>,
    ) -> (Self, Vec<Sender<Vec<f32>>>) {
        let mut senders = vec![];
        let mut receivers = vec![];
        for _ in 0..spec.channels {
            let (tx, rx) = unbounded();
            senders.push(tx);
            receivers.push(rx);
        }
        (
            AudioBus {
                spec,
                expected_total_samples,
                channels: receivers,
            },
            senders,
        )
    }

    pub fn collect_chunk(&mut self) -> Result<Audio> {
        let mut chunk = Vec::with_capacity(self.spec.channels as usize);
        for channel_rx in &self.channels {
            chunk.push(channel_rx.recv()?);
        }
        Ok(Audio {
            spec: self.spec,
            data: chunk,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_duration() {
        let audio = generate_audio(0.0, 10, 2, 2);
        assert_almost_eq(audio.duration().as_secs_f32(), 5.0);
    }

    #[test]
    fn test_clip_in_place_both_args_none() {
        let mut audio = generate_audio(0.0, 5, 2, 2);
        audio.clip_in_place(None, None);
        assert_eq!(audio.data.get(0).unwrap().len(), 5);
        assert_eq!(audio.data.get(1).unwrap().len(), 5);
    }

    #[test]
    fn test_clip_in_place_only_start_offset_given() {
        let mut audio = generate_audio(0.0, 5, 2, 2);
        audio.clip_in_place(Some(Duration::from_millis(500)), None);
        assert_eq!(audio.data.get(0).unwrap().len(), 4);
        assert_eq!(audio.data.get(1).unwrap().len(), 4);
    }

    #[test]
    fn test_clip_in_place_only_duration_given() {
        let mut audio = generate_audio(0.0, 5, 2, 2);
        audio.clip_in_place(None, Some(Duration::from_millis(500)));
        assert_eq!(audio.data.get(0).unwrap().len(), 1);
        assert_eq!(audio.data.get(1).unwrap().len(), 1);
    }

    #[test]
    fn test_clip_in_place_both_given() {
        let mut audio = generate_audio(0.0, 5, 2, 2);
        audio.clip_in_place(
            Some(Duration::from_millis(500)),
            Some(Duration::from_millis(1000)),
        );
        assert_eq!(audio.data.get(0).unwrap().len(), 2);
        assert_eq!(audio.data.get(1).unwrap().len(), 2);
    }

    #[test]
    fn test_amplify_in_place() {
        let mut audio = generate_audio(5.0, 2, 2, 44100);
        audio.amplify_in_place(2.0);
        assert_almost_eq_by_element(audio.data[0].clone(), vec![10.0, 10.0]);
        assert_almost_eq_by_element(audio.data[1].clone(), vec![10.0, 10.0]);
    }

    #[test]
    fn test_rotate_channels() {
        let mut audio = generate_audio(5.0, 2, 2, 44100);
        audio.data[0][0] = 6.0;
        audio.rotate_channels();
        assert_almost_eq_by_element(audio.data[0].clone(), vec![5.0, 5.0]);
        assert_almost_eq_by_element(audio.data[1].clone(), vec![6.0, 5.0]);
    }

    #[test]
    fn test_fade_in_at_sample() {
        let mut audio = generate_audio(1.0, 10, 2, 44100);
        audio.fade_in_at_sample(3, 4);
        assert_almost_eq_by_element(
            audio.data[0].clone(),
            vec![
                0.0, 0.0, 0.0, 0.0, 0.5, 0.70710677, 0.8660254, 1.0, 1.0, 1.0,
            ],
        );
        assert_almost_eq_by_element(audio.data[0].clone(), audio.data[1].clone());
    }

    #[test]
    fn test_fade_out_at_sample() {
        let mut audio = generate_audio(1.0, 10, 2, 44100);
        audio.fade_out_at_sample(3, 4);
        assert_almost_eq_by_element(
            audio.data[0].clone(),
            vec![
                1.0, 1.0, 1.0, 1.0, 0.8660254, 0.70710677, 0.5, 0.0, 0.0, 0.0,
            ],
        );
        assert_almost_eq_by_element(audio.data[0].clone(), audio.data[1].clone());
    }

    #[test]
    fn test_duration_to_sample() {
        let audio = generate_audio(1.0, 10, 2, 44100);
        assert_eq!(audio.duration_to_sample(Duration::from_secs(1)), 44100);
    }

    #[test]
    fn test_sample_to_duration() {
        let audio = generate_audio(1.0, 10, 2, 44100);
        assert_eq!(audio.sample_to_duration(44100), Duration::from_secs(1));
    }
}
