use anyhow::Result;
use rocoder::installation_processor::{
    InstallationProcessor, InstallationProcessorConfig, InstallationProcessorControlMessage,
};
use rocoder::runtime_setup;
use rocoder::signal_flow::node::Node;

#[macro_use]
extern crate log;

fn main() -> Result<()> {
    runtime_setup::setup_logging();

    let processor = InstallationProcessor::new(InstallationProcessorConfig::default());
    let root_node = Node::new(processor);
    root_node.join();
    Ok(())
}
