use crate::audio::{Audio, AudioBus, AudioSpec};
use crate::math;
use crate::slices;
use anyhow::{bail, Result};
use std::cmp::{Ord, Ordering};
use std::collections::HashMap;
use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;
use std::time::{Duration, Instant};

const STATUS_REPORT_INTERVAL: Duration = Duration::from_secs(1);

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

struct Layer {
    bus: AudioBus,
    amp_keyframes: Vec<Keyframe>,
    total_samples_played: usize,
    buffer: Audio<f32>,
    buffer_pos: usize,
    shutdown_when_finished: bool,
    last_status_report_instant: Instant,
}

impl Layer {
    fn new(bus: AudioBus, shutdown_when_finished: bool) -> Self {
        Layer {
            buffer: Audio::from_spec(&bus.spec),
            bus,
            shutdown_when_finished,
            amp_keyframes: vec![],
            total_samples_played: 0,
            buffer_pos: 0,
            last_status_report_instant: Instant::now(),
        }
    }

    fn load_next_chunk(&mut self) -> Result<()> {
        self.prune_keyframes();
        self.log_status();
        let mut chunk = self.bus.collect_chunk()?;
        for index in 0..chunk.data[0].len() {
            let amp = self.current_amp();
            for channel in chunk.data.iter_mut() {
                channel[index] *= amp;
            }
            self.total_samples_played += 1;
        }
        self.buffer = chunk;
        self.buffer_pos = 0;
        Ok(())
    }

    fn log_status(&mut self) {
        if self.last_status_report_instant.elapsed() < STATUS_REPORT_INTERVAL {
            return;
        }
        let played_dur = Duration::from_secs_f32(
            self.total_samples_played as f32 / self.bus.spec.sample_rate as f32,
        );
        match self.bus.expected_total_samples {
            Some(expected_total_samples) => {
                let total_dur = Duration::from_secs_f32(
                    expected_total_samples as f32 / self.bus.spec.sample_rate as f32,
                );
                let percent_played = (played_dur.div_duration_f32(total_dur) * 100.0) as u16;
                // TODO make this include how long left in minutes & seconds
                info!(
                    "Played {}s of {}s ~ {}%",
                    played_dur.as_secs(),
                    total_dur.as_secs(),
                    percent_played
                );
            }
            None => {
                info!("Played {}s", played_dur.as_secs());
            }
        }
        self.last_status_report_instant = Instant::now();
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

    fn clear_keyframes_after(&mut self, sample_pos: usize) {
        self.amp_keyframes
            .drain_filter(|k| k.sample_pos > sample_pos);
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

    #[inline]
    pub fn dur_to_sample(&self, dur: Duration) -> usize {
        (dur.as_secs_f32() * self.bus.spec.sample_rate as f32) as usize
    }

    pub fn fade_from_now(&mut self, to: f32, dur: Duration) {
        // assumes that no keyframes exist in the modified window
        let current_amp = self.current_amp();
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.total_samples_played,
            val: current_amp,
        });
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.total_samples_played + self.dur_to_sample(dur),
            val: to,
        });
        self.sort_keyframes();
    }

    pub fn fade(&mut self, start: Duration, start_val: f32, dur: Duration, end_val: f32) {
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.dur_to_sample(start),
            val: start_val,
        });
        self.amp_keyframes.push(Keyframe {
            sample_pos: self.dur_to_sample(start + dur),
            val: end_val,
        });
        self.sort_keyframes();
    }

    /// only fades out if both `fade_out_dur` and `self.bus.expected_total_samples` are present
    pub fn fade_in_out(&mut self, fade_in_dur: Option<Duration>, fade_out_dur: Option<Duration>) {
        if fade_in_dur.is_some() {
            self.fade(Duration::from_secs(0), 0.0, fade_in_dur.unwrap(), 1.0);
        }
        if fade_out_dur.is_some() && self.bus.expected_total_samples.is_some() {
            let total_dur = Duration::from_secs_f32(
                self.bus.expected_total_samples.unwrap() as f32 / self.bus.spec.sample_rate as f32,
            );
            let fade_start = total_dur - fade_out_dur.unwrap();
            self.fade(fade_start, 1.0, fade_out_dur.unwrap(), 0.0);
        }
    }
}

pub struct Mixer {
    pub spec: AudioSpec,
    pub finished_flag: Arc<AtomicBool>,
    layers: HashMap<u32, Layer>,
}

impl Mixer {
    pub fn new(spec: &AudioSpec) -> Self {
        Mixer {
            finished_flag: Arc::new(AtomicBool::from(false)),
            spec: *spec,
            layers: HashMap::new(),
        }
    }

    pub fn fill_buffer(&mut self, out_buf: &mut [f32]) {
        slices::zero_slice(out_buf);
        for buffer_interleaved_samples in out_buf.chunks_mut(self.spec.channels as usize) {
            // loop body covers 1 sample across all layers & channels
            let mut closed_layer_ids: Vec<u32> = Vec::with_capacity(0);
            for (layer_id, layer) in self.layers.iter_mut() {
                if layer.buffer_pos >= layer.buffer.data[0].len() {
                    // sets layer.buffer_pos = 0
                    if layer.load_next_chunk().is_err() {
                        if layer.shutdown_when_finished {
                            info!("Layer finished and requested mixer shutdown; setting flag.");
                            self.finished_flag.store(true, atomic::Ordering::Relaxed);
                        }
                        closed_layer_ids.push(*layer_id);
                        continue;
                    };
                }
                for (channel_idx, out_sample_channel) in
                    buffer_interleaved_samples.iter_mut().enumerate()
                {
                    *out_sample_channel += layer.buffer.data[channel_idx][layer.buffer_pos];
                }
                layer.buffer_pos += 1;
            }
            if !closed_layer_ids.is_empty() {
                for layer_id in closed_layer_ids.into_iter() {
                    self.layers.remove(&layer_id);
                }
            }
        }
    }

    pub fn insert_layer(
        &mut self,
        id: u32,
        bus: AudioBus,
        shutdown_when_finished: bool,
    ) -> Result<()> {
        let layer = Layer::new(bus, shutdown_when_finished);
        self.layers.insert(id, layer);
        Ok(())
    }

    pub fn fade_out_all_layers(&mut self, dur: Duration) {
        for layer in self.layers.values_mut() {
            layer.fade_from_now(0.0, dur);
            layer.clear_keyframes_after(layer.total_samples_played + layer.dur_to_sample(dur));
        }
    }

    pub fn fade_from_now(&mut self, id: u32, to: f32, dur: Duration) -> Result<()> {
        match self.layers.get_mut(&id) {
            Some(layer) => Ok(layer.fade_from_now(to, dur)),
            None => bail!("Layer not found"),
        }
    }

    pub fn fade(
        &mut self,
        id: u32,
        start: Duration,
        start_val: f32,
        dur: Duration,
        end_val: f32,
    ) -> Result<()> {
        match self.layers.get_mut(&id) {
            Some(layer) => Ok(layer.fade(start, start_val, dur, end_val)),
            None => bail!("Layer not found"),
        }
    }

    /// only fades out if `fade_out_dur` is present and the layer in question has an expected duration
    pub fn fade_in_out(
        &mut self,
        id: u32,
        fade_in_dur: Option<Duration>,
        fade_out_dur: Option<Duration>,
    ) -> Result<()> {
        match self.layers.get_mut(&id) {
            Some(layer) => Ok(layer.fade_in_out(fade_in_dur, fade_out_dur)),
            None => bail!("Layer not found"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn prune_keyframes() {
        // setup...
        let mut layer = basic_layer();
        assert!(layer.amp_keyframes.is_empty());
        layer.amp_keyframes = vec![
            basic_keyframe(4000),
            basic_keyframe(1500),
            basic_keyframe(1000),
        ];

        // Prune while playback pos is between or before the earliest 2
        // keyframes is a no-op
        layer.total_samples_played = 900;
        layer.prune_keyframes();
        assert_eq!(layer.amp_keyframes.len(), 3);
        layer.total_samples_played = 1200;
        layer.prune_keyframes();
        assert_eq!(layer.amp_keyframes.len(), 3);

        // Prune while playback pos is after earliest 2 keyframes will delete
        // the earliest keyframe
        layer.total_samples_played = 2000;
        layer.prune_keyframes();
        assert_eq!(layer.amp_keyframes.len(), 2);
        assert_eq!(layer.amp_keyframes[0].sample_pos, 4000);
        assert_eq!(layer.amp_keyframes[1].sample_pos, 1500);

        // Prune while playback pos is after all keyframes will leave just the last
        layer.total_samples_played = 5000;
        layer.prune_keyframes();
        assert_eq!(layer.amp_keyframes.len(), 1);
        assert_eq!(layer.amp_keyframes[0].sample_pos, 4000);
    }

    #[test]
    fn clear_keyframes_after_with_no_keyframes() {
        let mut layer = basic_layer();
        layer.clear_keyframes_after(0);
        assert!(layer.amp_keyframes.is_empty());
    }

    #[test]
    fn clear_keyframes_after_with_keyframes_before_and_after() {
        let mut layer = basic_layer();
        layer.fade(Duration::from_secs(0), 0.5, Duration::from_secs(2), 1.0);
        layer.fade(Duration::from_secs(5), 0.3, Duration::from_secs(6), 0.9);
        assert_eq!(layer.amp_keyframes.len(), 4);
        layer.clear_keyframes_after(layer.dur_to_sample(Duration::from_secs(4)));
        assert_eq!(layer.amp_keyframes.len(), 2);
        // remember - amp_keyframes is kept in reverse order
        assert_almost_eq(layer.amp_keyframes[0].val, 1.0);
        assert_almost_eq(layer.amp_keyframes[1].val, 0.5);
    }

    fn basic_layer() -> Layer {
        let (_, rx) = unbounded();
        let spec = AudioSpec {
            channels: 2,
            sample_rate: 44100,
        };
        let bus = AudioBus {
            spec,
            channels: vec![rx],
            expected_total_samples: None,
        };
        Layer::new(bus, false)
    }

    fn basic_keyframe(sample_pos: usize) -> Keyframe {
        Keyframe {
            sample_pos,
            val: 1.0,
        }
    }
}
