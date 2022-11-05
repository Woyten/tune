Make xenharmonic music and explore musical tunings.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Scale expressions](https://github.com/Woyten/tune/blob/master/tune-cli/README.md)

# Overview

`microwave` is a microtonal modular waveform synthesizer with soundfont rendering capabilities based on:

- [tune](https://github.com/Woyten/tune) &ndash; a microtonal library
- [magnetron](https://github.com/Woyten/tune/tree/master/magnetron) &ndash; a modular synthesizer architecture
- [fluid-xenth](https://github.com/Woyten/tune/tree/master/fluid-xenth) &ndash; a microtonal soundfont renderer
- [Nannou](https://nannou.cc/) &ndash; a UI framework

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, computer keyboard, MIDI keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

# Demo

- [Xênerie (15-EDO)](https://youtu.be/0PczKDrOdUA)
- [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)
- [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)

# Download / Installation

Option A: Try out the web app to get a very first impression:

- [microwave (Browser)](https://woyten.github.io/microwave) - Very experimental!

Option B: Download a precompiled version of `microwave` for the supported target architectures:

- [microwave 0.31.0 (Linux)](https://github.com/Woyten/tune/releases/download/microwave-0.31.0/microwave-0.31.0-x86_64-unknown-linux-gnu.zip)
- [microwave 0.31.0 (Windows)](https://github.com/Woyten/tune/releases/download/microwave-0.31.0/microwave-0.31.0-x86_64-pc-windows-msvc.zip)
- [microwave 0.31.0 (macOS)](https://github.com/Woyten/tune/releases/download/microwave-0.31.0/microwave-0.31.0-x86_64-apple-darwin.zip)

Option C: Use Cargo to build a fresh binary from scratch for your own target architecture:

```bash
# If you are using Linux: Make sure all dev dependencies are installed.
# On the CI environment (Ubuntu 20.04) we only need to add one library:
sudo apt install libasound2-dev

# Make sure pkg-config is installed
sudo apt install pkg-config

cargo install -f microwave
```

`microwave` should run out-of-the box on a recent (Ubuntu) Linux, Windows or macOS installation. If it doesn't, the problem is probably caused by the Nannou framework. In that case, try following these [instructions](https://guide.nannou.cc/getting_started/platform-specific_setup.html).

# Usage

Hint: Run `microwave` with parameters from a shell environment (Bash, PowerShell). Double-clicking on the executable will not provide access to all features!

```bash
microwave run                       # 12-EDO scale (default)
microwave run steps 1:22:2          # 22-EDO scale
microwave run scl-file my_scale.scl # imported scale
microwave run help                  # Print help about how to set the parameters to start microwave
```

This should spawn a window displaying a virtual keyboard. Use your touch screen, computer keyboard or mouse to play melodies on the virtual piano.

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot.png)

## MIDI In/Out

To enable playback via an external MIDI device you need to specify the name of the output device and a tuning method. The available tuning methods are `full`, `full-rt`, `octave-1`, `octave-1-rt`, `octave-2`, `octave-2-rt`, `fine-tuning` and `pitch-bend`.

```bash
microwave devices # List MIDI devices
microwave run --midi-out name-of-my-device --tun-method octave-1
microwave run --midi-in "name of my device" --tun-method octave-1 # If the device name contains spaces
```

To listen for events coming from a external MIDI device you only need to specify the name of the input device:

```bash
microwave devices # List MIDI devices
microwave run --midi-in name-of-my-device
microwave run --midi-in "name of my device" # If the device name contains spaces
```

## Soundfont Files

For playback of sampled sounds you need to provide the location of a soundfont file. The location can be set via the environment variable `MICROWAVE_SF_LOC` or the command line:

```bash
microwave run --sf-loc /usr/share/sounds/sf2/default-GM.sf2 steps 1:22:2
```

If you like to use compressed sf3 files you need to compile `microwave` with the `sf3` feature enabled. Note that the startup will take significantly longer since the soundfont needs to be decompressed first.

## Audio Options

The command-line enables you to set set up sample rates, buffer sizes and many other audio parameters. To print a full list of available options run:

```bash
microwave run help
```

## Modular Synth &ndash; Create Your Own Waveforms and Effects

On startup, `microwave` tries to locate a config file specified by the `--cfg-loc` parameter or the `MICROWAVE_CFG_LOC` environment variable. If no such file is found `microwave` will create a default config file with predefined waveforms and effects for you.

### `waveforms` section

The `waveforms` section of the config file defines waveform render stages to be applied sequentially when a key is pressed.

You can combine those stages to create the tailored sound you wish for. The following example config defines a clavinettish sounding waveform that I discovered by accident:

```yml
waveforms:
  - name: Funky Clavinet
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
              - Time:
                  start: 0.0
                  end: 0.1
                  from: 2.0
                  to: 4.0
          quality: 5.0
          in_buffer: 1
          out_buffer: AudioOut
          out_level: 1.0
```

While rendering the sound three stages are applied:

1. Generate a sine wave with the waveform's nominal frequency *F* and an amplitude of 440. Write this waveform to buffer 0.
1. Generate a triangle wave with frequency *F* and an amplitude of 1.0. Modulate the waveform's frequency (in Hz) sample-wise by the amount stored in buffer 0. Write the modulated waveform to buffer 1.
1. Apply a second-order high-pass filter to the samples stored in buffer 1. The high-pass's resonance frequency rises from 2*F* to 4*F* within 0.1 seconds. Write the result to `AudioOut`.

To create your own waveforms use the default config file as a starting point and try editing it by trial-and-error. Let `microwave`'s error messages guide you to find valid configurations.

### `effects` section

The `effects` section of the config file defines the effects to be applied sequentially after the waveforms have been rendered.

Use the following config as an example to add a rotary-speaker effect to your sound.

```yml
effects:
  - RotarySpeaker:
      buffer_size: 100000
      gain:
        Controller:
          kind: Sound9
          from: 0.0
          to: 0.5
      rotation_radius: 20.0
      speed:
        Controller:
          kind: Sound10
          from: 1.0
          to: 7.0
      acceleration: 7.0
      deceleration: 14.0
```

The given config defines the following properties:

- A fixed delay buffer size of 100000 samples
- An input gain ranging from 0.0 to 0.5. The input gain can be controlled via the F9 key or MIDI CCN 78.
- A rotation radius of 20 cm
- A target rotation speed ranging from 1 Hz to 7 Hz. The speed can be controlled via the F10 key or MIDI CCN 79.
- The speaker accelerates (decelerates) at 7 (14) Hz/s.

## Live Interactions

You can live-control your waveforms with your mouse pointer, touch pad or any MIDI Control Change messages source.

The following example stage defines a resonating low-pass filter whose resonance frequency can be controlled with a MIDI modulation wheel/lever from 0 to 10,000 Hz.

```yml
stages:
  - Filter:
      kind: LowPass2
      resonance:
        Controller:
          kind: Modulation
          from: 0.0
          to: 10000.0
      quality: 5.0
      in_buffer: 0
      out_buffer: AudioOut
      out_level: 1.0
```

If you want to use the mouse's vertical axis for sound control use the Breath controller.

```yml
resonance:
  Controller:
    kind: Breath
    from: 0.0
    to: 10000.0
```

If you want to use the touchpad for polyphonic sound control use the KeyPressure property.

```yml
resonance:
  Linear:
    value: KeyPressure
    from: 0.0
    to: 10000.0
```

# Feature List

- Sound features
  - Built-in modular waveform synthesizer with physical modeling synthesis
    ```bash
    microwave run --wv-loc <waveforms-file-location> [scale-expression]
    ```
  - Soundfont renderer
    ```bash
    microwave run --sf-loc <soundfont-file-location> [scale-expression]
    ```
  - External synthesizer via MIDI-out
    ```bash
    microwave run --midi-out <midi-target> --tun-method <tuning-method> [scale-expression]
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
  - Lumatone / multichannel input
    ```bash
    # 31-EDO Lumatone preset centered around D4 (62, Layout offset -5)
    microwave ref-note 62 --root 57 --luma-offs 31 --lo-key 0 --up-key 155 --midi-in lumatone steps 1:31:2
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
  - MIDI-out retuning via different tuning message types
  - Display frequencies and rational number approximations
  - Customizable second visual keyboard (`--kb2` option)

![](https://github.com/Woyten/tune/raw/master/microwave/screenshot2.png)

# Help

For a complete list of command line options run

```bash
microwave help
```

# License

`microwave` statically links against [OxiSynth](https://crates.io/crates/oxisynth) for soundfont rendering capabilities. This makes the *binary executable* of `microwave` a derivative work of OxiSynth. OxiSynth is licensed under the *GNU Lesser General Public License, version 2.1*.
