use std::cmp::Ordering::*;

/// Clamp a value to be within a given min and max value, inclusive.
#[allow(unused)]
#[inline]
pub fn clamp<T: PartialOrd>(val: T, min: T, max: T) -> T {
    match val.partial_cmp(&min) {
        Some(ordering) => match ordering {
            Less => min,
            _ => match val.partial_cmp(&max) {
                Some(Greater) => max,
                _ => val,
            },
        },
        None => val,
    }
}

pub fn partial_min<T: PartialOrd>(left: T, right: T) -> T {
    if left < right {
        left
    } else {
        right
    }
}

#[inline]
pub fn lerp(start: f32, end: f32, ratio: f32) -> f32 {
    start + (end - start) * ratio
}

#[inline]
pub fn sqrt_interp(start: f32, end: f32, ratio: f32) -> f32 {
    let increasing = start < end;
    // Reshape `ratio` to be within a domain of -1 -> 1
    let progress = ((ratio * 2.0) - 1.0) * if increasing { 1.0 } else { -1.0 };
    let factor = (0.5 * (1.0 + progress)).sqrt();
    let abs_interval = (end - start).abs();
    (if increasing { start } else { end }) + (abs_interval * factor)
}

#[cfg(test)]
mod test_clamp {
    use super::*;

    #[test]
    fn test_clamp_below_min() {
        assert_eq!(clamp(-1, 0, 10), 0);
    }

    #[test]
    fn test_clamp_above_max() {
        assert_eq!(clamp(11, 0, 10), 10);
    }

    #[test]
    fn test_clamp_within_bounds() {
        assert_eq!(clamp(5, 0, 10), 5);
    }
}

#[cfg(test)]
mod test_lerp {
    use super::*;
    use crate::test_utils::*;

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
