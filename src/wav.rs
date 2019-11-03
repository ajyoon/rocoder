use hound;

/// Read from a .wav file
///
/// Assumes the file is at 44.1k and f32 PCM
pub fn read(path: &str) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).unwrap();
    reader.samples::<f32>().map(|s| s.unwrap()).collect()
}

pub fn write(path: &str, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    samples
        .iter()
        .for_each(|s| writer.write_sample(*s).unwrap());
    writer.finalize().unwrap();
}
