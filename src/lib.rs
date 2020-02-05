#![feature(test)]

#[macro_use]
extern crate log;

mod crossfade;
mod mixer;
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
pub mod signal_flow;
//pub mod player;
pub mod recorder;
pub mod runtime_setup;
pub mod stretcher;
pub mod windows;
