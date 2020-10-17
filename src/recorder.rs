use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeInputBuffer,
};
use std::io;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::audio::{Audio, AudioSpec, Sample};
use crate::power;

/// Simple audio recording

const NOISE_ANALYSIS_WINDOW_SIZE: Duration = Duration::from_millis(100);
const NOISE_THRESHOLD_PERCENTILE: usize = 30;

pub fn record_audio(audio_spec: &AudioSpec) -> Audio<f32> {
    // wait_for_enter_keypress("Press ENTER to start recording");
    let host = cpal::default_host();
    let event_loop = Arc::new(host.event_loop());
    let event_loop_arc_for_run = Arc::clone(&event_loop);
    let (raw_samples_sender, raw_samples_receiver) = mpsc::channel::<f32>();

    let input_device = host
        .default_input_device()
        .expect("failed to get default input device");
    println!(
        "Using default input device: \"{}\"",
        input_device.name().unwrap()
    );

    let format = Format {
        channels: audio_spec.channels,
        sample_rate: SampleRate(audio_spec.sample_rate),
        data_type: SampleFormat::F32,
    };
    let input_stream_id = event_loop
        .build_input_stream(&input_device, &format)
        .unwrap();

    event_loop.play_stream(input_stream_id.clone()).unwrap();
    launch_cpal_thread(event_loop_arc_for_run, raw_samples_sender);

    wait_for_enter_keypress("Press ENTER to finish recording");
    event_loop.destroy_stream(input_stream_id);
    let mut audio = collect_samples(audio_spec, raw_samples_receiver);
    autocrop_audio(
        &mut audio,
        NOISE_ANALYSIS_WINDOW_SIZE,
        NOISE_THRESHOLD_PERCENTILE,
    );
    audio
}

fn collect_samples<T>(spec: &AudioSpec, raw_samples_receiver: mpsc::Receiver<T>) -> Audio<T>
where
    T: Sample,
{
    let mut audio = Audio::from_spec(&spec);
    for (i, sample) in raw_samples_receiver.try_iter().enumerate() {
        audio.data[i % spec.channels as usize].push(sample);
    }
    audio
}

fn wait_for_enter_keypress(message: &str) {
    println!("{}", message);
    let mut throwaway_input = String::new();
    match io::stdin().read_line(&mut throwaway_input) {
        Ok(_) => {}
        Err(error) => {
            error!("failed to get input: {}", error);
        }
    }
}

fn launch_cpal_thread<E>(event_loop: Arc<E>, raw_samples_sender: mpsc::Sender<f32>)
where
    E: EventLoopTrait + Send + Sync + 'static,
{
    thread::spawn(move || {
        event_loop.run(move |_stream_id, stream_data| {
            let buffer = match stream_data {
                Ok(res) => match res {
                    StreamData::Input {
                        buffer: UnknownTypeInputBuffer::F32(buffer),
                    } => buffer,
                    _ => panic!("unexpected buffer type"),
                },
                Err(e) => {
                    panic!("failed to fetch get audio stream: {:?}", e);
                }
            };
            for sample in buffer.iter() {
                match raw_samples_sender.send(*sample) {
                    Err(e) => {
                        error!("failed to send recorded sample: {}", e);
                    }
                    _ => (),
                }
            }
        });
    });
}

fn chunked_audio_power(audio: &Audio<f32>, bin_dur: Duration) -> Vec<(usize, f32)> {
    let bin_length = audio.duration_to_sample(bin_dur);
    let sample_dur = audio.data[0].len();
    let mut bins: Vec<(usize, f32)> =
        Vec::with_capacity((sample_dur as f32 / bin_length as f32).ceil() as usize);
    for bin_start_sample in (0..sample_dur).step_by(bin_length) {
        let bin_amplitude = &audio
            .data
            .iter()
            .map(|channel| {
                power::audio_power(
                    &channel[bin_start_sample..(bin_start_sample + bin_length).min(sample_dur)],
                )
            })
            .max_by(|x, y| x.partial_cmp(&y).unwrap())
            .unwrap();
        bins.push((bin_start_sample, *bin_amplitude));
    }
    bins
}

/// Analyze audio to determine when the recording subject begins and ends,
/// and crop to fit it
fn autocrop_audio(audio: &mut Audio<f32>, analysis_window: Duration, threshold_percentile: usize) {
    let amplitudes = chunked_audio_power(&audio, analysis_window);
    let autocrop_points = determine_autocrop_points(&amplitudes, threshold_percentile);
    if autocrop_points.is_none() {
        return;
    }
    let (start, end) = autocrop_points.unwrap();
    let start_time = audio.sample_to_duration(start);
    let clip_dur = audio.sample_to_duration(end - start);
    info!(
        "autocropping audio to start {:?} later and end {:?} earlier",
        start_time,
        audio.sample_to_duration(audio.data[0].len() - end)
    );
    audio.clip_in_place(Some(start_time), Some(clip_dur));
}

fn determine_noise_threshold(amplitudes: &Vec<(usize, f32)>, threshold_percentile: usize) -> f32 {
    debug_assert!(!amplitudes.is_empty());
    debug_assert!(threshold_percentile <= 100);
    let mut working_amplitudes = amplitudes.clone();
    working_amplitudes.sort_unstable_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
    let threshold_index =
        ((threshold_percentile as f32 / 100.0) * working_amplitudes.len() as f32).floor() as usize;
    working_amplitudes[threshold_index].1
}

// assumes `amplitudes` is sorted by sample number
fn determine_autocrop_points(
    amplitudes: &Vec<(usize, f32)>,
    threshold_percentile: usize,
) -> Option<(usize, usize)> {
    let noise_threshold = determine_noise_threshold(amplitudes, threshold_percentile);
    let start_sample = amplitudes.iter().find(|a| a.1 > noise_threshold)?.0;
    let last_signal_bin_index = amplitudes
        .iter()
        .enumerate()
        .rev()
        .find(|a| (a.1).1 > noise_threshold)?
        .0;
    let end_sample = amplitudes[(last_signal_bin_index + 1).min(amplitudes.len() - 1)].0;

    Some((start_sample, end_sample))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_chunked_audio_power() {
        let mut audio = generate_audio(0.0, 5, 2, 2);
        audio.data[0] = vec![
            -0.3, 0.2, // bin 0: amp = 0.3
            -0.1, 0.9, // bin 1: amp = 0.9
            0.0, // bin 2: amp = 0.0
        ];
        audio.data[1][4] = 0.7;

        let amplitudes = chunked_audio_power(&audio, Duration::from_secs(1));

        assert_eq!(amplitudes.len(), 3);
        assert_eq!(amplitudes[0].0, 0);
        assert_almost_eq(amplitudes[0].1, -10.457574);
        assert_eq!(amplitudes[1].0, 2);
        assert_almost_eq(amplitudes[1].1, -0.9151501);
        assert_eq!(amplitudes[2].0, 4);
        assert_almost_eq(amplitudes[2].1, -3.0980396);
    }

    #[test]
    fn test_determine_noise_threshold() {
        let amplitudes = vec![(0, 0.1), (0, 0.0), (0, 1.0), (0, 0.4)];
        assert_eq!(determine_noise_threshold(&amplitudes, 5), 0.0);
        assert_eq!(determine_noise_threshold(&amplitudes, 40), 0.1);
        assert_eq!(determine_noise_threshold(&amplitudes, 50), 0.4);
    }

    #[test]
    fn test_determine_autocrop_points() {
        let amplitudes = vec![
            (0, 0.0),
            (1, 0.1),
            (2, 1.0),
            (3, 0.4),
            (4, 0.8),
            (5, 1.0),
            (6, 0.1),
            (7, 0.0),
        ];
        let (start, stop) = determine_autocrop_points(&amplitudes, 25).unwrap();
        assert_eq!(start, 2);
        assert_eq!(stop, 6);
    }

    #[test]
    fn test_determine_autocrop_points_where_none_found() {
        let amplitudes = vec![(0, 0.0), (1, 0.0), (2, 0.0)];
        assert_eq!(determine_autocrop_points(&amplitudes, 10), None);
    }

    #[test]
    fn test_autocrop_audio() {
        let mut audio = generate_audio(0.0, 5, 2, 1);
        audio.data[0] = vec![0.0, 1.0, 0.1, -1.0, 0.0];
        audio.data[1] = vec![0.0, -1.0, -0.1, 0.7, 0.0];
        autocrop_audio(&mut audio, Duration::from_secs(1), 20);
        assert_eq!(audio.data[0].len(), 3);
        assert_eq!(audio.data[1].len(), 3);
        assert_almost_eq_by_element(audio.data[0].clone(), vec![1.0, 0.1, -1.0]);
        assert_almost_eq_by_element(audio.data[1].clone(), vec![-1.0, -0.1, 0.7]);
    }
}
