#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();

    (0..len)
        .map(|i| {
            if i % 3 != 0 {
                (input[i].0, input[i].1)
            } else {
                let t = ((elapsed_ms as f32 / 40.0).sin() * 100.0) as usize;
                (input[(i + t) % len].0, input[(i + t) % len].1)
            }
        })
        .collect()
}
