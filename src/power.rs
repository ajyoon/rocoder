use crate::audio::{Audio, AudioSpec, Sample};
use std::time::Duration;

const MIN_DECIBELS: f32 = -99999999.0;

/// Convert a linear amplitude (0-1) to a decibel measurement relative to max amplitude
///
/// To prevent `-inf` returns, the minimal return value is -99999999.0
fn relative_decibels(raw_amp: f32) -> f32 {
    (raw_amp.abs().log10() * 20.0).max(MIN_DECIBELS)
}

pub fn audio_power(audio: &[f32]) -> f32 {
    let raw_amp = audio
        .iter()
        .map(|s| s.abs())
        .max_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    return relative_decibels(raw_amp);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_relative_decibels() {
        assert_eq!(relative_decibels(0.0), MIN_DECIBELS);
        assert_almost_eq(relative_decibels(0.1), -19.999999999);
        assert_almost_eq(relative_decibels(1.0), 0.0);
    }
}
