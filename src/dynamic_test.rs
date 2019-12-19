use anyhow::Result;
use libloading::{Library, Symbol};
use rocoder::hotswapper;
use rocoder::runtime_setup;
use std::env;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use stopwatch::Stopwatch;

fn main() -> Result<()> {
    runtime_setup::setup_logging();
    let path = PathBuf::from("/home/ayoon/projects/rocoder/hotswap_sample.rs");

    let lib_receiver = hotswapper::hotswap(path)?;

    for lib in lib_receiver {
        unsafe {
            let sw = Stopwatch::start_new();
            let symbol: Symbol<fn(Vec<f32>) -> Vec<f32>> = lib.get(b"test\0").unwrap();
            println!("loaded symbol in {:?}", sw.elapsed());
            let sw = Stopwatch::start_new();
            println!("{:?}", symbol(vec![1.0, 2.0]));
            println!("executed in {:?}", sw.elapsed());
        }
    }

    Ok(())
}
