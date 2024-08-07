Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.

Current projects relying on `tune` are:

- [tune-cli](https://github.com/Woyten/tune/tree/main/tune-cli): A command line tool with live-retuning capabilities
- [microwave](https://github.com/Woyten/tune/tree/main/microwave): A microtonal modular waveform synthesizer
- [fluid-xenth](https://github.com/Woyten/tune/tree/main/fluid-xenth): A microtonal soundfont renderer

## Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [CLI Documentation](https://github.com/Woyten/tune/blob/main/tune-cli/README.md)
- [API Documentation](https://docs.rs/tune/)
- Demo: [Ephemeral Happiness (17-EDO)](https://youtu.be/FZlZE4hLLhs)
- Demo: [I'm a Lumatic (17-EDO)](https://youtu.be/zKnJJEaidWI)
- Demo: [Stay Strong (17-EDO)](https://youtu.be/JutcUVrA8Tg)
- Demo: [Xênerie (15-EDO)](https://youtu.be/0PczKDrOdUA)
- Demo: [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)
- Demo: [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)

## Features

### Pitch Conversions

- Convert between linear and logarithmic pitch ratios
- Determine the frequency for a given note in a custom tuning system
- Determine the note for a given frequency in a custom tuning system
- Find fractional approximations for frequency ratios

### Export Scales

- To Scala (scl and kbm) format
- As Midi Tuning Standard (MTS) Sysex Messages
  - Single Note Tuning Change (with Bank Select)
  - Scale/Octave Tuning (1-Byte and 2-Byte)

### Import Scales

- From Scala (scl and kbm) format

### Live Retuning

- Enhance the capabilities of synthesizers with limited tuning support
  - Tune channels ahead of time for a glitch free tuning experience
  - Tune channels just in time for full pitch freedom
- Pick the message type that your synth supports
  - Single Note Tuning Change (with Bank Select)
  - Scale/Octave Tuning (1-Byte and 2-Byte)
  - Channel Fine Tuning
  - Pitch Bend

### MOS Scales and Isomorphic Keyboards

- Find MOSes for a given generator
- Find generators for a given MOS
- Find MOS-based isomorphic keyboard layouts
  - Supported genchains: Meantone, Mavila, Porcupine, Tetracot, Hanson
  - Determine step sizes
  - Generate automatic color schemas
  - Print generalized note names and accidentals

### Commas and Temperaments

- Find patent vals
- Find tempered-out commas

### MIDI Messages

- Create basic MIDI messages
- Create tuning-related RPN messages
- Parse basic MIDI messages