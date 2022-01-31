# Rocoder

A live-codeable phase vocoder written in Rust.

Rocoder is a digital instrument command line program that transforms audio by slowing or speeding it and applying live-codeable frequency kernels. It can:

- Change audio speed
- Pitch shift (though only along harmonics and subharmonics)
- Apply arbitrary code transformations to frequency-domain audio representations
- Live compile and reload transformation code
- Read and ~~write~~ audio files [audio output temporarily broken]
- Do direct audio input and playback

## Installing

1. If you don't have it, install a Rust toolchain at https://rustup.rs/
2. Download or clone this repository
3. From within the project, run `cargo run --release --bin rocoder -- -h` to compile and print the help dialog.

## Usage

The rocoder is controlled using a series of command line arguments.

## How it works

The rocoder is a fairly naive, and probably not quite correct, [phase vocoder](https://en.wikipedia.org/wiki/Phase_vocoder). It processes audio using a 3 step process, and understanding the basics is necessary for advanced use, especially working with frequency kernels.

1. **Analysis**: Input audio is analyzed in overlapping [Hanning windows](https://en.wikipedia.org/wiki/Hann_function) using [FFTs](https://en.wikipedia.org/wiki/Fast_Fourier_transform). Each window analysis emits a frequency domain representation of its audio window, encoded as a buffer of complex numbers.
2. **Processing**: If a frequency kernel is provided, each frequency domain window buffer is then passed to it for arbitrary code-defined processing.
3. **Resynthesis**: The processed frequency domain buffers are then passed to an inverse FFT to be resynthesized. Resynthesis is done in such a way to allow both pitch shifting and speed changing.

## Live coding

Frequency kernels modify frequency-domain data before resynthesis, allowing you to perform very powerful transformations on your sounds. Kernels are defined in Rust files and must conform to the following signature:

```rs
#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    todo!() // Your code here
}
```

Both the `no_mangle` directive and the name `apply` are required.

The input is a buffer of complex numbers representing the frequency domain of a given audio window. By default, windows are ~16k samples long. The function's output is a transformed copy of the input.

Here is a simple kernel which simply increases the amplitude of the input audio by multiplying the input by a constant:

```rs
#[no_mangle]
pub fn apply(elapsed_ms: usize, input: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    return input
        .iter()
        .map(|(real, im)| (real * 2.0, im * 2.0))
        .collect();
}
```

If this is saved in a file `kernel.rs`, it can be used with:

```sh
cargo run --release --bin rocoder -- \
    -r -f 1 --freq-kernel path/to/kernel.rs
```

## The library

Various pieces of functionality from this tool are exposed in a crate library, but this API is highly unstable and expected to have major breaking changes soon.

## Credits

The basic implementation of the phase vocoder algorithm is been adapted from [Paulstretch](https://github.com/paulnasca/paulstretch_python).
