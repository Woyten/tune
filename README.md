Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Overview

`tune` is planned to be part of a larger ecosystem for microtonal software in Rust.
So far, `tune` offers a CLI and an API with the following features:

- Pitch conversions
  - Convert between linear and logarithmic ratios
  - Determine the frequency for a given note in a custom tuning system
  - Determine the note for a given frequency in a custom tuning system
  - Find fractional approximations for frequency ratios
- Export scales
  - To Scala (scl and kbm) format
  - As Midi Tuning Standard (MTS) Sysex Messages

[API documentation](https://docs.rs/tune/)

# Installation

```bash
cargo install -f tune
```

# Usage

## Create scl files

* 12-TET
  ```bash
  tune scl equal 1:12:2
  tune scl equal 100c
  ```
* Bohlen-Pierce
  ```bash
  tune scl equal 1:13:3
  ```
* Equal temperament with step size of 5 sixth tones
  ```bash
  tune scl equal 5:36:2
  tune scl equal 5/3:12:2
  tunc scl equal {500/3}c
  ```
* 7-note Pythagorean (lydian mode)
  ```bash
  tune scl rank2 3/2 6
  tune scl rank2 1.5 6
  ```
* 7-note quarter-comma meantone (major mode)
  ```bash
  tune scl rank2 1:4:5 5 1
  ```
* 8-note harmonic series
  ```bash
  tune scl harm 8
  ```
* Custom just intonation scale
  ```bash
  tune scl cust -n "Just intonation" 9/8 5/4 4/3 3/2 5/3 15/8 2
  ```

## Create kbm files

* Start scale at C4 at its usual frequency
  ```bash
  tune kbm 60
  ```

* Start scale at C4, 20 cents higher than usual
  ```bash
  tune kbm 60+20c
  ```

* Start scale at A4 at 450 Hz
  ```bash
  tune kbm 69@450Hz
  ```

* Start scale at C4, A4 should sound at 450 Hz
  ```bash
  tune kbm -r 60 69@450Hz
  ```

## Dump pitches of a scale

* 7-note Pythagorean (D dorian mode)
  ```bash
  tune dump 62 rank2 3/2 3 3
  ```
  **Output:**
  ```bash
  ..
  >  62 |   293.665 Hz | MIDI  62 |      D 4 |   +0.000¢ | 1/1 [+0c] (+0o)
     63 |   330.373 Hz | MIDI  64 |      E 4 |   +3.910¢ | 9/8 [+0c] (+0o)
     64 |   348.047 Hz | MIDI  65 |      F 4 |   -5.865¢ | 6/5 [-22c] (+0o)
     65 |   391.553 Hz | MIDI  67 |      G 4 |   -1.955¢ | 4/3 [+0c] (+0o)
     66 |   440.497 Hz | MIDI  69 |      A 4 |   +1.955¢ | 3/2 [+0c] (+0o)
     67 |   495.559 Hz | MIDI  71 |      B 4 |   +5.865¢ | 5/3 [+22c] (+0o)
     68 |   522.071 Hz | MIDI  72 |      C 5 |   -3.910¢ | 16/9 [+0c] (+0o)
     69 |   587.330 Hz | MIDI  74 |      D 5 |   +0.000¢ | 1/1 [+0c] (+1o)
  ..
  ```

* As JSON
  ```bash
  tune jdump 62 rank2 3/2 3 3
  ```
  **Output:**
  ```json
  ..
  {
    "key_midi_number": 62,
    "scale_degree": 0,
    "pitch_in_hz": 293.6647679174076
  },
  {
    "key_midi_number": 63,
    "scale_degree": 1,
    "pitch_in_hz": 330.3728639070835
  },
  ..
  ```

* Conversion between scales: What are the pitch differences between Pythagorean and quarter-comma meantone tuning?
  ```bash
  tune jdump 62 rank2 3/2 3 3
  ```
  **Output:**
  ```bash
  ..
  >  62 |   293.665 Hz | MIDI  62 | IDX   0 |   +0.000¢ | 1/1 [+0c] (+0o)
     63 |   330.373 Hz | MIDI  63 | IDX   1 |  +10.753¢ | 9/8 [+0c] (+0o)
     64 |   348.047 Hz | MIDI  64 | IDX   2 |  -16.130¢ | 6/5 [-22c] (+0o)
     65 |   391.553 Hz | MIDI  65 | IDX   3 |   -5.377¢ | 4/3 [+0c] (+0o)
     66 |   440.497 Hz | MIDI  66 | IDX   4 |   +5.377¢ | 3/2 [+0c] (+0o)
     67 |   495.559 Hz | MIDI  67 | IDX   5 |  +16.130¢ | 5/3 [+22c] (+0o)
     68 |   522.071 Hz | MIDI  68 | IDX   6 |  -10.753¢ | 16/9 [+0c] (+0o)
     69 |   587.330 Hz | MIDI  69 | IDX   7 |   +0.000¢ | 1/1 [+0c] (+1o)
  ..
  ```

## Create a Midi Tuning Standard Sysex message

* 19-TET
  ```bash
  tune mts 69 equal 1:19:2
  ```
  **Output:**
  ```bash
  0xf0
  0x7f
  0x7f
  0x08
  ..
  0x7f
  0x00
  0x00
  0xf7
  Number of retuned notes: 127
  Number of out-of-range notes: 0
  ```

## Expressions

Ordered by precedence:

1. `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
1. `<num>/<denom>` evaluates to `num/denom`
1. `<cents>c` evaluates to `2^(cents/1200)`
1. `{<expr>}` evaluates to `expr`
