Explore musical tunings and create synthesizer tuning files for microtonal scales.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [Web App](http://woyten.github.io/tune-cli/)


# Overview

`tune-cli` is the command line tool for the microtonal [tune](https://crates.io/crates/tune) library.

# Installation

```bash
cargo install -f tune-cli
```

# Usage

## Introduction

You want to know how to tune your piano in 7-EDO? Just use the following command:

```rust
tune scale 62 steps 1:7:2 | tune dump
```

This instructs `tune` to print the frequencies and approximate notes of a 7-EDO scale starting at D4 (MIDI number 62).

```rust
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

If you do not want to retune your keyboard manually you can instruct `tune-cli` to send MIDI Tuning Standard messages to your synthesizer. To do so, locate your target MIDI device first:

```bash
tune devices
```

This will list all available MIDI devices:

```bash
Readable MIDI devices:
(0) Midi Through:Midi Through Port-0 14:0
Writable MIDI devices:
(0) Midi Through:Midi Through Port-0 14:0
(1) FLUID Synth (23673):Synth input port (23673:0) 128:0
```

Now, send a 7-EDO *Scale/Octave Tuning* message to FLUID Synth:

```bash
tune mts --send-to 1 octave 62 steps 1:7:2
```

### Full Keyboard Tuning

The most generic type of tuning message is the *Single Note Tuning* message providing control over the pitch of each note. Note, however, that many synthesizers do not support this tuning message. The correspondig command is:

```bash
tune scale 62 steps 1:7:2 | tune mts from-json
```

This will print:

```rust
== SysEx start ==
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
== SysEx end ==
```

Some notes are reported to be out of range. This is because 7-EDO has a stronger per-step increase in frequency than  12-EDO, s.t. some frequencies become unmappable.

You can also save a binary version of the tuning message using the `--bin` option.

```bash
tune scale 62 steps 1:7:2 | tune mts --bin tuning_message.syx from-json
```

#### Limitations

The current implementation doesn't allow for gaps in a scale. This means the Single Note Tuning version of the 7-EDO scale has to be played on *all* piano keys with black and white keys mixed. Hopefully, this is going to be fixed soon.

### Live Retuning

If your synthesizer has no support for full-keyboard tuning messages but for octave-based tuning messages, there still is a way to play almost every scale on that device.

To do so, try using `tune-cli`'s live retuning feature which can be activated via the `tune live` subcommand:

```
tune live --midi-in 1 --midi-out 1 aot 62 steps 1:22:2
```

The given command will enable ahead-of-time live retuning for 22-EDO on device 1. The term "ahead-of-time" reflects the fact that several channels will be retuned on startup. After that, each incoming message is mapped to an outgoing message on the channel that has the appropriate tuning applied.

### Scala File Format

An alternative tuning method is to upload scl and kbm files to your synthesizer. See the scl and kbm sections below for more information.

### Approximate Ratios

The `dump` command provides information about the qualities of a scale. Let's have a look at the 19-EDO scale:

```bash
tune scale 62 steps 1:19:2 | tune dump
```

The output reveals that some rational intervals are well approximated. Especially the just minor third (6/5) which is approximated by less than than 1¢ and, therefore, displayed as 0¢:

```rust
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
..
   67 | IDX    5 |  6/5    +0¢  +0o ‖     352.428 Hz ‖   65 |       F 4 |  +15.789¢
..
```

The ratio approximation algorithm is not very advanced yet and does not use prime numbers.

### Compare Scales

Imagine, you want to know how well quarter-comma meantone is represented in 31-EDO. All you need to do is create the quarter-comma meantone scale (`tune scale`) and `tune diff` it against the 31-EDO scale.

In quarter-comma meantone the fifths are tempered in such a way that four of them match up a frequency ratio of 5. This makes the genator of the scale equal to 5^(1/4) or `1:4:5` in `tune` expression notation. To obtain a full scale, let's say ionian/major, you need to walk 5 generators/fifths upwards and one downwards which translates to the scale expression `rank2 1:4:5 5 1`.

The scale expression for the 31-EDO scale is `steps 1:31:2`, s.t. the full scale comparison command becomes:

```bash
tune scale 62 rank2 1:4:5 5 1 | tune diff 62 steps 1:31:2
```

This will print:

```rust
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

You can see that 31-EDO is a *very* good approximation of quarter-comma meantone with a maximum deviation of -0.979¢. You can also see that the step sizes of the corresponding 31-EDO scale are 5, 5, 3, 5, 5, 5 and 3.

### EDO analysis

The `tune est` command prints basic information about any equal-step tuning. The step sizes and sharp values are derived based on the arithmetics of meantone tuning.

Example output of `tune est 1:17:2`:

```rust
---- Properties of 17-EDO (Meantone) ----

Number of cycles: 1
1 fifth = 10 EDO steps = +705.9c (pythagorean +3.9c)
1 primary step = 3 EDO steps
1 secondary step = 1 EDO steps
1 sharp = 2 EDO steps

-- Keyboard layout --
 13  16  2   5   8   11  14  0   3   6
 14  0   3   6   9   12  15  1   4   7
 15  1   4   7   10  13  16  2   5   8
 16  2   5   8   11  14  0   3   6   9
 0   3   6   9   12  15  1   4   7   10
 1   4   7   10  13  16  2   5   8   11
 2   5   8   11  14  0   3   6   9   12
 3   6   9   12  15  1   4   7   10  13
 4   7   10  13  16  2   5   8   11  14
 5   8   11  14  0   3   6   9   12  15

-- Scale steps --
  0. D
  1. Eb
  2. D# / Fb
  3. E
  4. F **JI m3rd**
  5. E# / Gb **JI M3rd**
  6. F#
  7. G **JI P4th**
  8. Ab
  9. G#
 10. A **JI P5th**
 11. Bb
 12. A# / Cb
 13. B
 14. C
 15. B# / Db
 16. C#
```

## Create scl Files / Scale Expressions

* Equal temperament
  ```bash
  tune scl steps 1:12:2      # 12-EDO
  tune scl steps 100c        # 12-EDO
  tune scl steps 1:36:2      # Sixth-tone
  tune scl steps '(100/3)c'  # Sixth-tone
  tune scl steps 1:13:3      # Bohlen-Pierce
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
  tune scl -n "Just intonation" steps 9/8 5/4 4/3 3/2 5/3 15/8 2
  ```

* Imported scale
  ```bash
  tune scl import my_scale.scl
  ```

## Create kbm Files / Keyboad Mapping Expressions

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
tune scale 62 steps 1:7:2
```
**Output (shortened):**

```json
{
  "Scale": {
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
1. `(<expr>)` evaluates to `expr`
