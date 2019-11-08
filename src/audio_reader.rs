use hound;
use num_traits::Num;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Seek};
use std::marker::{self, PhantomData, Sized};

pub struct AudioSpec {
    /// Number of audio channels (e.g. 2 for stereo)
    pub channels: u16,
    /// Number of samples per second
    pub sample_rate: u32,
    /// Duration in samples, regardless of number of channels
    pub duration: u32,
    /// Total number in samples. This will be `duration * channels`.
    pub num_samples: u32,
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
}

// Is there a way to do this without a macro?
// Since it requires a specialization of R, I can't find one.
macro_rules! impl_from_path {
    ($type_name: ident) => {
        impl<T> $type_name<T, io::BufReader<fs::File>>
        where
            T: Sized + Num + hound::Sample,
        {
            pub fn open(path: &str) -> Result<Self, Box<dyn Error>> {
                let file = fs::File::open(path)?;
                let reader = io::BufReader::new(file);
                $type_name::new(reader)
            }
        }
    };
}

pub struct WavReader<T, R> {
    pub spec: AudioSpec,
    underlier: hound::WavIntoSamples<R, T>,
}

impl_from_path!(WavReader);

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
                return Err(Box::new(result.err().unwrap()));
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
            duration: hound_wav_reader.duration(),
            num_samples: hound_wav_reader.len(),
        };
        let underlier = hound_wav_reader.into_samples();
        Ok(WavReader { underlier, spec })
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
