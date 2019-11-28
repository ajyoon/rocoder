use yoonstretch::audio_files::{
    AudioReader, AudioSpec, AudioWriter, Mp3Reader, WavReader, WavWriter,
};
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use async_std;
use futures::executor::block_on;
use futures::future;
use std::error::Error;

use std::io::{self, Read};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "yoonstretch")]
struct Opt {
    #[structopt(short = "w", long = "window", default_value = "32768")]
    window_len: usize,

    #[structopt(short = "f", long = "factor")]
    factor: f32,

    #[structopt(short = "i", long = "input", parse(from_os_str))]
    input: Option<PathBuf>,
    //input: PathBuf,
    #[structopt(short = "o", long = "output", parse(from_os_str))]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let (spec, input_channels) = load_channels(&opt);

    let window = windows::hanning(opt.window_len);
    let output_channels: Vec<Vec<f32>> = future::join_all(
        input_channels
            .into_iter()
            .enumerate()
            .map(|(i, channel_samples)| {
                stretcher::stretch(
                    spec.sample_rate,
                    channel_samples,
                    opt.factor,
                    window.clone(),
                    i.to_string(),
                )
            })
            .map(async_std::task::spawn),
    )
    .await;

    let mut writer = WavWriter::open(opt.output.to_str().unwrap(), spec).unwrap();
    writer.write_into_channels(output_channels)?;
    writer.finalize().unwrap();
    Ok(())
}

fn load_channels(opt: &Opt) -> (AudioSpec, Vec<Vec<f32>>) {
    match &opt.input {
        Some(path) => {
            let mut reader = WavReader::open(path.to_str().unwrap()).unwrap();
            (reader.spec(), reader.read_into_channels())
        }
        None => {
            let mut reader = WavReader::new(io::stdin()).unwrap();
            (reader.spec(), reader.read_into_channels())
        }
    }
}
