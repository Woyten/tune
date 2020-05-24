Make xenharmonic music and explore musical tunings.

# Overview

`microwave` is a microtonal waveform synthesizer based on:

- [tune](https://crates.io/crates/tune) &ndash; a microtonal library
- [Nannou](https://nannou.cc/) &ndash; a UI framework
- [FluidLite](https://crates.io/crates/fluidlite) &ndash; a soundfont renderer

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

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
microwave -s /usr/share/sounds/sf2/FluidR3_GM.sf2 equal 1:22:2
```

The command should spawn a a window showing a virtual keyboard.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)

Use your touch screen or mouse to play melodies on the virtual piano. At present, polyphonic melodies can only be played via the touch screen.

To see how scale expressions work, visit [tune](https://crates.io/crates/tune-cli).




