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
use std::ops::MulAssign;
use std::path::PathBuf;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(name = "yoonstretch", setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
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
}

fn main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();
    let mut audio: Audio<f32> = load_audio(&opt);
    audio.amplify_in_place(opt.amplitude);
    player::play_audio(&audio.spec, audio.data);
    Ok(())
}

fn load_audio<T>(opt: &Opt) -> Audio<T>
where
    T: Sized + Num + Copy + MulAssign + hound::Sample,
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
