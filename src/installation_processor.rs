use crate::audio::{Audio, AudioBus, AudioSpec};
use crate::player_processor::{AudioOutputProcessor, AudioOutputProcessorControlMessage};
use crate::recorder_processor::{RecorderProcessor, RecorderProcessorControlMessage};
use crate::signal_flow::node::{ControlMessage, Node, Processor, ProcessorState};
use crate::stretcher::Stretcher;
use crate::stretcher_processor::{StretcherProcessor, StretcherProcessorControlMessage};
use crate::windows;

use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    SampleFormat, SampleRate, StreamData, UnknownTypeInputBuffer,
};
use crossbeam_channel::{unbounded, Receiver, RecvError, Sender, TryRecvError};
use rand::{self, Rng};
use slice_deque::SliceDeque;
// use slice_ring_buf::{SliceRB, SliceRbRef};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Debug)]
pub enum InstallationProcessorControlMessage {
    Shutdown,
}

impl ControlMessage for InstallationProcessorControlMessage {
    fn shutdown_msg() -> Self {
        InstallationProcessorControlMessage::Shutdown
    }
}

pub struct InstallationProcessorConfig {
    spec: AudioSpec,
    max_stretchers: u8,
    max_snippet_dur: Duration,
    ambient_volume_window_dur: Duration,
    current_volume_window_dur: Duration,
    amp_activation_factor: f32,
}

impl Default for InstallationProcessorConfig {
    fn default() -> Self {
        InstallationProcessorConfig {
            spec: AudioSpec {
                channels: 2,
                sample_rate: 44100,
            },
            max_stretchers: 4,
            max_snippet_dur: Duration::from_secs(1),
            ambient_volume_window_dur: Duration::from_secs(10),
            current_volume_window_dur: Duration::from_millis(300),
            amp_activation_factor: 1.5,
        }
    }
}

pub struct InstallationProcessor {
    config: InstallationProcessorConfig,
}

#[derive(Debug, Copy, Clone)]
enum ListeningState {
    Idle,
    Active,
}

impl InstallationProcessor {
    pub fn new(config: InstallationProcessorConfig) -> Self {
        InstallationProcessor { config }
    }

    fn run(mut self, ctrl_rx: Receiver<InstallationProcessorControlMessage>) -> Result<()> {
        let spec = self.config.spec;
        let (recorder_processor, recorder_bus) = RecorderProcessor::new(spec);
        let recorder = Node::new(recorder_processor);
        let player = Node::new(AudioOutputProcessor::new(spec));

        let mut stretcher_nodes = vec![];

        const rec_buf_chunks: usize = 512;
        let ambient_amp_window_size = (self.config.ambient_volume_window_dur.as_secs_f32()
            * spec.sample_rate as f32) as usize
            * spec.channels as usize;
        let current_amp_window_size = (self.config.current_volume_window_dur.as_secs_f32()
            * spec.sample_rate as f32) as usize
            * spec.channels as usize;
        let mut ambient_amplitude: f32 = 0.0;
        let mut current_amplitude: f32 = 0.0;
        // TODO this slicedeque is actually an auto-resizing
        // ringbuffer, so it will grow indefinitely when used like
        // this. need to replace with an actual fixed-size ring buffer
        // here, and elsewhere
        let mut recording_buffers: Vec<SliceDeque<Vec<f32>>> = (0..recorder_bus.channels.len())
            .map(|_| SliceDeque::with_capacity(rec_buf_chunks))
            .collect();
        let mut listening_state = ListeningState::Idle;
        let mut recording_buffer_listen_start = 0;

        loop {
            // Fetch latest data from recorder
            recorder_bus.channels.iter().enumerate().for_each(
                |(i, channel_recv)| match channel_recv.recv() {
                    Ok(chunk) => {
                        unsafe { recording_buffers.get_unchecked_mut(i) }.push_back(chunk);
                    }
                    Err(RecvError) => panic!("recorder unexpectedly crashed"),
                },
            );

            // Adjust the moving average amplitudes for ambient and current levels
            // new average = old average * (n-len(M))/n + (sum of values in M)/n).
            ambient_amplitude = Self::chunked_moving_average_amp(
                ambient_amplitude,
                ambient_amp_window_size,
                &recording_buffers,
            );
            current_amplitude = Self::chunked_moving_average_amp(
                current_amplitude,
                current_amp_window_size,
                &recording_buffers,
            );

            // todo this thresholding currently takes a flawed naive linear approach,
            // to work well it probably needs to be made exponential
            match listening_state {
                ListeningState::Idle => {
                    if recording_buffers[0].len() > 512
                        && current_amplitude > ambient_amplitude * self.config.amp_activation_factor
                    {
                        info!(
                            "Heard something, starting to listen. amp={}, ambient amp={}",
                            current_amplitude, ambient_amplitude
                        );
                        listening_state = ListeningState::Active;
                        recording_buffer_listen_start = recording_buffers[0].len();
                    }
                }
                ListeningState::Active => {
                    if current_amplitude < ambient_amplitude / self.config.amp_activation_factor {
                        info!(
                            "Event ended, playing back. amp={}, ambient amp={}",
                            current_amplitude, ambient_amplitude
                        );
                        listening_state = ListeningState::Idle;
                        let stretchers: Vec<Stretcher> = recording_buffers
                            .iter()
                            .map(|b| {
                                let input_chunks = &b[recording_buffer_listen_start..];
                                let (tx, rx) = unbounded();
                                input_chunks.iter().for_each(|chunk| {
                                    tx.send(chunk.clone()).unwrap();
                                });
                                return Stretcher::new(
                                    spec,
                                    rx,
                                    10.0,
                                    1.0,
                                    1,
                                    windows::hanning(4096),
                                    Duration::from_secs(4),
                                    None,
                                );
                            })
                            .collect();
                        let (processor, bus) = StretcherProcessor::new(stretchers, None);
                        stretcher_nodes.push(Node::new(processor));
                        player.send_control_message(
                            AudioOutputProcessorControlMessage::ConnectBus {
                                id: rand::thread_rng().gen(),
                                bus: bus,
                                fade: Some(Duration::from_millis(500)),
                                shutdown_when_finished: false,
                            },
                        );
                    }
                }
            }

            match self.handle_control_messages(&ctrl_rx)? {
                ProcessorState::Finished => {
                    break;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn chunked_moving_average_amp(
        last_avg: f32,
        window_size: usize,
        recording_buffers: &Vec<SliceDeque<Vec<f32>>>,
    ) -> f32 {
        let last_chunk_len = recording_buffers[0].back().unwrap().len() * recording_buffers.len();
        (last_avg * ((window_size - last_chunk_len) as f32 / window_size as f32))
            + (recording_buffers
                .iter()
                .map(|chunks| {
                    chunks
                        .back()
                        .unwrap()
                        .iter()
                        .map(|sample| sample.abs())
                        .sum::<f32>()
                })
                .sum::<f32>() as f32
                / window_size as f32)
    }
}

impl Processor<InstallationProcessorControlMessage> for InstallationProcessor {
    fn handle_control_messages(
        &mut self,
        rx: &Receiver<InstallationProcessorControlMessage>,
    ) -> Result<ProcessorState> {
        match rx.try_recv() {
            Ok(msg) => match msg {
                InstallationProcessorControlMessage::Shutdown => Ok(ProcessorState::Finished),
            },
            Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
            Err(TryRecvError::Empty) => Ok(ProcessorState::Running),
        }
    }

    fn start(
        self,
        finished: Arc<AtomicBool>,
    ) -> (Sender<InstallationProcessorControlMessage>, JoinHandle<()>) {
        let (ctrl_tx, ctrl_rx) = unbounded();
        let handle = thread::spawn(move || {
            self.run(ctrl_rx).unwrap();
            finished.store(true, Ordering::Relaxed);
        });
        (ctrl_tx, handle)
    }
}
