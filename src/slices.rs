use libc;
use std::mem;

#[inline]
pub fn zero_slice<T>(slc: &mut [T]) {
    unsafe {
        libc::memset(
            slc.as_mut_ptr() as *mut libc::c_void,
            0,
            mem::size_of::<T>() * slc.len(),
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_zero_slice_f32() {
        let mut v = vec![1.2, 3.4, 0.1];
        zero_slice(&mut v[0..2]);
        assert_almost_eq_by_element(v, vec![0.0, 0.0, 0.1]);
    }

    #[test]
    fn test_zero_slice_u64() {
        let mut v: Vec<u64> = vec![1, 2, 3];
        zero_slice(&mut v[1..3]);
        assert_eq!(v, vec![1, 0, 0]);
    }
}
