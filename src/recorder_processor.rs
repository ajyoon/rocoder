use crate::audio::{AudioBus, AudioSpec};
use crate::cpal_utils;
use crate::signal_flow::node::{ControlMessage, Processor, ProcessorState};

use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const RECORDER_POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum RecorderProcessorControlMessage {
    Shutdown,
}

impl ControlMessage for RecorderProcessorControlMessage {
    fn shutdown_msg() -> Self {
        RecorderProcessorControlMessage::Shutdown
    }
}

pub struct RecorderProcessor {
    spec: AudioSpec,
    finished: Arc<AtomicBool>,
    channel_senders: Vec<Sender<Vec<f32>>>,
}

impl RecorderProcessor {
    pub fn new(spec: AudioSpec) -> (RecorderProcessor, AudioBus) {
        let (bus, channel_senders) = AudioBus::from_spec(spec, None);
        (
            RecorderProcessor {
                spec,
                channel_senders,
                finished: Arc::new(AtomicBool::new(false)),
            },
            bus,
        )
    }

    fn run(mut self, ctrl_rx: Receiver<RecorderProcessorControlMessage>) -> Result<()> {
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .expect("failed to get default input device");
        info!(
            "Using default input device: \"{}\"",
            input_device.name().unwrap()
        );

        let supported_configs = input_device
            .supported_input_configs()
            .expect("failed to query input device configs");
        let stream_config = cpal_utils::find_input_stream_config(
            supported_configs,
            self.spec.channels,
            self.spec.sample_rate,
        )?;

        let channel_senders = self.channel_senders.clone();

        let input_stream = input_device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // react to stream events and read or write stream data here.
                    send_samples_from_raw_input(data, self.spec.channels, &channel_senders)
                },
                move |err| {
                    panic!("audio input stream failed: {:?}", err);
                },
                None,
            )
            .expect("failed to build input stream");
        input_stream.play().expect("failed to start input stream");
        loop {
            if self.finished.load(Ordering::Relaxed) {
                break;
            }
            match self.handle_control_messages(&ctrl_rx)? {
                ProcessorState::Finished => {
                    break;
                }
                _ => {}
            }
            thread::sleep(RECORDER_POLL);
        }
        Ok(())
    }
}

fn send_samples_from_raw_input(
    buf: &[f32],
    n_channels: u16,
    channel_senders: &Vec<Sender<Vec<f32>>>,
) {
    // optimisation opportunity here by creating inner vecs with capacities
    let mut channels: Vec<Vec<f32>> = (0..n_channels).map(|_| vec![]).collect();
    for buffer_interleaved_samples in buf.chunks(n_channels as usize) {
        for i in 0..n_channels as usize {
            unsafe {
                channels
                    .get_unchecked_mut(i)
                    .push(*buffer_interleaved_samples.get_unchecked(i));
            }
        }
    }
    for (i, channel) in channels.into_iter().enumerate() {
        unsafe {
            channel_senders.get_unchecked(i).send(channel).unwrap();
        }
    }
}

impl Processor<RecorderProcessorControlMessage> for RecorderProcessor {
    fn handle_control_messages(
        &mut self,
        rx: &Receiver<RecorderProcessorControlMessage>,
    ) -> Result<ProcessorState> {
        match rx.try_recv() {
            Ok(msg) => match msg {
                RecorderProcessorControlMessage::Shutdown => {
                    self.finished.store(true, Ordering::Relaxed);
                    Ok(ProcessorState::Finished)
                }
            },
            Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
            Err(TryRecvError::Empty) => Ok(ProcessorState::Running),
        }
    }

    fn start(
        self,
        finished: Arc<AtomicBool>,
    ) -> (Sender<RecorderProcessorControlMessage>, JoinHandle<()>) {
        let (ctrl_tx, ctrl_rx) = unbounded();
        let handle = thread::spawn(move || {
            self.run(ctrl_rx).unwrap();
            finished.store(true, Ordering::Relaxed);
        });
        (ctrl_tx, handle)
    }
}
