use yoonstretch::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use futures::executor::block_on;
use futures::future;
use std::error::Error;
use std::fs;
use std::future::Future;
use std::io::{self, Read, Seek};
use std::marker::{self, PhantomData, Sized};

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "yoonstretch")]
struct Opt {
    #[structopt(short = "w", long = "window", default_value = "32768")]
    window_len: usize,

    #[structopt(short = "f", long = "factor")]
    factor: f32,

    #[structopt(parse(from_os_str))]
    input: PathBuf,

    #[structopt(parse(from_os_str))]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let mut wav_reader = WavReader::open(opt.input.to_str().unwrap()).unwrap();
    let wav_spec = wav_reader.spec();

    let input_channels: Vec<Vec<f32>> = wav_reader.read_into_channels();

    let window = windows::hanning(opt.window_len);
    let channel_futures: Vec<_> = input_channels
        .into_iter()
        .map(|channel_samples| {
            stretcher::stretch(
                wav_spec.sample_rate,
                channel_samples,
                opt.factor,
                window.clone(),
            )
        })
        .collect();

    let output_channels: Vec<Vec<f32>> = block_on(future::join_all(channel_futures));

    let mut writer = WavWriter::open(opt.output.to_str().unwrap(), wav_spec).unwrap();
    writer.write_into_channels(output_channels);
    writer.finalize().unwrap();
    Ok(())
}
