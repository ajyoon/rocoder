#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();
    (0..len)
        .map(|i| {
            let re = input[(i + 3) % len].0;
            let im = input[(i + (elapsed_ms / 50)) % len].1;
            (re, im)
        })
        .collect()
}
