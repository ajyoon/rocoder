use yoonstretch::audio::{Audio, AudioSpec};
use yoonstretch::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use yoonstretch::duration_parser;
use yoonstretch::player;
use yoonstretch::recorder;
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use async_std;
use futures::executor::block_on;
use futures::future;
use std::error::Error;

use std::io;
use std::path::PathBuf;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(name = "yoonstretch", setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    #[structopt(short = "w", long = "window", default_value = "32768")]
    window_len: usize,

    #[structopt(short = "f", long = "factor")]
    factor: f32,

    #[structopt(short = "p", long = "pitch_multiple", default_value = "1")]
    pitch_multiple: i8,

    #[structopt(short = "a", long = "amplitude", default_value = "1")]
    amplitude: f32,

    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: Option<PathBuf>,

    #[structopt(
        long = "rotate-channels",
        help = "Rotate the input audio channels. With stereo audio this means swapping the left and right channels"
    )]
    rotate_channels: bool,

    #[structopt(
        short = "x",
        long = "fade",
        default_value = "1",
        parse(try_from_str = duration_parser::parse_duration),
        help = "fade generated audio in and out for the given duration (hh:mm:ss.ss)")]
    fade: Duration,

    #[structopt(short = "r", long = "record", conflicts_with = "input")]
    record: bool,

    #[structopt(
        short = "s",
        long = "start",
        help = "start time in input audio (hh:mm:ss.ss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    start: Option<Duration>,

    #[structopt(
        short = "d",
        long = "duration",
        help = "duration to use from input audio, starting at start time if given (hh:mm:ss.ss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    duration: Option<Duration>,

    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn Error>> {
    block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let audio = load_audio(&opt);
    let spec = audio.spec;
    let window = windows::hanning(opt.window_len);

    let output_channels: Vec<Vec<f32>> = future::join_all(
        audio
            .data
            .into_iter()
            .enumerate()
            .map(|(i, channel_samples)| {
                stretcher::stretch(
                    spec.sample_rate,
                    channel_samples,
                    opt.factor,
                    opt.amplitude,
                    opt.pitch_multiple,
                    window.clone(),
                    i.to_string(),
                )
            })
            .map(async_std::task::spawn),
    )
    .await;
    let output_audio = Audio {
        data: output_channels,
        spec,
    };
    handle_result(&opt, output_audio)?;
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

fn handle_result(opt: &Opt, mut output_audio: Audio<f32>) -> Result<(), Box<dyn Error>> {
    output_audio.fade_in(Duration::from_secs(0), opt.fade);
    output_audio.fade_out(output_audio.duration() - opt.fade, opt.fade);
    match &opt.output {
        Some(path) => {
            let mut writer = WavWriter::open(path.to_str().unwrap(), output_audio.spec).unwrap();
            writer.write_into_channels(output_audio.data)?;
            writer.finalize().unwrap();
        }
        None => {
            player::play_audio(output_audio);
        }
    }
    Ok(())
}
