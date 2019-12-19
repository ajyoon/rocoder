#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let len = input.len();
    (0..len)
        .map(|i| input[(i + (elapsed_ms / 100)) % len])
        .collect()
}
