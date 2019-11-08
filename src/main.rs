use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::wav;
use yoonstretch::windows;

fn main() {
    runtime_setup::setup_logging();
    let input_samples: Vec<f32> = wav::read("bach_cum_sancto.wav");
    let window = windows::hanning(2_usize.pow(12));
    let result = stretcher::stretch(44100, &input_samples, 1.9, window);
    wav::write("out.wav", &result);
}
