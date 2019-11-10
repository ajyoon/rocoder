use hound;
use num_traits::{Float, Num, PrimInt};
use std::error::Error;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::marker::{self, PhantomData, Sized};
use std::mem;

#[derive(Copy, Clone, Debug)]
pub struct AudioSpec {
    /// Number of audio channels (e.g. 2 for stereo)
    pub channels: u16,
    /// Number of samples per second
    pub sample_rate: u32,
}

pub trait AudioReader<T, R>: Iterator
where
    T: Sized + Num,
    R: Read + Seek,
{
    /// Create a new decoding reader from an existing data reader.
    ///
    /// Audio metadata is read immediately, while sample data will be done on demand.
    fn new(reader: R) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;

    /// Duration in samples, regardless of number of channels
    fn duration(&self) -> u32;

    /// Total number in samples. This will be `duration * channels`.
    fn num_samples(&self) -> u32;

    fn spec(&self) -> AudioSpec;
}

pub trait AudioWriter<T, W>
where
    T: Sized + Num,
    W: Write + Seek,
{
    fn new(writer: W, spec: AudioSpec) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized;

    fn write(&mut self, sample: T) -> Result<(), Box<dyn Error>>
    where
        Self: Sized;

    fn finalize(self) -> Result<(), Box<dyn Error>>
    where
        Self: Sized;
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct WavReader<T, R> {
    pub spec: AudioSpec,
    underlier: hound::WavIntoSamples<R, T>,
    duration: u32,
    num_samples: u32,
}

impl<T> WavReader<T, io::BufReader<fs::File>>
where
    T: Sized + Num + hound::Sample,
{
    pub fn open(path: &str) -> Result<Self, Box<dyn Error>> {
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        WavReader::new(reader)
    }
}

impl<T, R> WavReader<T, R> {
    fn validate(hound_wav_reader: &mut hound::WavReader<R>) -> Result<(), Box<dyn Error>>
    where
        T: Sized + Num + hound::Sample,
        R: Read + Seek,
    {
        let first_sample_returned = hound_wav_reader.samples::<T>().next();
        // reset
        let _ = hound_wav_reader.seek(0);
        if let Some(result) = first_sample_returned {
            // urgh.. trying to unpack option of hound result to dyn result...
            // definitely a better way to do this but I can't find it
            if result.is_err() {
                return Err(Box::from(result.err().unwrap()));
            }
        }
        Ok(())
    }
}

impl<T, R> AudioReader<T, R> for WavReader<T, R>
where
    T: Sized + Num + hound::Sample,
    R: Read + Seek,
{
    fn new(reader: R) -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        // TODO impl from for hound error
        let mut hound_wav_reader = hound::WavReader::new(reader).unwrap();
        Self::validate(&mut hound_wav_reader)?;
        let hound_spec = hound_wav_reader.spec();
        let spec = AudioSpec {
            channels: hound_spec.channels,
            sample_rate: hound_spec.sample_rate,
        };
        let duration = hound_wav_reader.duration();
        let num_samples = hound_wav_reader.len();
        let underlier = hound_wav_reader.into_samples();
        Ok(WavReader {
            underlier,
            spec,
            duration,
            num_samples,
        })
    }

    fn duration(&self) -> u32 {
        self.duration
    }

    fn num_samples(&self) -> u32 {
        self.num_samples
    }

    fn spec(&self) -> AudioSpec {
        self.spec
    }
}

impl<T, R> Iterator for WavReader<T, R>
where
    T: Sized + Num + hound::Sample,
    R: Read,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.underlier.next();
        result.map(|s| s.unwrap())
    }
}

pub struct WavWriter<T, W>
where
    W: Seek + Write,
{
    pub spec: AudioSpec,
    underlier: hound::WavWriter<W>,
    _phantom: PhantomData<T>,
}

trait HoundSampleFormat<T> {
    fn hound_sample_format() -> hound::SampleFormat;
}

macro_rules! impl_hound_sample_format {
    ($type_name: ty, $hound_sample_format: path) => {
        impl<W> HoundSampleFormat<$type_name> for WavWriter<$type_name, W>
        where
            W: Seek + Write,
        {
            fn hound_sample_format() -> hound::SampleFormat {
                $hound_sample_format
            }
        }
    };
}

impl_hound_sample_format!(f32, hound::SampleFormat::Float);
impl_hound_sample_format!(i8, hound::SampleFormat::Int);
impl_hound_sample_format!(i16, hound::SampleFormat::Int);
impl_hound_sample_format!(i32, hound::SampleFormat::Int);

impl<T, W> AudioWriter<T, W> for WavWriter<T, W>
where
    Self: HoundSampleFormat<T>,
    T: Sized + Num + hound::Sample,
    W: Write + Seek,
{
    fn new(writer: W, spec: AudioSpec) -> Result<Self, Box<dyn Error>> {
        let hound_spec = hound::WavSpec {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: mem::size_of::<T>() as u16 * 8,
            sample_format: Self::hound_sample_format(),
        };
        let underlier = hound::WavWriter::new(writer, hound_spec)?;
        Ok(WavWriter {
            spec,
            underlier,
            _phantom: PhantomData,
        })
    }

    fn write(&mut self, sample: T) -> Result<(), Box<dyn Error>>
    where
        Self: Sized,
    {
        self.underlier
            .write_sample(sample)
            .map_err(|e| Box::from(e))
    }

    fn finalize(self) -> Result<(), Box<dyn Error>>
    where
        Self: Sized,
    {
        self.underlier.finalize().map_err(|e| Box::from(e))
    }
}

impl<T> WavWriter<T, io::BufWriter<fs::File>>
where
    Self: HoundSampleFormat<T>,
    T: Sized + Num + hound::Sample,
{
    pub fn open(path: &str, spec: AudioSpec) -> Result<Self, Box<dyn Error>> {
        let file = fs::File::create(path)?;
        let buf_writer = io::BufWriter::new(file);
        WavWriter::new(buf_writer, spec)
    }
}
