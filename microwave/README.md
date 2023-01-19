Make xenharmonic music and explore musical tunings.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Scale expressions](https://github.com/Woyten/tune/blob/master/tune-cli/README.md)

# Overview

`microwave` is a microtonal modular waveform synthesizer with soundfont rendering capabilities based on:

- [tune](https://github.com/Woyten/tune) &ndash; a microtonal library
- [magnetron](https://github.com/Woyten/tune/tree/master/magnetron) &ndash; a modular synthesizer architecture
- [fluid-xenth](https://github.com/Woyten/tune/tree/master/fluid-xenth) &ndash; a microtonal soundfont renderer
- [Bevy](https://bevyengine.org) &ndash; a graphics and game engine

It features a virtual piano UI enabling you to play polyphonic microtonal melodies with your touch screen, computer keyboard, MIDI keyboard or mouse. The UI provides information about pitches and just intervals in custom tuning systems.

The built-in modular synthesis engine does not use any fixed architecture and can be customized to react to all sorts of input events.

# Demo

- [Xênerie (15-EDO)](https://youtu.be/0PczKDrOdUA)
- [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)
- [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)

# Download / Installation

Option A: Try out the web app to get a very first impression:

- [microwave (Browser)](https://woyten.github.io/microwave) - Very experimental!

Option B: Download a precompiled version of `microwave` for the supported target architectures:

- [microwave 0.33.0 (Linux)](https://github.com/Woyten/tune/releases/download/microwave-0.33.0/microwave-0.33.0-x86_64-unknown-linux-gnu.zip)
- [microwave 0.33.0 (Windows)](https://github.com/Woyten/tune/releases/download/microwave-0.33.0/microwave-0.33.0-x86_64-pc-windows-msvc.zip)
- [microwave 0.33.0 (macOS)](https://github.com/Woyten/tune/releases/download/microwave-0.33.0/microwave-0.33.0-x86_64-apple-darwin.zip)

Option C: Use Cargo to build a fresh binary from scratch for your own target architecture:

```bash
# If you are using Linux: Make sure all dev dependencies are installed.
# On the CI environment (Ubuntu 20.04) we only need to add two dev dependencies:
sudo apt install libasound2-dev libudev-dev

# Make sure pkg-config is installed
sudo apt install pkg-config

cargo install -f microwave
```

`microwave` should run out-of-the box on a recent (Ubuntu) Linux, Windows or macOS installation. If it doesn't, the problem is probably caused by the Bevy framework. In that case, try following these [instructions](https://bevyengine.org/learn/book/getting-started/setup/).

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

## MIDI In

To listen for events originating from an external MIDI device you need to specify the name of the input device:

```bash
microwave devices # List MIDI devices
microwave run --midi-in name-of-my-device
microwave run --midi-in "name of my device" # If the device name contains spaces
```

## MIDI Out

To enable playback through an external MIDI device you need to specify the name of the output device *and* a tuning method. The available tuning methods are `full`, `full-rt`, `octave-1`, `octave-1-rt`, `octave-2`, `octave-2-rt`, `fine-tuning` and `pitch-bend`.

```bash
microwave devices # List MIDI devices
microwave run --midi-out name-of-my-device --tun-method octave-1
microwave run --midi-in "name of my device" --tun-method octave-1 # If the device name contains spaces
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

### LF Sources

Almost all waveform and effect parameters are real numbers that can update in real-time. To keep the waveforms engine performant updates are usually evaluated at a much lower rate than the audio sampling rate. LF sources, therefore, add control and expressiveness to your playing but aren't well suited for spectral modulation.

To define LF sources the following data types can be used:

- Numbers, e.g.
  ```yml
  frequency: 440.0
  ```
- Strings referring to a named template, e.g.
  ```yml
  frequency: WaveformPitch
  ```
- Nested LF source expressions, e.g.
  ```yml
  frequency: { Mul: [ 2.0, WaveformPitch ] }
  ```
  or (using indented style)
  ```yml
  frequency:
    Mul:
      - 2.0
      - WaveformPitch
  ```

Unfortunately, no detailed LF source documentation is available yet. However, the example config, `microwave`'s error messages and basic YAML knowledge should enable you to find valid LF source expressions.

### `waveform_templates` Section

The purpose of the `waveform_templates` section of the config file is to define the most important LF sources s.t. they do not have to be redefined over and over again.

#### Example: Using Pitch-Bend Events

Looking at the initially created config file the following template definition can be found:

```yml
waveform_templates:
  - name: WaveformPitch
    value:
      Mul:
        - Property:
            kind: WaveformPitch
        - Semitones:
            Controller:
              kind: PitchBend
              map0: 0.0
              map1: 2.0
```

The given fragment defines a template with name `WaveformPitch` which provides values by reading the waveform's `WaveformPitch` property and multiplying it with the pitch-bend wheel's value in whole tones.

This means reacting to pitch-bend events is not a hardcoded feature of `microwave` but a behavior that the user can define by themself!

#### Example: Using the Damper Pedal

A second important template from the initial config file is the following one:

```yml
waveform_templates:
  - name: Fadeout
    value:
      Controller:
        kind: Damper
        map0:
          Property:
            kind: OffVelocitySet
        map1: 0.0
```

The given template provides a value describing to what extent a waveform is supposed to be faded out. It works in the following way:

While a key is pressed down, `OffVelocitySet` resolves to 0.0. As a result, `Controller`, as well, resolves to 0.0, regardless of the damper pedal state. Therefore, the waveform remains stable.

As soon as a key is released, `OffVelocitySet` will resolve to 1.0. Now, `Controller` will interpolate between 0.0 (damper pedal pressed) and 1.0 (damper pedal released). Ergo, the waveform will fade out unless the damper pedal is pressed.

Like in the example before, reacting to the damper pedal is not a hardcoded feature built into `microwave` but customizable behavior.

#### How to Access Templates

Just provide the name of the template as a single string argument. Examples:

```yml
frequency: WaveformPitch
```

```yml
fadeout: Fadeout
```

### `waveform_envelopes` Section

Every waveform needs to refer to an envelope defined in the `waveform_envelopes` section of the config file. Envelopes transfer the result of the waveform's `AudioOut` buffer to the main audio pipeline and limit the waveform's lifetime.

An envelope definition typically looks like the following:

```yml
waveform_envelopes:
  - name: Piano
    amplitude: Velocity
    fadeout: Fadeout
    attack_time: 0.01
    decay_rate: 1.0
    release_time: 0.25
```

with

- `name`: The name of the envelope.
- `amplitude`: The amplitude factor to apply to the `AudioOut` buffer. It makes sense to use `Velocity` as a value but the user can choose whatever LF source expression they find useful.
- `fadeout`: Defines the amount by which the waveform should fade out. **Important:** If this value is set to constant 0.0 the waveform will never fade out and continue to consume CPU resources, eventually leading to an overload of the audio thread.
- `attack_time`: The linear attack time in seconds.
- `decay_rate`: The exponential decay rate in 1/seconds (inverse half-life) after the attack phase is over.
- `release_time`: The linear release time in seconds. The waveform is considered exhausted as soon as the integral over `fadeout / release_time * dt` reaches 1.0.

### `waveforms` Section

The `waveforms` section of the config file defines the waveform render stages to be applied sequentially when a waveform is triggered.

You can mix and match as many stages as you want to create the tailored sound you wish for. The following example config defines a clavinettish sounding waveform that I discovered by accident:

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

### `effect_templates` Section

This section is completely analogous to the `waveform_templates` section but it is dedicated to work in combination with the following `effects` section.

### `effects` Section

The `effects` section of the config file defines the effects to be applied sequentially after the waveforms have been rendered.

Use the following config as an example to add a rotary-speaker effect to your sound.

```yml
effects:
  - RotarySpeaker:
      buffer_size: 100000
      gain:
        Controller:
          kind: Sound9
          map0: 0.0
          map1: 0.5
      rotation_radius: 20.0
      speed:
        Controller:
          kind: Sound10
          map0: 1.0
          map1: 7.0
      acceleration: 6.0
      deceleration: 12.0
```

The given config defines the following properties:

- `buffer_size`: A fixed delay buffer size of 100000 samples
- `gain`: An input gain ranging from 0.0 to 0.5. The input gain can be controlled via the F9 key or MIDI CCN 78.
- `rotation_radius`: A rotation radius of 20 cm
- `speed`: A target rotation speed ranging from 1 Hz to 7 Hz. The speed can be controlled via the F10 key or MIDI CCN 79.
- `{acc,dec}eleration`: The speaker accelerates (decelerates) at 6 (12) Hz/s.

## Live Interactions

You can live-control your waveforms with your mouse pointer, touch pad or any MIDI Control Change messages source.

The following example stage defines a resonating low-pass filter whose resonance frequency can be controlled with a MIDI modulation wheel/lever from 0 to 10,000 Hz.

```yml
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      map0: 0.0
      map1: 10000.0
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0
```

If you want to use the mouse's vertical axis for sound control use the `Breath` controller.

```yml
resonance:
  Controller:
    kind: Breath
    map0: 0.0
    map1: 10000.0
```

If you want to use the touchpad for polyphonic sound control use the `KeyPressure` template.

```yml
resonance:
  Linear:
    input: KeyPressure
    map0: 0.0
    map1: 10000.0
```

# Feature List

- Sound features
  - Built-in modular waveform synthesizer with physical modeling synthesis
    ```bash
    microwave run --cfg-loc <config-file-location> [scale-expression]
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
