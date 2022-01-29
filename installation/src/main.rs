use anyhow::Result;
use rocoder::audio::AudioSpec;
use rocoder::duration_parser;
use rocoder::runtime_setup;
use rocoder::signal_flow::node::Node;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

use installation::installation_processor::{
    InstallationProcessor, InstallationProcessorConfig, InstallationProcessorControlMessage,
};

#[macro_use]
extern crate log;

#[derive(Debug, StructOpt)]
#[structopt(name = "installation", setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    #[structopt(long = "channels", default_value = "1")]
    channels: u8,

    #[structopt(long = "max-stretchers", default_value = "4")]
    max_stretchers: u8,

    #[structopt(long = "max-snippet-dur", default_value = "1",
                parse(try_from_str = duration_parser::parse_duration))]
    max_snippet_dur: Duration,

    #[structopt(long = "ambient-volume-window-dur", default_value = "10",
                parse(try_from_str = duration_parser::parse_duration))]
    ambient_volume_window_dur: Duration,

    #[structopt(long = "current-volume-window-dur", default_value = "0.3",
                parse(try_from_str = duration_parser::parse_duration))]
    current_volume_window_dur: Duration,

    #[structopt(long = "amp-activation-db-step", default_value = "2.0")]
    amp_activation_db_step: f32,

    #[structopt(long = "window-size", default_value = "8192")]
    window_size: usize,

    #[structopt(long = "min-stretch-factor", default_value = "6")]
    min_stretch_factor: f32,

    #[structopt(long = "max-stretch-factor", default_value = "12")]
    max_stretch_factor: f32,

    #[structopt(long = "min-pause-between-events", default_value = "0",
                parse(try_from_str = duration_parser::parse_duration))]
    min_pause_between_events: Duration,

    #[structopt(long = "max-pause-between-events", default_value = "15",
                parse(try_from_str = duration_parser::parse_duration))]
    max_pause_between_events: Duration,
}

impl Opt {
    fn into_config(self) -> InstallationProcessorConfig {
        return InstallationProcessorConfig {
            spec: AudioSpec {
                channels: self.channels as u16,
                sample_rate: 44100,
            },
            max_stretchers: self.max_stretchers,
            max_snippet_dur: self.max_snippet_dur,
            ambient_volume_window_dur: self.ambient_volume_window_dur,
            current_volume_window_dur: self.current_volume_window_dur,
            amp_activation_db_step: self.amp_activation_db_step,
            window_sizes: vec![self.window_size],
            min_stretch_factor: self.min_stretch_factor,
            max_stretch_factor: self.max_stretch_factor,
            min_pause_between_events: self.min_pause_between_events,
            max_pause_between_events: self.max_pause_between_events,
        };
    }
}

fn main() -> Result<()> {
    runtime_setup::setup_logging();

    let processor = InstallationProcessor::new(Opt::from_args().into_config());
    let root_node = Node::new(processor);
    root_node.join();
    Ok(())
}
