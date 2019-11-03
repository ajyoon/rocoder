use yoonstretch::stretcher;
use yoonstretch::wav;
use yoonstretch::windows;

fn main() {
    let input_samples: Vec<f32> = wav::read("/home/ayoon/tools/paulstretch_python/test.wav");
    let window = windows::hanning(10000);
    let result = stretcher::stretch(&input_samples, 4.0, window);
    wav::write("out.wav", &result);
}
