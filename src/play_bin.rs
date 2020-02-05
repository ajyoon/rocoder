use rocoder::audio::{Audio, AudioBus, AudioSpec};
use rocoder::audio_files::{AudioReader, WavReader};
use rocoder::duration_parser;
use rocoder::player_processor::{AudioOutputProcessor, AudioOutputProcessorControlMessage};
use rocoder::recorder;
use rocoder::runtime_setup;
use rocoder::signal_flow::node::Processor;

use anyhow::Result;
use crossbeam_channel::unbounded;

use std::io;
use std::path::PathBuf;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(name = "rocoder", setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    #[structopt(short = "a", long = "amplitude", default_value = "1")]
    amplitude: f32,

    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    #[structopt(
        long = "rotate-channels",
        help = "Rotate the input audio channels. With stereo audio this means swapping the left and right channels"
    )]
    rotate_channels: bool,

    #[structopt(short = "r", long = "record", conflicts_with = "input")]
    record: bool,

    #[structopt(
        short = "s",
        long = "start",
        help = "start time in input audio (hh:mm:ss.ssss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    start: Option<Duration>,

    #[structopt(
        short = "d",
        long = "duration",
        help = "duration to use from input audio, starting at start time if given (hh:mm:ss.ssss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    duration: Option<Duration>,
}

fn main() -> Result<()> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();
    let mut audio: Audio<f32> = load_audio(&opt);
    audio.amplify_in_place(opt.amplitude);
    let spec = audio.spec;
    let total_samples = audio.data[0].len();
    let player = AudioOutputProcessor::new(spec, Some(total_samples));
    let (player_ctrl_tx, player_ctrl_rx) = unbounded();
    let player_join_handle = player.start(player_ctrl_rx);
    let bus = AudioBus::from_audio(audio);
    player_ctrl_tx.send(AudioOutputProcessorControlMessage::ConnectBus {
        id: 0,
        bus,
        fade: None,
        shutdown_when_finished: true,
    });
    player_join_handle.join();
    Ok(())
}

fn load_audio(opt: &Opt) -> Audio<f32> {
    let mut audio = if opt.record {
        recorder::record_audio(&AudioSpec {
            channels: 2,
            sample_rate: 44100,
        })
    } else {
        match &opt.input {
            Some(path) => {
                let mut reader = WavReader::open(path.to_str().unwrap()).unwrap();
                reader.read_all()
            }
            None => {
                let mut reader = WavReader::new(io::stdin()).unwrap();
                reader.read_all()
            }
        }
    };
    if opt.start.is_some() || opt.duration.is_some() {
        audio.clip_in_place(opt.start, opt.duration);
    }

    if opt.rotate_channels {
        audio.rotate_channels();
    }
    audio
}
