use rocoder::audio::{Audio, AudioBus, AudioSpec};
use rocoder::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use rocoder::duration_parser;
use rocoder::player_processor::{AudioOutputProcessor, AudioOutputProcessorControlMessage};
use rocoder::recorder;
use rocoder::runtime_setup;
use rocoder::signal_flow::node::Node;
use rocoder::stretcher::Stretcher;
use rocoder::stretcher_processor::{StretcherProcessor, StretcherProcessorControlMessage};
use rocoder::windows;

use anyhow::Result;
use crossbeam_channel::unbounded;
use ctrlc;

use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use structopt::{clap::AppSettings, StructOpt};

#[macro_use]
extern crate log;

#[derive(Debug, StructOpt)]
#[structopt(name = "rocoder", setting = AppSettings::AllowNegativeNumbers, about = "A live-codeable phase vocoder. See https://github.com/ajyoon/rocoder for detailed docs.")]
struct Opt {
    #[structopt(
        short = "w",
        long = "window",
        default_value = "16384",
        help = "Processing window size"
    )]
    window_len: usize,

    #[structopt(
        short = "b", 
        long = "buffer", 
        default_value = "1", 
        parse(try_from_str = duration_parser::parse_duration),
        help = "The maximum amount of audio to process ahead of time. this controls the response time to changes like kernel modifications.")]
    buffer_dur: Duration,

    #[structopt(
        short = "f",
        long = "factor",
        default_value = "1",
        help = "Stretch factor; e.g. 5 to slow 5x and 0.2 to speed up 5x"
    )]
    factor: f32,

    #[structopt(
        short = "p",
        long = "pitch_multiple",
        default_value = "1",
        help = "A non-zero integer pitch multiplier."
    )]
    pitch_multiple: i8,

    #[structopt(
        short = "a",
        long = "amplitude",
        default_value = "1",
        help = "Output amplitude"
    )]
    amplitude: f32,

    #[structopt(
        short = "i",
        long = "input",
        parse(from_os_str),
        help = "An audio file; currently supports .wav and .mp3. Use '-' for stdin. Omit this option to record audio from your default sound input device."
    )]
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
        help = "Fade generated audio in and out for the given duration (hh:mm:ss.ss)")]
    fade: Duration,

    #[structopt(
        short = "s",
        long = "start",
        help = "Start time in input audio (hh:mm:ss.ss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    start: Option<Duration>,

    #[structopt(
        short = "d",
        long = "duration",
        help = "Duration to use from input audio, starting at start time if given (hh:mm:ss.ss)",
	parse(try_from_str = duration_parser::parse_duration)
    )]
    duration: Option<Duration>,

    #[structopt(
        short = "o",
        long = "output",
        parse(from_os_str),
        help = "Output .wav file path. Uses 32-bit float."
    )]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let audio = load_audio(&opt);
    let total_samples_len = audio.data[0].len();
    let spec = audio.spec;
    let window = windows::hanning(opt.window_len);

    let stretchers = audio
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
            stretcher
        })
        .collect();
    let expected_total_samples = Some((total_samples_len as f32 * opt.factor) as usize);
    let (stretcher_processor, bus) = StretcherProcessor::new(stretchers, expected_total_samples);
    let stretcher_node = Node::new(stretcher_processor);

    handle_result(&opt, bus, stretcher_node)?;
    Ok(())
}

fn load_audio(opt: &Opt) -> Audio {
    let mut audio = match &opt.input {
        Some(path) => {
            if path.to_str() == Some("-") {
                let mut reader = WavReader::new(io::stdin()).unwrap();
                reader.read_all()
            } else {
                let mut reader = WavReader::open(path.to_str().unwrap()).unwrap();
                reader.read_all()
            }
        }
        None => recorder::record_audio(&AudioSpec {
            channels: 2,
            sample_rate: 44100,
        }),
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
    stretcher_node: Node<StretcherProcessor, StretcherProcessorControlMessage>,
) -> Result<()> {
    match &opt.output {
        Some(path) => {
            // This approach requires the entire audio output to fit
            // in memory before we save it. Changes would be needed to
            // stream output directly to disk.
            let output_audio = audio_bus.into_audio();
            let mut writer = WavWriter::open(path.to_str().unwrap(), output_audio.spec).unwrap();
            writer.write_into_channels(output_audio.data)?;
            writer.finalize().unwrap();
        }
        None => {
            play(audio_bus, Some(opt.fade));
        }
    }
    stretcher_node.join();
    Ok(())
}

const PLAY_POLL: Duration = Duration::from_millis(500);

fn play(bus: AudioBus, fade: Option<Duration>) {
    let player_node = Arc::new(Node::new(AudioOutputProcessor::new(bus.spec)));
    player_node
        .send_control_message(AudioOutputProcessorControlMessage::ConnectBus {
            fade,
            bus,
            id: 0,
            shutdown_when_finished: true,
        })
        .unwrap();
    let quit_counter = Arc::new(AtomicU16::new(0));
    let player_node_clone = Arc::clone(&player_node);
    let quit_counter_clone = Arc::clone(&quit_counter);
    ctrlc::set_handler(move || {
        control_c_handler(&quit_counter_clone, Arc::clone(&player_node_clone));
    })
    .unwrap();
    loop {
        thread::sleep(PLAY_POLL);
        if player_node.is_finished() {
            // need to explicitly exit with a non-zero exit code so the control-c quit
            // makes it to the shell so, for instance, bash loops can be broken.
            std::process::exit(1);
        }
    }
}

const QUIT_FADE: Option<Duration> = Some(Duration::from_secs(3));

fn control_c_handler(
    quit_counter: &Arc<AtomicU16>,
    node: Arc<Node<AudioOutputProcessor, AudioOutputProcessorControlMessage>>,
) {
    if quit_counter.fetch_add(1, Ordering::Relaxed) > 0 {
        // If ctrl-c was received more than once, quit without fading out
        println!("\nExiting immediately");
        node.send_control_message(AudioOutputProcessorControlMessage::Shutdown { fade: None })
            .unwrap();
        return;
    }
    println!(
        "\nGot quit signal, fading out audio for {:#?}",
        QUIT_FADE.unwrap()
    );
    node.send_control_message(AudioOutputProcessorControlMessage::Shutdown { fade: QUIT_FADE })
        .unwrap();
}
