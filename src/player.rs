use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeOutputBuffer,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use pbr::ProgressBar;

use crate::audio::AudioSpec;

const PLAYBACK_SLEEP: Duration = Duration::from_millis(250);

/// Simple audio playback

pub fn play_audio(audio_spec: &AudioSpec, audio_channels: Vec<Vec<f32>>) {
    let samples_dur = audio_channels.get(0).unwrap().len();
    let playback_position: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let cloned_playback_position = Arc::clone(&playback_position);

    let host = cpal::default_host();
    let event_loop = Arc::new(host.event_loop());
    let event_loop_arc_for_run = Arc::clone(&event_loop);
    let output_device = host
        .default_output_device()
        .expect("failed to get default output device");
    println!(
        "Using default output device: \"{}\"",
        output_device.name().unwrap()
    );

    let format = Format {
        channels: audio_spec.channels,
        sample_rate: SampleRate(audio_spec.sample_rate),
        data_type: SampleFormat::F32,
    };

    let output_stream_id = event_loop
        .build_output_stream(&output_device, &format)
        .unwrap();

    event_loop.play_stream(output_stream_id.clone()).unwrap();

    thread::spawn(move || {
        event_loop_arc_for_run.run(move |_stream_id, stream_data| {
            let mut buffer = match stream_data {
                Ok(res) => match res {
                    StreamData::Output {
                        buffer: UnknownTypeOutputBuffer::F32(buffer),
                    } => buffer,
                    _ => panic!("unexpected buffer type"),
                },
                Err(e) => {
                    panic!("failed to fetch get audio stream: {:?}", e);
                }
            };
            let mut playback_pos = cloned_playback_position.lock().unwrap();

            for buffer_interleaved_samples in buffer.chunks_mut(format.channels as usize) {
                for (dest, src_channel) in
                    buffer_interleaved_samples.iter_mut().zip(&audio_channels)
                {
                    match src_channel.get(*playback_pos) {
                        Some(sample) => *dest = *sample,
                        None => {
                            *dest = 0.0;
                        }
                    }
                }
                *playback_pos += 1;
            }
        });
    });

    let mut progress_bar = playback_progress_bar();
    loop {
        let current_playback_position = *playback_position.lock().unwrap();
        if current_playback_position >= samples_dur {
            break;
        }
        progress_bar.set(((current_playback_position as f32 / samples_dur as f32) * 100.0) as u64);
        progress_bar.tick();
        thread::sleep(PLAYBACK_SLEEP);
    }
    progress_bar.finish();
    println!("\nplayback complete");
    event_loop.destroy_stream(output_stream_id);
}

fn playback_progress_bar() -> ProgressBar<std::io::Stdout> {
    let mut progress_bar = ProgressBar::new(100);
    progress_bar.show_speed = false;
    progress_bar.show_counter = false;
    progress_bar.tick_format("▁▂▃▄▅▆▇█▇▆▅▄▃");
    progress_bar
}
