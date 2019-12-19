#[no_mangle]
pub fn test(input: Vec<f32>) -> Vec<f32> {
    return input.iter().map(|x| x * 3.0).collect();
}
