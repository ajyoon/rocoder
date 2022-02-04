# Rocoder

A live-codeable phase vocoder written in Rust.

Rocoder is a digital instrument command line program that transforms audio by slowing or speeding it and applying live-codeable frequency kernels. It can:

- Change audio speed
- Pitch shift (though only along harmonics and subharmonics)
- Apply arbitrary code transformations to frequency-domain audio representations (Mac and Linux only)
- Live compile and reload transformation code
- Read and write audio files
- Do direct audio input and playback

## Installing

1. If you don't have it, install a Rust toolchain at https://rustup.rs/
2. Install this tool by running `cargo install rocoder`
3. Run `rocoder -h` to get started!

## How it works

The rocoder is a fairly naive, and probably not quite correct, [phase vocoder](https://en.wikipedia.org/wiki/Phase_vocoder). It processes audio using a 3 step process, and understanding the basics is necessary for advanced use, especially working with frequency kernels.

1. **Analysis**: Input audio is analyzed in overlapping [Hanning windows](https://en.wikipedia.org/wiki/Hann_function) using [FFTs](https://en.wikipedia.org/wiki/Fast_Fourier_transform). Each window analysis emits a frequency domain representation of its audio window, encoded as a buffer of complex numbers.
2. **Processing**: If a frequency kernel is provided, each frequency domain window buffer is then passed to it for arbitrary code-defined processing.
3. **Resynthesis**: The processed frequency domain buffers are then passed to an inverse FFT to be resynthesized. Resynthesis is done in such a way to allow both pitch shifting and speed changing.

## Usage

The rocoder is controlled using a series of command line arguments. Run `rocoder -h` to list them.

### `-r`, `--record`

Get audio input from your default audio input device. When set, the rocoder will start by recording input until you press Enter. It will automatically attempt to trim the audio start/end to cut out dead noise.

### `--rotate-channels`

Rotate the input audio channels by 1. For stereo input this swaps left and right channels.

### `-a`, `--amplitude` `<amplitude>`

An output amplitude multiplier. Defaults to `1`;

### `-b`, `--buffer` `<buffer-dur>`

The maximum duration of audio to process ahead of time. This is mostly useful if you want to alter the response time of live code changes. The value is specified in seconds, e.g. `-b 1.5` for 1.5 seconds.

### `-d`, `--duration` `<duration>`

The amount of audio to read from the input source, starting from the starting time if provided. Specified as a duration string `hh:mm:ss.ss` where larger divisions may be omitted, e.g. `1:0:0` for 1 hour, `1:30` for 90 seconds, `1.5` for 1.5 seconds.

### `-f`, `--factor` `<factor>`

The stretch factor; e.g. 5 to slow 5x and 0.2 to speed up 5x. Defaults to `1` (no speed change).

### `-x`, `--fade` `<fade>`

Duration of a fade in/out to apply to the output audio. See `--duration` for specification format. Defaults to `1` (1 second).

### `--freq-kernel` `<freq-kernel>`

Path to a rust frequency kernel.

### `-i`, `--input` `<input>`

Path to an audio file to read from. Currently supports `.wav` (8, 16, 24, 32 bit integer and 32 bit float formats) and `.mp3`.

### `-o`, `--output` `<output>`

Path to an audio output file. If set, output is not played to a device; instead the rocoder will run as fast as possible and persist the output to disk.

This only supports `.wav` output in 32-bit float format.

Due to a hacky implementation, this requires the entire output file fits in memory before being written to disk.

### `-p`, `--pitch-multiple` `<pitch-multiple>`

A non-zero integer pitch multiplier. Positive numbers above 1 are used to pitch shift along the [harmonic series](https://en.wikipedia.org/wiki/Harmonic_series_(music)), while negative numbers below -1 are used to shift along the [subharmonic series](https://en.wikipedia.org/wiki/Undertone_series).

| arg  | result                         |
|------|--------------------------------|
| `1`  | No pitch shift                 |
| `2`  | Up an octave                   |
| `3`  | Up an octave and a fifth       |
| `4`  | Up 2 octaves                   |
| `5`  | up 2 octaves and a major third |
| `-1` | No pitch shift                 |
| `-2` | Down an octave                 |
| `-3` | Down and octave and a fifth    |
| `0`  | [Invalid]                      |

Defaults to `1` (no pitch shift).

### `-s`, `--start` `<start>`

Start time in the input audio. (See `--duration` for argument format)

### `-w`, `--window` `<window-len>`

The size of processing windows. Small values cause distortion while large values cause smearing. Powers of 2 are recommended for optimal performance. Defaults to `16384`.

## Live coding

**Frequency kernels are only supported on Mac and Linux. Contributions to support Windows are welcome.**

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
cargo run --release -- \
    -r -f 1 --freq-kernel path/to/kernel.rs
```

When the rocoder is running live and playing audio back (not writing to a file), it will watch this file for changes and automatically compile and hotswap it into the process on the fly. Simply edit the file and save to live code on your kernel!

## The library

Various pieces of functionality from this tool are exposed in a crate library, but this API is currently undocumented and very unstable.

## Credits

The basic implementation of the phase vocoder algorithm is been adapted from [Paulstretch](https://github.com/paulnasca/paulstretch_python).
