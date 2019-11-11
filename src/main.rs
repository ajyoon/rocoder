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

fn main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let mut wav_reader = WavReader::open("bach_crucifixus.wav").unwrap();
    let wav_spec = wav_reader.spec();

    let input_channels: Vec<Vec<f32>> = wav_reader.read_into_channels();

    let window = windows::hanning(2_usize.pow(15));
    let channel_futures: Vec<_> = input_channels
        .into_iter()
        .map(|channel_samples| {
            stretcher::stretch(wav_spec.sample_rate, channel_samples, 2.0, window.clone())
        })
        .collect();

    let output_channels: Vec<Vec<f32>> = block_on(future::join_all(channel_futures));

    let mut writer = WavWriter::open("out.wav", wav_spec).unwrap();
    writer.write_into_channels(output_channels);
    writer.finalize().unwrap();
    Ok(())
}
