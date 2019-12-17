use libloading::{Library, Symbol};
use std::sync::mpsc::{channel, Receiver, Sender};

fn main() {
    unsafe {
        let library = Library::new("libhello.so").unwrap();
        let symbol: Symbol<unsafe extern "C" fn() -> String> = library.get(b"foo\0").unwrap();
        println!("{}", symbol());
    }
}
