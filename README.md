Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.
So far, `tune` offers a CLI and an API with the following features:

- Pitch conversions
  - Convert between linear and logarithmic ratios
  - Determine the frequency for a given note in a custom tuning system
  - Determine the note for a given frequency in a custom tuning system
  - Find fractional approximations for frequency ratios
- EDO scales
  - Analyze meantone temperaments
  - Find keyboard layouts
- Export scales
  - To Scala (scl and kbm) format
  - As Midi Tuning Standard (MTS) Sysex Messages

## CLI Documentation

https://crates.io/crates/tune-cli

## API Documentation

https://docs.rs/tune/
