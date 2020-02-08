use crate::audio::{AudioBus, AudioSpec};
use crate::mixer::Mixer;
use crate::signal_flow::node::{ControlMessage, Processor, ProcessorState};
use anyhow::Result;
use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeOutputBuffer,
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
        event_loop.destroy_stream(output_stream_id);
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
