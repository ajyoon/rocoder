use cpal::{
    self,
    traits::{DeviceTrait, EventLoopTrait, HostTrait},
    Format, SampleFormat, SampleRate, StreamData, UnknownTypeOutputBuffer,
};
use crossbeam_channel::{Receiver, Sender};
use ctrlc;
use pbr::ProgressBar;
use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;

use crate::audio::{Audio, AudioSpec, Sample};
use crate::mixer::{Mixer, MixerState};

const PLAYBACK_SLEEP: Duration = Duration::from_millis(250);
const QUIT_FADE: Duration = Duration::from_secs(5);

/// Simple audio playback

pub fn play_audio<T>(
    spec: AudioSpec,
    stream: Receiver<Audio<T>>,
    expected_total_samples: Option<usize>,
) where
    T: Sample,
{
    let format = Format {
        channels: spec.channels,
        sample_rate: SampleRate(spec.sample_rate),
        data_type: SampleFormat::F32,
    };

    let mixer_arc = Arc::new(Mutex::new(Mixer::new(
        &spec,
        stream,
        expected_total_samples,
    )));
    let mixer_arc_for_run = Arc::clone(&mixer_arc);

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

    let output_stream_id = event_loop
        .build_output_stream(&output_device, &format)
        .unwrap();

    event_loop.play_stream(output_stream_id.clone()).unwrap();

    launch_cpal_thread(event_loop_arc_for_run, mixer_arc_for_run);

    wait_for_playback(mixer_arc, event_loop, output_stream_id);
}

fn playback_progress_bar() -> ProgressBar<std::io::Stdout> {
    let mut progress_bar = ProgressBar::new(100);
    progress_bar.show_speed = false;
    progress_bar.show_counter = false;
    progress_bar.tick_format("▁▂▃▄▅▆▇█▇▆▅▄▃");
    progress_bar
}

fn launch_cpal_thread<T, E>(event_loop: Arc<E>, mixer_arc: Arc<Mutex<Mixer<T>>>)
where
    T: Sample,
    E: EventLoopTrait + Send + Sync + 'static,
{
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

            let mut mixer = mixer_arc.lock().unwrap();
            mixer.fill_buffer(&mut buffer);
        });
    });
}

fn control_c_handler<T>(quit_counter: &Arc<AtomicU16>, mixer_arc: &Arc<Mutex<Mixer<T>>>)
where
    T: Sample,
{
    if quit_counter.fetch_add(1, Ordering::SeqCst) > 0 {
        // If ctrl-c was received more than once, quit without fading out
        println!("\nExiting immediately");
        return;
    }
    println!("\nGot quit signal, fading out audio for {:#?}", QUIT_FADE);
    // nb fade doesnt work with streaming model yet
    // let mut audio = audio_arc.lock().unwrap();
    // let fade_out_start = audio.sample_to_duration(total_playback_pos.load(Ordering::SeqCst));
    // audio.fade_out(fade_out_start, QUIT_FADE);
    // drop(audio);
    let quit_counter_2 = Arc::clone(&quit_counter);
    thread::spawn(move || {
        thread::sleep(QUIT_FADE + Duration::from_millis(50));
        quit_counter_2.fetch_add(1, Ordering::SeqCst);
    });
}

fn wait_for_playback<T, E>(
    mixer_arc: Arc<Mutex<Mixer<T>>>,
    event_loop: Arc<E>,
    output_stream_id: <E>::StreamId,
) where
    T: Sample,
    E: EventLoopTrait,
{
    // On early quit, fade out the sound before quitting
    let quit_counter = Arc::new(AtomicU16::new(0));
    let quit_counter_clone = Arc::clone(&quit_counter);
    let mixer_arc_ctrlc_clone = Arc::clone(&mixer_arc);
    ctrlc::set_handler(move || {
        control_c_handler(&quit_counter_clone, &mixer_arc_ctrlc_clone);
    })
    .unwrap();

    // Manage progress bar and wait for playback to complete
    let mut progress_bar = playback_progress_bar();
    loop {
        // get all data from mixer (under mutex) at once
        let mixer = mixer_arc.lock().unwrap();
        let mixer_state = mixer.state;
        let progress = mixer.progress();
        drop(mixer);

        if mixer_state == MixerState::DONE {
            progress_bar.finish();
            println!("\nplayback complete");
            break;
        } else if quit_counter.load(Ordering::SeqCst) > 1 {
            progress_bar.finish();
            println!("\nplayback aborted");
            // need to explicitly exit with a non-zero exit code so the control-c quit
            // makes it to the shell so, for instance, bash loops can be broken.
            std::process::exit(1);
        }
        match progress {
            Some(percent) => {
                progress_bar.set(percent as u64);
            }
            None => {}
        }
        progress_bar.tick();
        thread::sleep(PLAYBACK_SLEEP);
    }
    event_loop.destroy_stream(output_stream_id);
}
