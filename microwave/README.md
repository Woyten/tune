Make xenharmonic music and explore musical tunings.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Scale expressions](https://crates.io/crates/tune-cli)

# Overview

`microwave` is a microtonal modular waveform synthesizer based on:

- [tune](https://crates.io/crates/tune) &ndash; a microtonal library
- [Nannou](https://nannou.cc/) &ndash; a UI framework
- [FluidLite](https://crates.io/crates/fluidlite) &ndash; a soundfont renderer

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, computer keyboard, MIDI keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

# Installation

```bash
cargo install -f microwave
```

To *install* `microwave` (build it from scratch) additional dev dependencies required by Nannou might need to be installed. On the CI environment (Ubuntu 18.04 LTS) the following installation step is sufficient:

```bash
sudo apt install libxcb-composite0-dev libasound2-dev
```

To *run* `microwave` you need the appropriate runtime libraries for your graphics card. For me (Ubuntu 18.04 LTS) the following step worked:

```bash
sudo apt install libvulkan1 mesa-vulkan-drivers vulkan-utils
```

If this doesn't help or you don't use Ubuntu/`apt` try following these [instructions](https://guide.nannou.cc/getting_started/platform-specific_setup.html).

# Usage

```bash
microwave run                     # 12-EDO scale (default)
microwave run steps 1:22:2        # 22-EDO scale
microwave run import my_scale.scl # imported scale
```

This should spawn a window displaying a virtual keyboard. Use your touch screen, computer keyboard or mouse to play melodies on the virtual piano.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)

## Soundfont Files

For playback of sampled sounds you need to provide the location of a soundfont file. The location can be set via the environment variable `MICROWAVE_SF_LOC` or the command line:

```bash
microwave run --sf-loc /usr/share/sounds/sf2/default-GM.sf2 steps 1:22:2
```

If you like to use compressed sf3 files you need to compile `microwave` with the `sf3` feature enabled. Note that the startup will take significantly longer since the soundfont needs to be decompressed first.

## Modular Synth &ndash; Create Your Own Waveforms

On startup, `microwave` tries to locate a waveforms file specified by the `--wv-loc` parameter or the `MICROWAVE_WV_LOC` environment variable. If no such file is found `microwave` will create a default waveforms file for you.

Let's have a look at an example clavinettish sounding waveform that I discovered by accident:

```yml
name: Clavinet
envelope_type: Piano
stages:
  - Oscillator:
      kind: Sin
      frequency: WaveformPitch
      modulation: None
      destination:
        buffer: Buffer0
        intensity: 440.0
  - Oscillator:
      kind: Triangle
      frequency: WaveformPitch
      modulation:
        ByFrequency: Buffer0
      destination:
        buffer: AudioOut
        intensity: 1.0
```

This waveform has two stages:

1. Generate a sine wave with the waveform's nominal frequency and an amplitude of 440. Write this waveform to `Buffer0`.
1. Generate a triangle wave with the waveform's nominal frequency and an amplitude of 1.0. Modulate the waveform's frequency (in Hz) sample-wise by the amount stored in `Buffer0`. Write the modulated waveform to `AudioOut`.

To create your own waveforms edit the waveforms file by trial-and-error. Let `microwave`'s error messages guide you to find valid configurations.

## Live Interactions

You can live-control your waveforms with your mouse pointer or any MIDI Control Change messages source.

The following example stage defines a resonating low-pass filter whose resonance frequency can be controlled with a MIDI modulation wheel/lever from 0 to 10,000 Hz.

```yml
Filter:
  kind: LowPass2
  resonance:
    Controller:
      controller: Modulation
      from: 0.0
      to: 10000.0
  quality: 5.0
  source: Buffer0
  destination:
    buffer: AudioOut
    intensity: 1.0
```

# Feature List

- Sound features
  - Built-in modular waveform synhesizer
    ```bash
    microwave run --wv-loc <waveforms-file-location> [scale-expression]
    ```
  - FluidLite soundfont renderer
    ```bash
    microwave run --sf-loc <soundfont-file-location> [scale-expression]
    ```
  - External synthesizer via MIDI-out
    ```bash
    microwave run --midi-out <midi-target> [scale-expression]
    ```
  - Microphone / aux input
    ```bash
    microwave run --audio-in [scale-expression]
    ```
  - WAV recording
- Control features
  - Sequencer / piano keyboard via MIDI-in
    ```bash
    microwave run --midi-in <midi-source> [scale-expression]
    ```
  - Computer keyboard (configurable isomorphic layout)
  - Touch Screen
  - Mouse
  - LF sources, e.g. time slices and oscillators
- Effects
  - Low-pass
  - 2nd order low-pass
  - High-pass
  - 2nd order high-pass
  - Band-pass
  - Notch filter
  - All-pass
  - Reverb
  - Spatial delay
  - Rotary speaker
- Microtuning features
  - Custom scales
  - SCL import
  - Tuning-dependent automatic keyboard layout
  - MIDI-out retuning via Single Note Tuning messages
  - Display frequencies and rational number approximations

# Help

For a complete list of command line options run

```bash
microwave help
```
