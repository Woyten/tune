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

## Introduction

You want to know how to tune your piano in 7-EDO? Just use the following command:

```rust
tune dump 62 equal 1:7:2
```

This instructs `tune` to print the frequencies and approximate notes of a 7-EDO scale starting at D4 (MIDI number 62).

```bash
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
..
>  62 | IDX    0 |  1/1    +0¢  +0o ‖     293.665 Hz ‖   62 |       D 4 |   +0.000¢
   63 | IDX    1 | 11/10   +6¢  +0o ‖     324.232 Hz ‖   64 |       E 4 |  -28.571¢
   64 | IDX    2 | 11/9    -5¢  +0o ‖     357.981 Hz ‖   65 |       F 4 |  +42.857¢
   65 | IDX    3 |  4/3   +16¢  +0o ‖     395.243 Hz ‖   67 |       G 4 |  +14.286¢
   66 | IDX    4 |  3/2   -16¢  +0o ‖     436.384 Hz ‖   69 |       A 4 |  -14.286¢
   67 | IDX    5 | 18/11   +5¢  +0o ‖     481.807 Hz ‖   71 |       B 4 |  -42.857¢
   68 | IDX    6 | 20/11   -6¢  +0o ‖     531.958 Hz ‖   72 |       C 5 |  +28.571¢
   69 | IDX    7 |  2/1    -0¢  +0o ‖     587.330 Hz ‖   74 |       D 5 |   -0.000¢
..
```

The table tells us that the first step of the 7-EDO scale (`IDX 0`) has a frequency of 293.655 Hz and matches D4 *exactly*. This is obvious since we chose D4 be the origin of the 7-EDO scale. `IDX 1`, the second step of the scale, is reported to be close to E4 but with an offset of -28.571¢.

You can now detune every note D on your piano by -28.571¢. On an electric piano with octave-based tuning support, this is a very easy task. It is also possible to retune a real piano using a tuning device.

Retune every note of the 7-EDO scale according to the table and the 7-EDO scale will be playable on the white keys!

### MIDI Tuning Standard

The most generic way to tune your piano is the MIDI Tuning Standard. You can print out a *Single Note Tuning* Message (i.e. every note is retuned individually) with the following command:

```bash
tune jdump 62 equal 1:7:2 | tune mts
```

The output will be:

```bash
0xf0
0x7f
0x7f
0x08
..
0x7f
0x12
0x25
0xf7
Number of retuned notes: 75
Number of out-of-range notes: 52
```

Some notes are reported to be out of range. This is because 7-EDO has a stronger per-step increase in frequency than  12-EDO, s.t. some frequencies become unmappable.

#### Limitations

The current implemention doesn't allow for gaps in a scale. This means the MTS version of the 7-EDO scale has to be played on *all* piano keys with black and white keys mixed. Hopefully, this is going to be fixed soon.

### Scala File Format

An alternative tuning method is to upload scl and kbm files to your synthesizer. See the scl and kbm sections below for more information.

### Approximate Ratios

The `dump` command provides further information about the qualities of a scale. Let's have a look at the 19-EDO scale:

```bash
tune dump 62 equal 1:19:2
```

The output reveals that some rational intervals are well approximated. Especially the just minor third (6/5) which is approximated by less than than 1¢ and, therefore, displayed as 0¢:

```bash
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
..
   67 | IDX    5 |  6/5    +0¢  +0o ‖     352.428 Hz ‖   65 |       F 4 |  +15.789¢
..
```

The ratio approximation algorithm is not very advanced yet and does not use prime numbers.

### Compare Scales

Imagine, you want to know how well quarter-comma meantone is represented in 31-EDO. All you need to do is `jdump` a quarter-comma meantone scale and `diff` it against the 31-EDO scale.

In quarter-comma meantone the fifths are tempered in such a way that four of them match up a frequency ratio of 5. This makes the genator of the scale equal to 5^(1/4) or `1:4:5` in `tune` expression notation. To obtain a full scale, let's say ionian/major, you need to walk 5 generators/fifths upwards and one downwards which translates to the scale expression `rank2 1:4:5 5 1`.

The scale expression for the 31-EDO scale is `equal 1:31:2`, s.t. the full scale comparison command becomes:

```bash
tune jdump 62 rank2 1:4:5 5 1 | tune diff 62 equal 1:31:2
```

This will print:

```bash
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
..
>  62 | IDX    0 |  1/1    +0¢  +0o ‖     293.665 Hz ‖   62 | IDX     0 |   +0.000¢
   63 | IDX    1 |  9/8   -11¢  +0o ‖     328.327 Hz ‖   67 | IDX     5 |   -0.392¢
   64 | IDX    2 |  5/4    +0¢  +0o ‖     367.081 Hz ‖   72 | IDX    10 |   -0.783¢
   65 | IDX    3 |  4/3    +5¢  +0o ‖     392.771 Hz ‖   75 | IDX    13 |   +0.196¢
   66 | IDX    4 |  3/2    -5¢  +0o ‖     439.131 Hz ‖   80 | IDX    18 |   -0.196¢
   67 | IDX    5 |  5/3    +5¢  +0o ‖     490.964 Hz ‖   85 | IDX    23 |   -0.587¢
   68 | IDX    6 | 11/6   +34¢  +0o ‖     548.914 Hz ‖   90 | IDX    28 |   -0.979¢
   69 | IDX    7 |  1/1    +0¢  +1o ‖     587.330 Hz ‖   93 | IDX    31 |   +0.000¢
..
```

You can see that 31-EDO is a *very* good approximation of quarter-comma meantone with a maximum deviation of -0.979¢. You can also see that the steps sizes of the corresponding 31-EDO scale are 5, 5, 3, 5, 5, 5 and 3.

## Create scl Files / Scale Expressions

* Equal temperament
  ```bash
  tune scl equal 1:12:2      # 12-EDO
  tune scl equal 100c        # 12-EDO
  tune scl equal 1:36:2      # Sixth-tone
  tune scl equal {100/3}c    # Sixth-tone
  tune scl equal 1:13:3      # Bohlen-Pierce
  ```

* Meantone temperament
  ```bash
  tune scl rank2 3/2 6       # Pythagorean (lydian)
  tune scl rank2 1.5 6 6     # Pythagorean (12-note)
  tune scl rank2 1:4:5 5 1   # quarter-comma meantone (major)
  tune scl rank2 18:31:2 3 3 # 31-EDO meantone (dorian)
  ```

* Harmonic series
  ```bash
  tune scl harm 8            # 8:9:10:11:12:13:14:15:16 scale
  tune scl harm -s 8         # ¹/₁₆:¹/₁₅:¹/₁₄:¹/₁₃:¹/₁₂:¹/₁₁:¹/₁₀:¹/₉:¹/₈ scale
  ```

* Custom scale
  ```bash
  tune scl cust -n "Just intonation" 9/8 5/4 4/3 3/2 5/3 15/8 2
  ```

## Create kbm Files / Key Map Expressions

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

## JSON Output

`tune` uses JSON as an exchange format between pipelined calls. You can use `tune`'s output as an input for an external application (or the other way around) or inspect/modify the output manually before further processing.

### Example Usage

```bash
cargo jdump 62 equal 1:7:2
```
**Output (shortened):**

```json
{
  "Dump": {
    "root_key_midi_number": 62,
    "root_pitch_in_hz": 293.6647679174076,
    "items": [
      {
        "key_midi_number": 62,
        "pitch_in_hz": 293.6647679174076
      },
      {
        "key_midi_number": 63,
        "pitch_in_hz": 324.23219079306347
      },
    ]
  }
}
```

## Expressions

Ordered by precedence:

1. `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
1. `<num>/<denom>` evaluates to `num/denom`
1. `<cents>c` evaluates to `2^(cents/1200)`
1. `{<expr>}` evaluates to `expr`
