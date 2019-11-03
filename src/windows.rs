use std::f32;

/// The 'upper part' of a cosine period
pub fn hanning(len: usize) -> Vec<f32> {
    let two_pi = f32::consts::PI * 2.0;
    (0..len)
        .map(|i| 0.5 - (f32::cos((i as f32 * two_pi) / (len - 1) as f32) * 0.5))
        .collect()
}

/// A naive always-1.0 window
pub fn rectangular(len: usize) -> Vec<f32> {
    vec![1.0; len]
}

/// Compute a vec where every element times its original = 1
///
/// Assumes no elements in `elements` is 0.0
pub fn inverse(elements: &[f32]) -> Vec<f32> {
    elements.iter().map(|f| 1.0 / f).collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    #[rustfmt::skip]
    fn hanning_result() {
        let result = hanning(32);
        let expected = vec![
            0.0, 0.010235041, 0.040521085, 0.089618266, 
	    0.15551656, 0.23551801, 0.32634735, 0.4242862, 
	    0.5253246, 0.6253263, 0.7201971, 0.80605304, 
	    0.87937903, 0.93717337, 0.9770697, 0.9974347, 
	    0.9974346, 0.9770696, 0.9371733, 0.87937903,
	    0.8060529, 0.720197, 0.62532616, 0.5253247,
	    0.4242862, 0.32634744, 0.23551783, 0.15551639,
	    0.08961815, 0.040521085, 0.010235012, 0.0,
        ];
	assert_almost_eq_by_element(result, expected);
    }

    #[test]
    fn rectangular_result() {
        let result = rectangular(4);
        assert_almost_eq_by_element(result, vec![1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn inv_hanning() {
        let basis = vec![1.0, 0.7, 0.3];
        let result = inverse(&basis);
        let expected = vec![1.0, 1.4285715, 3.3333333];
        assert_almost_eq_by_element(result, expected)
    }
}
