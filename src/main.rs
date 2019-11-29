use yoonstretch::audio::{Audio, AudioSpec};
use yoonstretch::audio_files::{AudioReader, AudioWriter, Mp3Reader, WavReader, WavWriter};
use yoonstretch::duration_parser;
use yoonstretch::player;
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use async_std;
use futures::executor::block_on;
use futures::future;
use num_traits::Num;
use std::error::Error;

use std::io::{self, Read};
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
    handle_result(&opt, &spec, output_channels);
    Ok(())
}

fn load_audio<T>(opt: &Opt) -> Audio<T>
where
    T: Sized + Num + Copy + hound::Sample,
{
    let mut audio = match &opt.input {
        Some(path) => {
            let mut reader = WavReader::open(path.to_str().unwrap()).unwrap();
            reader.read_all()
        }
        None => {
            let mut reader = WavReader::new(io::stdin()).unwrap();
            reader.read_all()
        }
    };

    if opt.start.is_some() || opt.duration.is_some() {
        audio.clip_in_place(opt.start, opt.duration);
    }

    audio
}

fn handle_result(
    opt: &Opt,
    spec: &AudioSpec,
    output_channels: Vec<Vec<f32>>,
) -> Result<(), Box<dyn Error>> {
    match &opt.output {
        Some(path) => {
            let mut writer = WavWriter::open(path.to_str().unwrap(), *spec).unwrap();
            writer.write_into_channels(output_channels)?;
            writer.finalize().unwrap();
        }
        None => {
            player::play_audio(spec, output_channels);
        }
    }
    Ok(())
}
