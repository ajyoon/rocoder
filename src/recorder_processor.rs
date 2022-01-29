use crate::audio::{AudioBus, AudioSpec};
use crate::signal_flow::node::{ControlMessage, Processor, ProcessorState};

use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeInputBuffer,
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
        let event_loop = Arc::new(host.event_loop());
        let input_device = host
            .default_input_device()
            .expect("failed to get default input device");
        info!(
            "Using default input device: \"{}\"",
            input_device.name().unwrap()
        );

        let format = Format {
            channels: self.spec.channels,
            sample_rate: SampleRate(self.spec.sample_rate),
            data_type: SampleFormat::F32,
        };
        let input_stream_id = event_loop
            .build_input_stream(&input_device, &format)
            .unwrap();
        event_loop.play_stream(input_stream_id.clone()).unwrap();
        let event_loop_arc_for_run = Arc::clone(&event_loop);
        self.launch_cpal_thread(event_loop_arc_for_run);
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
        event_loop.destroy_stream(input_stream_id);
        Ok(())
    }

    fn launch_cpal_thread<E>(&self, event_loop: Arc<E>)
    where
        E: EventLoopTrait + Send + Sync + 'static,
    {
        let n_channels = self.spec.channels;
        let senders = self.channel_senders.clone();
        thread::spawn(move || {
            event_loop.run(move |_stream_id, stream_data| {
                let buffer = match stream_data {
                    Ok(res) => match res {
                        StreamData::Input {
                            buffer: UnknownTypeInputBuffer::F32(buffer),
                        } => buffer,
                        _ => panic!("unexpected buffer type"),
                    },
                    Err(e) => {
                        panic!("failed to fetch get audio stream: {:?}", e);
                    }
                };
                // optimisation opportunity here by creating inner vecs with capacities
                let mut channels: Vec<Vec<f32>> = (0..n_channels).map(|_| vec![]).collect();
                for buffer_interleaved_samples in buffer.chunks(n_channels as usize) {
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
                        senders.get_unchecked(i).send(channel).unwrap();
                    }
                }
            });
        });
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
