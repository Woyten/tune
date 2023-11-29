Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.

Current projects relying on `tune` are:

- [tune-cli](https://github.com/Woyten/tune/tree/master/tune-cli): A command line tool with live-retuning capabilities
- [microwave](https://github.com/Woyten/tune/tree/master/microwave): A microtonal modular waveform synthesizer
- [fluid-xenth](https://github.com/Woyten/tune/tree/master/fluid-xenth): A microtonal soundfont renderer

## Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [CLI Documentation](https://github.com/Woyten/tune/blob/master/tune-cli/README.md)
- [API Documentation](https://docs.rs/tune/)
- Demo: [I'm a Lumatic (17-EDO)](https://youtu.be/zKnJJEaidWI)
- Demo: [Stay Strong (17-EDO)](https://youtu.be/JutcUVrA8Tg)
- Demo: [Xênerie (15-EDO)](https://youtu.be/0PczKDrOdUA)
- Demo: [Don't Take Five (16-EDO)](https://youtu.be/LLgClI8pyNw)
- Demo: [The Bedoginning (17-EDO)](https://youtu.be/gaYvK9OBHK0)

## Features

### Pitch Conversions

- Convert between linear and logarithmic ratios
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
  - Tune channels ahead of time to keep the bandwidth low
  - Tune channels just in time for full pitch freedom
- Pick the message type that your synth supports
  - Single Note Tuning Change (with Bank Select)
  - Scale/Octave Tuning (1-Byte and 2-Byte)
  - Channel Fine Tuning
  - Pitch Bend

### Equal-Step Tunings

- Find patent vals
- Find tempered-out commas
- Isomorphic keyboards / Lumatone
  - Find appropriate layouts (meantone or porcupine)
  - Determine isomorphic step sizes
  - Print generalized note names and accidentals
  - Auto-generate nice-looking keyboard color schemas

### MOS Scales

- Find MOSes for a given generator
- Find generators for a given MOS

### MIDI Messages

- Create basic MIDI messages
- Create tuning-related RPN messages
- Parse basic MIDI messages