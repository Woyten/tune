Make xenharmonic music and explore musical tunings.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Scale expressions](https://crates.io/crates/tune-cli)

# Overview

`microwave` is a microtonal waveform synthesizer based on:

- [tune](https://crates.io/crates/tune) &ndash; a microtonal library
- [Nannou](https://nannou.cc/) &ndash; a UI framework
- [FluidLite](https://crates.io/crates/fluidlite) &ndash; a soundfont renderer

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, computer keyboard, MIDI keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

# Installation

```bash
cargo install -f microwave
```

You might need to install additional dependencies required by Nannou. For me, the following setup worked:

```bash
sudo apt install pkg-config libx11-dev
```

If this doesn't help or you don't use `apt`, try following these [instructions](https://guide.nannou.cc/getting_started/platform-specific_setup.html).

# Usage

```bash
microwave              # 12-EDO scale (default)
microwave equal 1:22:2 # 22-EDO scale
```

This should spawn a window displaying a virtual keyboard. Use your touch screen, computer keyboard or mouse to play melodies on the virtual piano.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)


## Soundfont Files

For playback of sampled sounds you need to provide the location of a soundfont file. The location can be set via the environment variable `MICROWAVE_SF` or the command line:

```bash
microwave --sf /usr/share/sounds/sf2/default-GM.sf2 equal 1:22:2
```

If you like to use compressed sf3 files you need to compile `microwave` with the `sf3` feature enabled. Note that the startup will take significantly longer since the soundfont needs to be decompressed first.

## MIDI Input

To use a MIDI device as an input source, use the `--ms` option:

```bash
microwave --ms 1 equal 1:22:2
```

## More Options

For a complete list of command line options run

```bash
microwave help
```