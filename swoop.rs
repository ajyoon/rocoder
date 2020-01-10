#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();
    let t = ((elapsed_ms as f64 / 100.0).sin() * 40.0) as i64;
    let noise = 0.6;
    (0..len)
        .map(|i| {
            let re = input[((i as i64 + t) % len as i64).abs() as usize].0 + noise;
            let im = if elapsed_ms % 20 != 0 || i % 3 != 0 {
                input[i].1
            } else {
                input[(i + (elapsed_ms / 30)) % len].1 + noise
            };
            (re, im)
        })
        .collect()
}
