#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();

    (0..len)
        .map(|i| {
            if i % 3 != 0 {
                (input[i].0, input[i].1)
            } else {
                (0.0, 0.0)
            }
        })
        .collect()
}
