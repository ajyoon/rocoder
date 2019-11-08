use std::f32;

/// Taken from paulstretch
pub fn hanning_crossfade_compensation(len: usize) -> Vec<f32> {
    let two_pi = f32::consts::PI * 2.0;
    let hinv_sqrt2 = (1.0 + f32::sqrt(0.5).sqrt()) * 0.5;
    (0..len)
        .map(|i| 0.5 - ((1.0 - hinv_sqrt2) * f32::cos((i as f32 * two_pi) / (len - 1) as f32)))
        .collect()
}
