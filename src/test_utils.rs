use crate::audio::{Audio, AudioSpec};
use std::fmt::Debug;

const F32_EPSILON: f32 = 1.0e-4;

#[allow(unused)]
pub fn assert_almost_eq_by_element(left: Vec<f32>, right: Vec<f32>) {
    if left.len() != right.len() {
        panic!(
            "lengths differ: left.len() = {}, right.len() = {}",
            left.len(),
            right.len()
        );
    }
    for (left_val, right_val) in left.iter().zip(right.iter()) {
        assert!(
            f32_almost_eq(*left_val, *right_val),
            "{} is not approximately equal to {}. \
             complete left vec: {:?}. complete right vec: {:?}",
            *left_val,
            *right_val,
            left,
            right
        );
    }
}

#[allow(unused)]
pub fn assert_eq_by_element<T>(left: Vec<T>, right: Vec<T>)
where
    T: PartialEq + Debug,
{
    if left.len() != right.len() {
        panic!(
            "lengths differ: left.len() = {:?}, right.len() = {:?}",
            left.len(),
            right.len()
        );
    }
    for (left_val, right_val) in left.iter().zip(right.iter()) {
        assert!(
            left_val == right_val,
            "{:?} is not equal to {:?}. \
             complete left side: \n{:?} \n \
             complete right side: \n{:?} \n",
            *left_val,
            *right_val,
            left,
            right
        );
    }
}

#[allow(unused)]
pub fn assert_almost_eq(left: f32, right: f32) {
    assert!(
        f32_almost_eq(left, right),
        "{} is not approximately equal to {}.",
        left,
        right,
    );
}

#[allow(unused)]
fn f32_almost_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < F32_EPSILON
}

#[allow(unused)]
pub fn generate_audio(fill_val: f32, len: usize, channels: u16, sample_rate: u32) -> Audio<f32> {
    let spec = AudioSpec {
        channels,
        sample_rate,
    };
    let mut audio = Audio::from_spec(&spec);
    for channel in audio.data.iter_mut() {
        for _ in 0..len {
            channel.push(fill_val);
        }
    }
    audio
}
