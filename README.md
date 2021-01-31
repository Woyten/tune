Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [CLI documentation](https://crates.io/crates/tune-cli)
- [API documentation](https://docs.rs/tune/)

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.

Current projects relying on `tune` are:

- [`tune-cli`](https://crates.io/crates/tune-cli): A command line tool with live-retuning capabilities
- [`microwave`](https://crates.io/crates/microwave): A microtonal modular waveform synthesizer

# Feature List

- Pitch conversions
  - Convert between linear and logarithmic ratios
  - Determine the frequency for a given note in a custom tuning system
  - Determine the note for a given frequency in a custom tuning system
  - Find fractional approximations for frequency ratios
- Export scales
  - To Scala (scl and kbm) format
  - As Midi Tuning Standard (MTS) Sysex Messages
    - Single Note Tuning
    - Scale/Octave Tuning
- Import scales
  - From Scala (scl and kbm) format
- Create tuning maps / Apply live retuning
  - Enhance the capabilities of synthesizers with limited tuning support
  - Tune multiple channels ahead of time or tune a single channel just in time
  - Send pitch-bend messages (polyphonic or monophonic) if no MTS is supported at all
- Equal-step tunings
  - Analyze meantone and porcupine temperaments
  - Find keyboard layouts
  - Find patent vals
- MIDI messages
  - Create basic MIDI messages
  - Parse basic MIDI messages