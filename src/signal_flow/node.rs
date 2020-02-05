use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

pub trait ControlMessage: Send + Sync + Debug + 'static {
    fn shutdown_msg() -> Self;
}

pub struct Node<P, M>
where
    P: Processor<M>,
    M: ControlMessage,
{
    control_message_sender: Sender<M>,
    join_handle: JoinHandle<()>,
    phantom: PhantomData<P>,
}

impl<P, M> Node<P, M>
where
    P: Processor<M>,
    M: ControlMessage,
{
    pub fn new(processor: P) -> Node<P, M> {
        let (control_message_sender, control_message_receiver) = unbounded::<M>();
        let join_handle = processor.start(control_message_receiver);
        Node {
            control_message_sender,
            join_handle,
            phantom: PhantomData,
        }
    }

    pub fn send_control_message(&self, message: M) -> Result<()> {
        self.control_message_sender.send(message)?;
        Ok(())
    }

    pub fn shutdown(self) -> Result<JoinHandle<()>> {
        self.send_control_message(M::shutdown_msg())?;
        Ok(self.join_handle)
    }
}

pub enum ProcessorState {
    Running,
    Finished,
}

pub trait Processor<M>: Sized + Send + 'static
where
    M: ControlMessage,
{
    fn start(self, rx: Receiver<M>) -> JoinHandle<()>;

    /// Handle control messages, if any are ready.
    ///
    /// When receiving messages, be sure to use `rx.try_recv()` to ensure
    /// this method does not block when no control messages are available.
    ///
    /// If a shutdown message is received, return `Ok(ProcessorState::Finished)`.
    /// Otherwise return `Ok(ProcessorState::Running)`. If fatal unexpected errors
    /// occur, return the error.
    fn handle_control_messages(&mut self, rx: &Receiver<M>) -> Result<ProcessorState>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn node_start_shutdown_and_join() {
        let node = Node::new(TestProcessor {});
        let handle = node.shutdown().unwrap();
        handle.join().unwrap();
    }

    #[derive(Debug)]
    enum TestControlMessage {
        Shutdown,
    }

    impl ControlMessage for TestControlMessage {
        fn shutdown_msg() -> Self {
            TestControlMessage::Shutdown
        }
    }

    struct TestProcessor {}

    impl Processor<TestControlMessage> for TestProcessor {
        fn start(mut self, rx: Receiver<TestControlMessage>) -> JoinHandle<()> {
            thread::spawn(move || loop {
                let state = self.handle_control_messages(&rx).unwrap_or_else(|e| {
                    error!("{:?}", e);
                    ProcessorState::Running
                });
                if let ProcessorState::Finished = state {
                    break;
                }
                thread::sleep(Duration::from_millis(10))
            })
        }

        fn handle_control_messages(
            &mut self,
            rx: &Receiver<TestControlMessage>,
        ) -> Result<ProcessorState> {
            match rx.try_recv() {
                Ok(msg) => match msg {
                    TestControlMessage::Shutdown => Ok(ProcessorState::Finished),
                },
                Err(TryRecvError::Disconnected) => Ok(ProcessorState::Finished),
                _ => Ok(ProcessorState::Finished),
            }
        }
    }
}
