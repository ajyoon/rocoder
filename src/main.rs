use yoonstretch::audio_files::{AudioReader, AudioWriter, Mp3Reader, WavReader, WavWriter};
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use async_std;
use futures::executor::block_on;
use futures::future;
use std::error::Error;

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
    block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let opt = Opt::from_args();

    let mut reader = Mp3Reader::open(opt.input.to_str().unwrap()).unwrap();
    let spec = reader.spec();

    let input_channels: Vec<Vec<f32>> = reader.read_into_channels();

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
