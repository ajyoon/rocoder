use crate::audio::{Audio, AudioSpec, Sample};
use anyhow::{anyhow, Result};
use hound;
use minimp3;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::iter::FromIterator;
use std::marker::Sized;

pub trait AudioReader<R>: Iterator<Item = f32>
where
    R: Read,
{
    /// Create a new decoding reader from an existing data reader.
    ///
    /// Audio metadata is read immediately, while sample data will be done on demand.
    fn new(reader: R) -> Result<Self>
    where
        Self: Sized;

    /// Duration in samples, regardless of number of channels
    fn duration(&self) -> Option<u32>;

    /// Total number in samples. This will be `duration * channels`.
    fn num_samples(&self) -> Option<u32>;

    fn spec(&self) -> AudioSpec;

    fn read_all(&mut self) -> Audio {
        let num_channels = self.spec().channels as usize;
        let mut channels: Vec<Vec<f32>> = (0..num_channels)
            .map(|_| match self.duration() {
                Some(dur) => Vec::with_capacity(dur as usize),
                None => Vec::new(),
            })
            .collect();

        for (i, sample) in self.enumerate() {
            // Wav streams are interleaved, so we separate them here
            let sample_channel = i % num_channels;
            channels[sample_channel].push(sample);
        }

        Audio {
            data: channels,
            spec: self.spec(),
        }
    }
}

pub trait AudioWriter<W>: Sized
where
    W: Write + Seek,
{
    fn new(writer: W, spec: AudioSpec) -> Result<Self>
    where
        Self: Sized;

    fn write(&mut self, sample: f32) -> Result<()>
    where
        Self: Sized;

    fn finalize(self) -> Result<()>
    where
        Self: Sized;

    fn write_into_channels(&mut self, channels: Vec<Vec<f32>>) -> Result<()> {
        // Sanity check that each channel has the same length, and that there is at least one channel
        debug_assert!(HashSet::<usize>::from_iter(channels.iter().map(|c| c.len())).len() == 1);
        let samples_per_channel = channels.get(0).unwrap().len();

        for i in 0..samples_per_channel {
            for channel in &channels {
                unsafe {
                    self.write(*channel.get_unchecked(i))?;
                }
            }
        }
        Ok(())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct WavReader<R> {
    pub spec: AudioSpec,
    underlier: hound::WavReader<R>,
    duration: u32,
    num_samples: u32,
}

impl WavReader<io::BufReader<fs::File>> {
    pub fn open(path: &str) -> Result<Self> {
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        WavReader::new(reader)
    }
}

impl<R> WavReader<R> {
    fn validate_num_samples(num_samples: u32, channels: u16) -> Result<()> {
        return if num_samples % channels as u32 != 0 {
            Err(anyhow!(
                "num_samples {} is not a multiple of channel count {}",
                num_samples,
                channels
            ))
        } else {
            Ok(())
        };
    }
}

impl<R> AudioReader<R> for WavReader<R>
where
    R: Read,
{
    fn new(reader: R) -> Result<Self>
    where
        Self: Sized,
    {
        let underlier = hound::WavReader::new(reader)?;
        let hound_spec = underlier.spec();
        let spec = AudioSpec {
            channels: hound_spec.channels,
            sample_rate: hound_spec.sample_rate,
        };
        let duration = underlier.duration();
        let num_samples = underlier.len();
        Self::validate_num_samples(num_samples, spec.channels)?;
        Ok(WavReader {
            underlier,
            spec,
            duration,
            num_samples,
        })
    }

    fn duration(&self) -> Option<u32> {
        Some(self.duration)
    }

    fn num_samples(&self) -> Option<u32> {
        Some(self.num_samples)
    }

    fn spec(&self) -> AudioSpec {
        self.spec
    }
}

impl<R> Iterator for WavReader<R>
where
    R: Read,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let format = (
            self.underlier.spec().sample_format,
            self.underlier.spec().bits_per_sample,
        );
        match format {
            (hound::SampleFormat::Float, 32) => self.underlier.samples().next().map(|s| s.unwrap()),
            (hound::SampleFormat::Int, 8) => self
                .underlier
                .samples()
                .next()
                .map(|s| f32::from_i8(s.unwrap())),
            (hound::SampleFormat::Int, 16) => self
                .underlier
                .samples()
                .next()
                .map(|s| f32::from_i16(s.unwrap())),
            (hound::SampleFormat::Int, 24) => self
                .underlier
                .samples()
                .next()
                .map(|s| f32::from_i24(s.unwrap())),
            (hound::SampleFormat::Int, 32) => self
                .underlier
                .samples()
                .next()
                .map(|s| f32::from_i32(s.unwrap())),
            _ => panic!("Cannot read unsupported .wav format: {:?}", format),
        }
    }
}

pub struct WavWriter<W>
where
    W: Seek + Write,
{
    pub spec: AudioSpec,
    underlier: hound::WavWriter<W>,
}

impl<W> AudioWriter<W> for WavWriter<W>
where
    W: Write + Seek,
{
    fn new(writer: W, spec: AudioSpec) -> Result<Self> {
        let hound_spec = hound::WavSpec {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let underlier = hound::WavWriter::new(writer, hound_spec)?;
        Ok(WavWriter { spec, underlier })
    }

    fn write(&mut self, sample: f32) -> Result<()>
    where
        Self: Sized,
    {
        Ok(self.underlier.write_sample(sample)?)
    }

    fn finalize(self) -> Result<()>
    where
        Self: Sized,
    {
        Ok(self.underlier.finalize()?)
    }
}

impl WavWriter<io::BufWriter<fs::File>> {
    pub fn open(path: &str, spec: AudioSpec) -> Result<Self> {
        let file = fs::File::create(path)?;
        let buf_writer = io::BufWriter::new(file);
        WavWriter::new(buf_writer, spec)
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct Mp3Reader<R> {
    pub spec: AudioSpec,
    underlier: minimp3::Decoder<R>,
    buffer: Vec<i16>,
    buffer_i: usize,
}

impl<R> AudioReader<R> for Mp3Reader<R>
where
    R: Read,
{
    fn new(reader: R) -> Result<Self>
    where
        Self: Sized,
    {
        let mut underlier = minimp3::Decoder::new(reader);

        let first_frame = underlier.next_frame()?;
        let spec = AudioSpec {
            channels: first_frame.channels as u16,
            sample_rate: first_frame.sample_rate as u32,
        };
        let buffer = first_frame.data;
        let buffer_i = 0;

        Ok(Mp3Reader {
            spec,
            underlier,
            buffer,
            buffer_i,
        })
    }

    fn duration(&self) -> Option<u32> {
        None
    }

    fn num_samples(&self) -> Option<u32> {
        None
    }

    fn spec(&self) -> AudioSpec {
        self.spec
    }
}

impl Mp3Reader<io::BufReader<fs::File>> {
    pub fn open(path: &str) -> Result<Self> {
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        Mp3Reader::new(reader)
    }
}

impl<R> Mp3Reader<R>
where
    R: Read,
{
    // TODO seems weird to use `unsafe` and manually implement the buffer here.
    //      why not use a std::io::BufReader instead?
    fn next_i16_sample(&mut self) -> Option<i16> {
        if self.buffer_i < self.buffer.len() {
            let result = Some(unsafe { *self.buffer.get_unchecked(self.buffer_i) });
            self.buffer_i += 1;
            result
        } else {
            let next_frame = self.underlier.next_frame().ok()?;
            debug_assert!(next_frame.channels as u16 == self.spec.channels);
            debug_assert!(next_frame.sample_rate as u32 == self.spec.sample_rate);
            debug_assert!(next_frame.data.len() > 0);
            self.buffer_i = 1;
            self.buffer = next_frame.data;
            Some(unsafe { *self.buffer.get_unchecked(0) })
        }
    }
}

impl<R> Iterator for Mp3Reader<R>
where
    R: Read,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.next_i16_sample().map(f32::from_i16)
    }
}
