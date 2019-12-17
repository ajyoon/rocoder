use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use libloading::{Library, Symbol};
use notify::{immediate_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::ffi::CString;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn hotswap(path: String, name: String) -> Result<Receiver<Library>> {
    let (sender, receiver) = unbounded::<Library>();

    let (watcher_tx, watcher_rx) = unbounded();
    let mut watcher = immediate_watcher(watcher_tx)?;
    watcher.watch(path.clone(), RecursiveMode::NonRecursive)?;

    loop {
        match watcher_rx.recv() {
            Ok(event) => {
                let library = Library::new(&path).unwrap();
                sender.send(library);
            }
            Err(err) => {
                error!("error while watching file {}: {:?}", &path, err);
            }
        }
    }

    Ok(receiver)
}

pub fn load_fn<'lib, T>(library: &'lib Library, symbol: &[u8]) -> Result<Symbol<'lib, T>> {
    unsafe { Ok(library.get(CString::new(symbol)?.as_bytes())?) }
}
