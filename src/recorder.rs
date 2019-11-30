use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeInputBuffer,
};
use num_traits::Num;
use std::io;
use std::ops::MulAssign;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use crate::audio::{Audio, AudioSpec};

/// Simple audio recording

pub fn record_audio(audio_spec: &AudioSpec) -> Audio<f32> {
    wait_for_enter_keypress("Press ENTER to start recording");

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
    thread::spawn(move || {
        event_loop_arc_for_run.run(move |_stream_id, stream_data| {
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

    wait_for_enter_keypress("Press ENTER to finish recording");
    event_loop.destroy_stream(input_stream_id);
    collect_samples(audio_spec, raw_samples_receiver)
}

fn collect_samples<T>(spec: &AudioSpec, raw_samples_receiver: mpsc::Receiver<T>) -> Audio<T>
where
    T: Sized + Num + Copy + MulAssign,
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
