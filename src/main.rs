use yoonstretch::runtime_setup;
use yoonstretch::stretcher;
use yoonstretch::wav;
use yoonstretch::windows;

fn main() {
    runtime_setup::setup_logging();
    let input_samples: Vec<f32> = wav::read("melodicas.wav");
    //let input_samples: Vec<f32> = wav::read("goldberg.wav");
    let window = windows::hanning(4096 * 4);
    let result = stretcher::stretch(44100, &input_samples, 30.0, window);
    wav::write("out.wav", &result);
}
