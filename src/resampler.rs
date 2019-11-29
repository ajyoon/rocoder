pub fn resample(samples: Vec<f32>, factor: i8) -> Vec<f32> {
    if factor > 1 {
        resample_faster(samples, factor as usize)
    } else if factor < 1 {
        resample_slower(samples, factor.abs() as usize)
    } else {
        panic!("invalid resample factor");
    }
}

fn resample_faster(samples: Vec<f32>, factor: usize) -> Vec<f32> {
    debug_assert!(factor > 1);
    samples.into_iter().step_by(factor).collect()
}

fn resample_slower(samples: Vec<f32>, factor: usize) -> Vec<f32> {
    debug_assert!(factor > 1);
    let mut result = Vec::with_capacity(samples.len() * factor);
    for i in 0..samples.len() - 1 {
        let current_src_sample = samples[i];
        let next_src_sample = samples[i + 1];
        for j in 0..factor {
            result.push(lerp(
                current_src_sample,
                next_src_sample,
                j as f32 / factor as f32,
            ))
        }
    }
    result
}

#[inline]
fn lerp(start: f32, end: f32, ratio: f32) -> f32 {
    start + (end - start) * ratio
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_resample_faster() {
        assert_almost_eq_by_element(resample_faster(vec![1.0, 2.0, 3.0, 4.0], 2), vec![1.0, 3.0]);
    }

    #[test]
    fn test_lerp_across_0_to_1() {
        assert_almost_eq(lerp(0.0, 1.0, 0.6), 0.6);
    }

    #[test]
    fn test_lerp_across_arbitrary_range() {
        assert_almost_eq(lerp(234.287, 239847.45, 0.6), 144002.18480000002);
    }

    #[test]
    fn test_lerp_descending() {
        assert_almost_eq(lerp(10.0, -20.0, 0.5), -5.0);
    }
}
