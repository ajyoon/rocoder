use anyhow::Result;
use rocoder::audio::AudioSpec;
use rocoder::installation_processor::{
    InstallationProcessor, InstallationProcessorConfig, InstallationProcessorControlMessage,
};
use rocoder::runtime_setup;
use rocoder::signal_flow::node::Node;

#[macro_use]
extern crate log;

fn main() -> Result<()> {
    runtime_setup::setup_logging();

    let processor = InstallationProcessor::new(InstallationProcessorConfig {
        max_stretchers: 30,
        window_sizes: vec![2048, 4096, 8192, 16384],
        min_stretch_factor: 2.0,
        max_stretch_factor: 20.0,
        ..Default::default()
    });
    let root_node = Node::new(processor);
    root_node.join();
    Ok(())
}
