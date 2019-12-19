use rocoder::audio::{Audio, AudioSpec};
use rocoder::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use rocoder::duration_parser;
use rocoder::player;
use rocoder::recorder;
use rocoder::runtime_setup;
use rocoder::stretcher;
use rocoder::windows;

use anyhow::Result;
use async_std;
use crossbeam_channel::{bounded, Receiver, Sender};
use futures::executor::block_on;
use futures::future;

use std::io;
use std::path::PathBuf;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[derive(Debug, StructOpt)]
#[structopt(name = "rocoder", setting = AppSettings::AllowNegativeNumbers)]
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
        long = "freq-kernel",
        help = "Path to a rust frequency kernel file",
        parse(from_os_str)
    )]
    freq_kernel: Option<PathBuf>,

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

fn main() -> Result<()> {
    block_on(async_main())
}

async fn async_main() -> Result<()> {
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
                    opt.freq_kernel.clone(),
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

fn handle_result(opt: &Opt, mut output_audio: Audio<f32>) -> Result<()> {
    output_audio.fade_in(Duration::from_secs(0), opt.fade);
    output_audio.fade_out(output_audio.duration() - opt.fade, opt.fade);
    match &opt.output {
        Some(path) => {
            let mut writer = WavWriter::open(path.to_str().unwrap(), output_audio.spec).unwrap();
            writer.write_into_channels(output_audio.data)?;
            writer.finalize().unwrap();
        }
        None => {
            let spec = output_audio.spec;
            let total_samples_len = output_audio.data[0].len();
            let (tx, rx) = bounded::<Audio<f32>>(10);
            tx.send(output_audio);
            drop(tx);
            player::play_audio(spec, rx, Some(total_samples_len));
        }
    }
    Ok(())
}
