use anyhow::{bail, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fwatch::{BasicTarget, Transition, Watcher};
use libloading::{Library, Symbol};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use tempfile;

const WATCHER_POLL_DUR: Duration = Duration::from_millis(100);

pub fn hotswap(path: PathBuf) -> Result<Receiver<Library>> {
    let (sender, receiver) = unbounded::<Library>();

    attempt_lib_update(&path, &sender);

    let mut watcher: Watcher<BasicTarget> = Watcher::new();
    watcher.add_target(BasicTarget::new(&path));

    thread::spawn(move || loop {
        for event in watcher.watch() {
            match event {
                Transition::Modified => attempt_lib_update(&path, &sender),
                _ => {}
            }
        }
        thread::sleep(WATCHER_POLL_DUR);
    });

    Ok(receiver)
}

fn attempt_lib_update(src_path: &Path, lib_sender: &Sender<Library>) {
    let library = match compile(&src_path) {
        Ok(lib) => lib,
        Err(_e) => {
            warn!("Failed to compile library for file {:?}", &src_path);
            return;
        }
    };
    match lib_sender.send(library) {
        Ok(_) => (),
        Err(_) => trace!(
            "Failed to send library down channel for file {:?}",
            &src_path
        ),
    }
}

fn compile(path: &Path) -> Result<Library> {
    if cfg!(target_os = "windows") {
        // this definitely _can_ be done, but the code would be different here
        // and I don't have a windows machine to develop on
        panic!("hotswapping is not supported on windows");
    }
    let build_target = tempfile::Builder::new().suffix(".so").tempfile()?;
    let build_target_path = build_target.path().as_os_str();
    let compile_output = Command::new("rustc")
        .arg("--color")
        .arg("always")
        .arg("-A")
        .arg("warnings")
        .arg("--codegen")
        .arg("opt-level=3")
        .arg("--crate-type")
        .arg("dylib")
        .arg(path)
        .arg("-o")
        .arg(build_target_path)
        .output()?;
    if !compile_output.stderr.is_empty() {
        println!("=========================================================");
        println!("================rust compilation failed==================");
        println!("=========================================================");
        println!("{}", String::from_utf8(compile_output.stderr)?);
        println!("=========================================================");
        println!("================end of compiler output===================");
        println!("=========================================================");
    }
    if !compile_output.status.success() {
        bail!("rustc compilation failed");
    }
    Ok(unsafe { Library::new(&build_target_path)? })
}

pub fn load_fn<'lib, T>(library: &'lib Library, symbol: &[u8]) -> Result<Symbol<'lib, T>> {
    unsafe { Ok(library.get(CString::new(symbol)?.as_bytes())?) }
}
