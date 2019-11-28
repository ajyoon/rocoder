use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeOutputBuffer,
};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::audio::AudioSpec;

/// Simple audio playback

pub fn play_audio(audio_spec: &AudioSpec, mut audio_channels: Vec<Vec<f32>>) {
    let expected_dur_micros = ((audio_channels.get(0).unwrap().len() as f32
        / audio_spec.sample_rate as f32)
        * 1_000_000.0) as u64;
    let playback_position: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let cloned_playback_position = Arc::clone(&playback_position);

    let host = cpal::default_host();
    let event_loop = host.event_loop();
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
    let sample_width = format.data_type.sample_size();

    let output_stream_id = event_loop
        .build_output_stream(&output_device, &format)
        .unwrap();

    event_loop.play_stream(output_stream_id.clone()).unwrap();

    thread::spawn(move || {
        event_loop.run(move |_stream_id, stream_data| {
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
            let samples_needed = buffer.len() / format.channels as usize;

            for buffer_interleaved_samples in buffer.chunks_mut(format.channels as usize) {
                for (dest, src_channel) in
                    buffer_interleaved_samples.iter_mut().zip(&audio_channels)
                {
                    *dest = unsafe { *src_channel.get_unchecked(*playback_pos) };
                }
                *playback_pos += 1;
            }

            // for (audio_channel_buffer, channel_output_buffer) in audio_channels
            //     .iter_mut()
            //     .zip(buffer.chunks_mut(format.channels as usize))
            // {

            //     // let bytes = samples_needed * sample_width;
            //     // let src_ptr =
            //     //     (&audio_channel_buffer[*playback_pos..]).as_ptr() as *mut libc::c_void;
            //     // let write_ptr = channel_output_buffer.as_ptr() as *mut libc::c_void;
            //     // unsafe {
            //     //     libc::memcpy(write_ptr, src_ptr, bytes);
            //     // }
            // }
        });
    });

    // super hacky "wait till playback is done"

    thread::sleep(Duration::from_micros(expected_dur_micros));
}
