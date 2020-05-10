Make xenharmonic music and explore musical tunings.

# Overview

`microwave` is a microtonal waveform synthesizer based on the microtonal [tune](https://crates.io/crates/tune) library and the [Nannou](https://nannou.cc/) UI framework.

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

You can run `microwave` in continous or scale-based mode:

```bash
microwave              # Continuous
microwave equal 1:22:2 # 22-EDO scale
```

The command should spawn a a window showing a virtual keyboard.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)

Use your touch screen or mouse to play melodies on the virtual piano. At present, polyphonic melodies can only be played via the touch screen.

To see how scale expressions work, visit [tune](https://crates.io/crates/tune).




