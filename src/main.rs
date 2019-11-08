use yoonstretch::audio_reader::{AudioReader, WavReader};
use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::wav;
use yoonstretch::windows;

use std::error::Error;
use std::fs;
use std::io::{self, Read, Seek};
use std::marker::{self, PhantomData, Sized};

fn main() -> Result<(), Box<dyn Error>> {
    runtime_setup::setup_logging();
    let input_samples: Vec<f32> = WavReader::open("bach_crucifixus.wav").unwrap().collect();
    let window = windows::hanning(2_usize.pow(15));
    let result = stretcher::stretch(44100, &input_samples, 10.0, window);
    wav::write("out.wav", &result);

    Ok(())
}
