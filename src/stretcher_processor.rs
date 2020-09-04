use crate::audio::AudioBus;
use crate::signal_flow::node::{ControlMessage, Processor, ProcessorState};
use crate::stretcher::Stretcher;
use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender, TryRecvError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

#[derive(Debug)]
pub enum StretcherProcessorControlMessage {
    Shutdown,
}

impl ControlMessage for StretcherProcessorControlMessage {
    fn shutdown_msg() -> Self {
        StretcherProcessorControlMessage::Shutdown
    }
}

pub struct StretcherProcessor {
    channels: Vec<(Sender<Vec<f32>>, Stretcher)>,
}

impl StretcherProcessor {
    pub fn new(
        channel_stretchers: Vec<Stretcher>,
        expected_total_samples: Option<usize>,
    ) -> (StretcherProcessor, AudioBus) {
        let spec = channel_stretchers[0].spec;
        let mut channels: Vec<(Sender<Vec<f32>>, Stretcher)> = vec![];
        let mut receivers: Vec<Receiver<Vec<f32>>> = vec![];
        for stretcher in channel_stretchers.into_iter() {
            let (tx, rx) = bounded(stretcher.channel_bound());
            channels.push((tx, stretcher));
            receivers.push(rx);
        }
        (
            StretcherProcessor { channels },
            AudioBus {
                spec,
                channels: receivers,
                expected_total_samples,
            },
        )
    }
}

impl Processor<StretcherProcessorControlMessage> for StretcherProcessor {
    fn start(
        mut self,
        finished: Arc<AtomicBool>,
    ) -> (Sender<StretcherProcessorControlMessage>, JoinHandle<()>) {
        let (ctrl_tx, ctrl_rx) = unbounded();
        let handle = thread::spawn(move || {
            'outer: loop {
                match self.handle_control_messages(&ctrl_rx).unwrap() {
                    ProcessorState::Finished => {
                        break 'outer;
                    }
                    _ => {}
                }
                for (output, stretcher) in self.channels.iter_mut() {
                    if stretcher.is_done() {
                        // assuming each stretcher finishes at the same time
                        info!("stretch process completed");
                        break 'outer;
                    }
                    output.send(stretcher.next_window()).unwrap();
                }
            }
            finished.store(true, Ordering::Relaxed);
        });
        (ctrl_tx, handle)
    }

    fn handle_control_messages(
        &mut self,
        rx: &Receiver<StretcherProcessorControlMessage>,
    ) -> Result<ProcessorState> {
        match rx.try_recv() {
            Ok(msg) => match msg {
                StretcherProcessorControlMessage::Shutdown => Ok(ProcessorState::Finished),
            },
            Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
            Err(TryRecvError::Empty) => Ok(ProcessorState::Running),
        }
    }
}
