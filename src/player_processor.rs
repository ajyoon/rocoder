use crate::audio::{AudioBus, AudioSpec};
use crate::mixer::Mixer;
use crate::signal_flow::node::{ControlMessage, Node, Processor, ProcessorState};
use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeOutputBuffer,
};
use crossbeam_channel::{Receiver, TryRecvError};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const PLAYBACK_SLEEP: Duration = Duration::from_millis(250);
const QUIT_FADE: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum AudioOutputProcessorControlMessage {
    Shutdown {
        fade: Option<Duration>,
    },
    ConnectBus {
        id: u32,
        bus: AudioBus,
        fade: Option<Duration>,
        shutdown_when_finished: bool,
    },
}

impl ControlMessage for AudioOutputProcessorControlMessage {
    fn shutdown_msg() -> Self {
        AudioOutputProcessorControlMessage::Shutdown {
            fade: Some(Duration::from_secs(1)),
        }
    }
}

pub struct AudioOutputProcessor {
    spec: AudioSpec,
    mixer: Arc<Mutex<Mixer>>,
    expected_total_samples: Option<usize>,
}

impl AudioOutputProcessor {
    pub fn new(spec: AudioSpec, expected_total_samples: Option<usize>) -> Self {
        AudioOutputProcessor {
            mixer: Arc::new(Mutex::new(Mixer::new(&spec))),
            expected_total_samples,
            spec,
        }
    }

    fn launch_cpal_thread<E>(event_loop: Arc<E>, mixer_arc: Arc<Mutex<Mixer>>)
    where
        E: EventLoopTrait + Send + Sync + 'static,
    {
        thread::spawn(move || {
            event_loop.run(move |_stream_id, stream_data| {
                let mut buffer = match stream_data {
                    Ok(res) => match res {
                        StreamData::Output {
                            buffer: UnknownTypeOutputBuffer::F32(buffer),
                        } => buffer,
                        _ => panic!("unexpected buffer type"),
                    },
                    Err(e) => {
                        panic!("failed to fetch get audio stream: {:?}", e);
                    }
                };
                let mut mixer = mixer_arc.lock().unwrap();
                mixer.fill_buffer(&mut buffer);
            });
        });
    }

    fn run(mut self, ctrl_rx: Receiver<AudioOutputProcessorControlMessage>) -> Result<()> {
        let mixer_arc_for_run = Arc::clone(&self.mixer);
        let format = Format {
            channels: self.spec.channels,
            sample_rate: SampleRate(self.spec.sample_rate),
            data_type: SampleFormat::F32,
        };
        let host = cpal::default_host();
        let event_loop = Arc::new(host.event_loop());
        let event_loop_arc_for_run = Arc::clone(&event_loop);
        let output_device = host.default_output_device().unwrap();
        info!("Using default output device: \"{}\"", output_device.name()?);
        let output_stream_id = event_loop.build_output_stream(&output_device, &format)?;
        event_loop.play_stream(output_stream_id.clone())?;
        Self::launch_cpal_thread(event_loop_arc_for_run, mixer_arc_for_run);
        loop {
            match self.handle_control_messages(&ctrl_rx)? {
                ProcessorState::Finished => {
                    break;
                }
                _ => {}
            }
            if self
                .mixer
                .lock()
                .unwrap()
                .finished_flag
                .load(Ordering::SeqCst)
            {
                break;
            }
            thread::sleep(PLAYBACK_SLEEP);
        }
        event_loop.destroy_stream(output_stream_id);
        Ok(())
    }
}

impl Processor<AudioOutputProcessorControlMessage> for AudioOutputProcessor {
    fn start(self, ctrl_rx: Receiver<AudioOutputProcessorControlMessage>) -> JoinHandle<()> {
        thread::spawn(move || self.run(ctrl_rx).unwrap())
    }

    fn handle_control_messages(
        &mut self,
        rx: &Receiver<AudioOutputProcessorControlMessage>,
    ) -> Result<ProcessorState> {
        match rx.try_recv() {
            Ok(msg) => match msg {
                AudioOutputProcessorControlMessage::Shutdown { fade: _ } => {
                    Ok(ProcessorState::Finished)
                }
                AudioOutputProcessorControlMessage::ConnectBus {
                    id: id,
                    bus: bus,
                    fade: fade,
                    shutdown_when_finished: shutdown_when_finished,
                } => {
                    let mut mixer = self.mixer.lock().unwrap();
                    mixer.insert_layer(id, bus, shutdown_when_finished);
                    mixer.fade_in_out(id, fade.clone(), fade);
                    Ok(ProcessorState::Running)
                }
                _ => todo!(),
            },
            Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
            Err(TryRecvError::Empty) => Ok(ProcessorState::Running),
        }
    }
}
