use crate::audio::{AudioBus, AudioSpec};
use crate::cpal_utils;
use crate::mixer::Mixer;
use crate::signal_flow::node::{ControlMessage, Processor, ProcessorState};
use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const PLAYBACK_SLEEP: Duration = Duration::from_millis(250);

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
    shutdown_after: Option<Instant>,
}

impl AudioOutputProcessor {
    pub fn new(spec: AudioSpec) -> Self {
        AudioOutputProcessor {
            mixer: Arc::new(Mutex::new(Mixer::new(&spec))),
            shutdown_after: None,
            spec,
        }
    }

    fn run(mut self, ctrl_rx: Receiver<AudioOutputProcessorControlMessage>) -> Result<()> {
        let mixer_arc = Arc::clone(&self.mixer);
        let host = cpal::default_host();
        let output_device = host.default_output_device().unwrap();
        info!("Using default output device: \"{}\"", output_device.name()?);
        let supported_configs = output_device
            .supported_output_configs()
            .expect("failed to query output device configs");
        let stream_config = cpal_utils::find_output_stream_config(
            supported_configs,
            self.spec.channels,
            self.spec.sample_rate,
        )?;
        let output_stream = output_device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // react to stream events and read or write stream data here.
                    let mut mixer = mixer_arc.lock().unwrap();
                    mixer.fill_buffer(data);
                },
                move |err| {
                    panic!("audio output stream failed: {:?}", err);
                },
            )
            .expect("failed to build output stream");
        output_stream.play().expect("failed to start output stream");

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
                .load(Ordering::Relaxed)
            {
                break;
            }
            if let Some(shutdown_after) = self.shutdown_after {
                if Instant::now() > shutdown_after {
                    break;
                }
            }
            thread::sleep(PLAYBACK_SLEEP);
        }
        Ok(())
    }

    const FADE_SHUTDOWN_PADDING: Duration = Duration::from_secs(1);

    fn fade_shutdown(&mut self, fade_dur: Duration) {
        self.shutdown_after = Some(Instant::now() + fade_dur + Self::FADE_SHUTDOWN_PADDING);
        self.mixer.lock().unwrap().fade_out_all_layers(fade_dur);
    }
}

impl Processor<AudioOutputProcessorControlMessage> for AudioOutputProcessor {
    fn start(
        self,
        finished: Arc<AtomicBool>,
    ) -> (Sender<AudioOutputProcessorControlMessage>, JoinHandle<()>) {
        let (ctrl_tx, ctrl_rx) = unbounded();
        let handle = thread::spawn(move || {
            self.run(ctrl_rx).unwrap();
            finished.store(true, Ordering::Relaxed);
        });
        (ctrl_tx, handle)
    }

    fn handle_control_messages(
        &mut self,
        rx: &Receiver<AudioOutputProcessorControlMessage>,
    ) -> Result<ProcessorState> {
        match rx.try_recv() {
            Ok(msg) => match msg {
                AudioOutputProcessorControlMessage::Shutdown { fade } => Ok(match fade {
                    Some(fade_dur) => {
                        self.fade_shutdown(fade_dur);
                        ProcessorState::Running
                    }
                    None => ProcessorState::Finished,
                }),
                AudioOutputProcessorControlMessage::ConnectBus {
                    id,
                    bus,
                    fade,
                    shutdown_when_finished,
                } => {
                    let mut mixer = self.mixer.lock().unwrap();
                    mixer.insert_layer(id, bus, shutdown_when_finished)?;
                    mixer.fade_in_out(id, fade.clone(), fade)?;
                    Ok(ProcessorState::Running)
                }
            },
            Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
            Err(TryRecvError::Empty) => Ok(ProcessorState::Running),
        }
    }
}
