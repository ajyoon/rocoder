use yoonstretch::audio_files::{AudioReader, AudioWriter, WavReader, WavWriter};
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::windows;

use std::error::Error;
use std::fs;
use std::io::{self, Read, Seek};
use std::marker::{self, PhantomData, Sized};

fn main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let wav_reader = WavReader::open("bach_crucifixus.wav").unwrap();
    let wav_spec = wav_reader.spec();
    let input_samples: Vec<f32> = wav_reader.collect();
    let window = windows::hanning(2_usize.pow(15));
    let result = stretcher::stretch(44100, &input_samples, 2.0, window);
    let mut writer = WavWriter::open("out.wav", wav_spec).unwrap();
    result.into_iter().for_each(|s| writer.write(s).unwrap());
    writer.finalize().unwrap();
    Ok(())
}
