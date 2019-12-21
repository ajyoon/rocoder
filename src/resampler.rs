use crate::math::lerp;

pub fn resample(samples: &[f32], factor: i8) -> Vec<f32> {
    if factor == 1 {
        samples.to_vec()
    } else if factor > 1 {
        resample_faster(samples, factor as usize)
    } else if factor < -1 {
        resample_slower(samples, factor.abs() as usize)
    } else {
        panic!("invalid resample factor");
    }
}

fn resample_faster(samples: &[f32], factor: usize) -> Vec<f32> {
    debug_assert!(factor > 1);
    samples.iter().step_by(factor).map(|s| *s).collect()
}

fn resample_slower(samples: &[f32], factor: usize) -> Vec<f32> {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_resample_noop() {
        let v = vec![1.0, 2.0, 3.0];
        assert_almost_eq_by_element(resample(&v, 1), v);
    }

    #[test]
    fn test_resample_faster() {
        assert_almost_eq_by_element(
            resample_faster(&vec![1.0, 2.0, 3.0, 4.0], 2),
            vec![1.0, 3.0],
        );
    }
}
