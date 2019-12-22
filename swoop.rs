#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();
    let t = ((elapsed_ms as f64 / 100.0).sin() * 300.0) as i64;
    (0..len)
        .map(|i| {
            let re = input[((i as i64 + t) % len as i64).abs() as usize].0;
            let im = if elapsed_ms % 5 != 0 || i % 3 != 0 {
                input[i].1
            } else {
                input[(i + (elapsed_ms / 10)) % len].1
            };
            (re, im)
        })
        .collect()
}
