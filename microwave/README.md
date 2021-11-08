Make xenharmonic music and explore musical tunings.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Scale expressions](https://github.com/Woyten/tune/blob/master/tune-cli/README.md)

# Overview

`microwave` is a microtonal modular waveform synthesizer with soundfont rendering capabilities based on:

- [tune](https://github.com/Woyten/tune) &ndash; a microtonal library
- [Nannou](https://nannou.cc/) &ndash; a UI framework
- [FluidLite](https://crates.io/crates/fluidlite) &ndash; a soundfont renderer
- [fluid-xenth](https://github.com/Woyten/tune/tree/master/fluid-xenth) &ndash; a microtonal wrapper around FluidLite

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, computer keyboard, MIDI keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

# Demo

- [Xênerie (15-EDO)](https://youtu.be/0PczKDrOdUA)
- [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)
- [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)

# Download / Installation

You can download a precompiled version of `microwave` from the [Releases](https://github.com/Woyten/tune/releases) section or you can build a fresh binary from scratch using Cargo:

```bash
cargo install -f microwave
```

To *build* `microwave` additional dev dependencies required by Nannou might need to be installed. On the CI environment (Ubuntu 20.04 LTS) the following installation step is sufficient:

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
name: Funky Clavinet
envelope: Piano
stages:
  - Oscillator:
      kind: Sin
      frequency: WaveformPitch
      modulation: None
      out_buffer: 0
      out_level: 440.0
  - Oscillator:
      kind: Triangle
      frequency: WaveformPitch
      modulation: ByFrequency
      mod_buffer: 0
      out_buffer: 1
      out_level: 1.0
  - Filter:
      kind: HighPass2
      resonance:
        Mul:
          - WaveformPitch
          - Envelope:
              name: Piano
              from: 2.0
              to: 4.0
      quality: 5.0
      in_buffer: 1
      out_buffer: AudioOut
      out_level: 1.0
```

This waveform has three stages:

1. Generate a sine wave with the waveform's nominal frequency *F* and an amplitude of 440. Write this waveform to buffer 0.
1. Generate a triangle wave with frequency *F* and an amplitude of 1.0. Modulate the waveform's frequency (in Hz) sample-wise by the amount stored in buffer 0. Write the modulated waveform to buffer 1.
1. Apply a second-order high-pass filter to the samples stored in buffer 1. The high-pass's resonance frequency is modulated by the envelope named `Piano` and ranges from 2*F* to 4*F*. Write the result to `AudioOut`.

To create your own waveforms use the default waveforms file as a starting point and try editing it by trial-and-error. Let `microwave`'s error messages guide you to find valid configurations.

## Live Interactions

You can live-control your waveforms with your mouse pointer or any MIDI Control Change messages source.

The following example stage defines a resonating low-pass filter whose resonance frequency can be controlled with a MIDI modulation wheel/lever from 0 to 10,000 Hz.

```yml
Filter:
  kind: LowPass2
  resonance:
    Control:
      controller: Modulation
      from: 0.0
      to: 10000.0
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0
```

# Feature List

- Sound features
  - Built-in modular waveform synhesizer with physical modeling synhesis
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
  - Channel events (pitch-bend, modulation, pedals, aftertouch, etc.)
  - Polyphonic events (key pressure)
  - LF sources (envelopes, time slices, oscillators, etc.)
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
  - SCL imports
  - KBM imports
  - Tuning-dependent automatic isomorphic keyboard layouts
  - Customizable second visual keyboard (`--kb2` option)
  - MIDI-out retuning via different tuning message types
  - Display frequencies and rational number approximations

# Help

For a complete list of command line options run

```bash
microwave help
```

# License

`microwave` statically links against [`fluidlite`](https://crates.io/crates/fluidlite) for soundfont renderering capabilities. This makes the *binary executable* of `microwave` a derivative  work of `fluidlite`. `fluidlite` is licensed under the *GNU Lesser General Public License, version 2.1*.