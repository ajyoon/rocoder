use rocoder::audio::{Audio, AudioSpec, AudioBus};
use rocoder::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use rocoder::duration_parser;
use rocoder::recorder;
use rocoder::runtime_setup;
use rocoder::stretcher::Stretcher;
use rocoder::windows;
use rocoder::signal_flow::node::Processor;
use rocoder::player_processor::{AudioOutputProcessor, AudioOutputProcessorControlMessage};

use anyhow::Result;
use crossbeam_channel::unbounded;

use std::io;
use std::path::PathBuf;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[macro_use]
extern crate log;

#[derive(Debug, StructOpt)]
#[structopt(name = "rocoder", setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    #[structopt(short = "w", long = "window", default_value = "16384")]
    window_len: usize,

    #[structopt(
        short = "b", 
        long = "buffer", 
        default_value = "1", 
        parse(try_from_str = duration_parser::parse_duration), 
        help = "the maximum amount of audio to process ahead of time. this controls the response time to changes like kernel modifications.")]
    buffer_dur: Duration,

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
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let audio = load_audio(&opt);
    let total_samples_len = audio.data[0].len();
    let spec = audio.spec;
    let window = windows::hanning(opt.window_len);

    let channel_receivers = audio
        .data
        .into_iter()
        .map(|channel| {
            let (stretcher_in_tx, stretcher_in_rx) = unbounded();
            let stretcher = Stretcher::new(
                spec,
                stretcher_in_rx,
                opt.factor,
                opt.amplitude,
                opt.pitch_multiple,
                window.clone(),
                opt.buffer_dur,
                opt.freq_kernel.clone(),
            );
            if stretcher_in_tx.send(channel).is_err() {
                warn!("failed to send channel data");
            }
            stretcher.into_thread()
        })
        .collect();

    let expected_total_samples = Some((total_samples_len as f32 * opt.factor) as usize);
    let bus = AudioBus {
        spec,
        expected_total_samples,
        channels: channel_receivers,
    };
    handle_result(&opt, bus)?;
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

fn handle_result(
    opt: &Opt,
    audio_bus: AudioBus,
) -> Result<()> {
    match &opt.output {
        Some(path) => {
            let output_audio = audio_bus.into_audio();
            let mut writer = WavWriter::open(path.to_str().unwrap(), output_audio.spec).unwrap();
            writer.write_into_channels(output_audio.data)?;
            writer.finalize().unwrap();
        }
        None => {
            let player = AudioOutputProcessor::new(audio_bus.spec, audio_bus.expected_total_samples);
            let (player_ctrl_tx, player_ctrl_rx) = unbounded();
            let player_join_handle = player.start(player_ctrl_rx);
            player_ctrl_tx.send(AudioOutputProcessorControlMessage::ConnectBus {
                id: 0,
                bus: audio_bus,
                fade: Some(opt.fade),
                shutdown_when_finished: true,
            }).unwrap();
            player_join_handle.join().unwrap();
        }
    }
    Ok(())
}
