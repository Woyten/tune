Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [CLI documentation](https://github.com/Woyten/tune/blob/master/tune-cli/README.md)
- [API documentation](https://docs.rs/tune/)

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.

Current projects relying on `tune` are:

- [`tune-cli`](https://github.com/Woyten/tune/tree/master/tune-cli): A command line tool with live-retuning capabilities
- [`microwave`](https://github.com/Woyten/tune/tree/master/microwave): A microtonal modular waveform synthesizer
- [`fluid-xenth`](https://github.com/Woyten/tune/tree/master/fluid-xenth): A microtonal soundfont renderer

# Demo

- [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)
- [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)

# Feature List

- Pitch conversions
  - Convert between linear and logarithmic ratios
  - Determine the frequency for a given note in a custom tuning system
  - Determine the note for a given frequency in a custom tuning system
  - Find fractional approximations for frequency ratios
- Export scales
  - To Scala (scl and kbm) format
  - As Midi Tuning Standard (MTS) Sysex Messages
    - Single Note Tuning Change (with Bank Select)
    - Scale/Octave Tuning (1-Byte and 2-Byte)
- Import scales
  - From Scala (scl and kbm) format
- Live retuning
  - Enhance the capabilities of synthesizers with limited tuning support
    - Tune channels ahead of time to keep the bandwidth low
    - Tune channels just in time for full pitch freedom
  - Pick the message type that your synth supports
    - Single Note Tuning Change
    - Scale/Octave Tuning
    - Channel Fine Tuning
    - Pitch Bend
- Equal-step tunings
  - Analyze meantone, mavila and porcupine temperaments
  - Find patent vals
  - Find tempered-out commas
  - PerGen-based notation
    - Determine generalized accidentals
    - Render generalized note names
  - Render generalized keyboard layouts
- MOS scales
  - Find MOSes for a given generator
  - Find generators for a given MOS
- MIDI messages
  - Create basic MIDI messages
  - Create tuning-related RPN messages
  - Parse basic MIDI messages