Make xenharmonic music and explore musical tunings.

# Overview

`microwave` is a microtonal waveform synthesizer based on:

- [tune](https://crates.io/crates/tune) &ndash; a microtonal library
- [Nannou](https://nannou.cc/) &ndash; a UI framework
- [FluidLite](https://crates.io/crates/fluidlite) &ndash; a soundfont renderer

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

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

You can run `microwave` in continuous or scale-based mode:

```bash
microwave              # Continuous
microwave equal 1:22:2 # 22-EDO scale
```

For playback of sampled sounds provide the location of a soundfont file:

```bash
microwave --sf /usr/share/sounds/sf2/default-GM.sf2 equal 1:22:2
```

If you want to load compressed sf3 files you need to enable the `sf3` feature. Note that the startup will be a lot slower since the sondfont file needs to be decompressed first.

After everything has been loaded you should see a window showing a virtual keyboard.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)

Use your touch screen, keyboard or mouse to play melodies on the virtual piano.

To see how scale expressions work, visit [tune](https://crates.io/crates/tune-cli).




