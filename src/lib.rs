#![feature(test)]
#![feature(div_duration)]
#![feature(drain_filter)]

#[macro_use]
extern crate log;

mod crossfade;
mod mixer;
mod power;
mod resampler;
mod slices;
mod test_utils;

pub mod audio;
pub mod audio_files;
pub mod duration_parser;
pub mod fft;
pub mod hotswapper;
pub mod math;
pub mod player_processor;
pub mod recorder;
pub mod recorder_processor;
pub mod runtime_setup;
pub mod signal_flow;
pub mod stretcher;
pub mod stretcher_processor;
pub mod windows;
