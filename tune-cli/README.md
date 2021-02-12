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
tune dump ref-note 62 --lo-key 61 --up-key 71 steps 1:7:2
```

This instructs `tune` to print the frequencies and approximate notes of a 7-EDO scale starting at D4 (MIDI number 62). Output:

```rust
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
   61 | IDX   -1 | 20/11   -6¢  -1o ‖     265.979 Hz ‖   60 |      C  4 |  +28.571¢
>  62 | IDX    0 |  1/1    +0¢  +0o ‖     293.665 Hz ‖   62 |      D  4 |   +0.000¢
   63 | IDX    1 | 11/10   +6¢  +0o ‖     324.232 Hz ‖   64 |      E  4 |  -28.571¢
   64 | IDX    2 | 11/9    -5¢  +0o ‖     357.981 Hz ‖   65 |      F  4 |  +42.857¢
   65 | IDX    3 |  4/3   +16¢  +0o ‖     395.243 Hz ‖   67 |      G  4 |  +14.286¢
   66 | IDX    4 |  3/2   -16¢  +0o ‖     436.384 Hz ‖   69 |      A  4 |  -14.286¢
   67 | IDX    5 | 18/11   +5¢  +0o ‖     481.807 Hz ‖   71 |      B  4 |  -42.857¢
   68 | IDX    6 | 20/11   -6¢  +0o ‖     531.958 Hz ‖   72 |      C  5 |  +28.571¢
   69 | IDX    7 |  2/1    -0¢  +0o ‖     587.330 Hz ‖   74 |      D  5 |   -0.000¢
   70 | IDX    8 | 11/10   +6¢  +1o ‖     648.464 Hz ‖   76 |      E  5 |  -28.571¢
```

The table tells us that the first step of the 7-EDO scale (`IDX 0`) has a frequency of 293.655 Hz and matches D4 *exactly*. This is obvious since we chose D4 be the origin of the 7-EDO scale. `IDX 1`, the second step of the scale, is reported to be close to E4 but with an offset of -28.571¢.

You can now detune every note D on your piano by -28.571¢. On an electric piano with octave-based tuning support, this is a very easy task. It is also possible to retune a real piano using a tuning device.

Retune every note of the 7-EDO scale according to the table and the 7-EDO scale will be playable on the white keys!

### MIDI Tuning Standard

If you do not want to retune your electric piano manually you can instruct `tune-cli` to send MIDI Tuning Standard (MTS) messages to your synthesizer. To do so, locate your target MIDI device first:

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

You can now send a 7-EDO *Scale/Octave Tuning* message to device 1 (FLUID Synth):

```bash
tune mts --send-to 1 octave ref-note 62 steps 1:7:2
```

Moreover, the command will log the tuning message to `stdout`:

```rust
== SysEx start (channel 0) ==
0xf0
0x7e
0x7f
0x08
..
0x32
0x40
0x15
0xf7
Sending MIDI data to FLUID Synth (8506):Synth input port (8506:0) 128:0
== SysEx end ==
```

### Full Keyboard Tuning

The most generic MTS-compliant message is the *Single Note Tuning* message providing control over the pitch of each note. Note, however, that many synthesizers do not support this tuning message. The correspondig command is:

```bash
tune mts full ref-note 69 steps 1:7:2
```

Output:

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
Sending MIDI data to FLUID Synth (8506):Synth input port (8506:0) 128:0
Number of retuned notes: 75
Number of out-of-range notes: 13
== SysEx end ==
```

Some notes are reported to be out of range. This is because 7-EDO has a stronger per-step increase in frequency than  12-EDO, s.t. some frequencies become unmappable.

You can also save a binary version of the tuning message using the `--bin` option.

```bash
tune mts --bin tuning_message.syx full ref-note 69 steps 1:7:2
```

#### Limitations

The current implementation doesn't allow for gaps in a scale. This means the Single Note Tuning version of the 7-EDO scale has to be played on *all* piano keys with black and white keys mixed. Hopefully, this is going to be fixed soon.

### Live Retuning

Scale/Octave Tuning messages are not sufficient for most tuning scenarios and the more powerful Single Note Tuning messages are not supported on many synthesizers. Despite all, there are workarounds to play in almost every scale on a device without full MTS support.

To enable `tune-cli`'s *Live Retuning* feature use the `tune live` subcommand:

```bash
tune live --midi-in 1 --midi-out 1 aot ref-note 62 steps 1:22:2
```

This will enable ahead-of-time live retuning for 22-EDO on device 1. The term "ahead-of-time" reflects the fact that several channels will be tuned via Scale/Octave Tuning messages at startup. After that, each incoming message is mapped to an outgoing message on the channel that has the appropriate tuning applied.

Even if your synthesizer has no MTS support at all you can still use pitch-bend based live retuning:

```bash
tune live --midi-in 1 --midi-out 1 ppb ref-note 62 steps 1:22:2
```

This will enable polyphonic pitch-bend live retuning. Since pitch-bend messages are channel-global each active note needs to allocate its own channel from a channel pool. The pool size can be controlled via the `--lo-chan` and `--up-chan` parameters. If the pool is empty new notes cannot be played.

### Scala File Format

An alternative tuning method is to upload scl and kbm files to your synthesizer. See the scl and kbm sections below for more information.

### Approximate Ratios

The `dump` command provides information about the qualities of a scale. Let's have a look at the 19-EDO scale:

```bash
dump ref-note 62 --lo-key 62 --up-key 69 steps 1:19:2
```

The output reveals that some rational intervals are well approximated. Especially the just minor third (6/5) which is approximated by less than than 1¢ and, therefore, displayed as 0¢:

```rust
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
>  62 | IDX    0 |  1/1    +0¢  +0o ‖     293.665 Hz ‖   62 |      D  4 |   +0.000¢
   63 | IDX    1 |  1/1   +63¢  +0o ‖     304.576 Hz ‖   63 |  D#/Eb  4 |  -36.842¢
   64 | IDX    2 | 12/11  -24¢  +0o ‖     315.892 Hz ‖   63 |  D#/Eb  4 |  +26.316¢
   65 | IDX    3 | 10/9    +7¢  +0o ‖     327.629 Hz ‖   64 |      E  4 |  -10.526¢
   66 | IDX    4 |  7/6   -14¢  +0o ‖     339.803 Hz ‖   65 |      F  4 |  -47.368¢
   67 | IDX    5 |  6/5    +0¢  +0o ‖     352.428 Hz ‖   65 |      F  4 |  +15.789¢
   68 | IDX    6 |  5/4    -7¢  +0o ‖     365.522 Hz ‖   66 |  F#/Gb  4 |  -21.053¢
```

The ratio approximation algorithm is not very advanced yet and does not use prime numbers.

### Compare Scales

Imagine, you want to know how well quarter-comma meantone is represented in 31-EDO. All you need to do is create the quarter-comma meantone scale (`tune scale`) and `tune diff` it against the 31-EDO scale.

In quarter-comma meantone the fifths are tempered in such a way that four of them match up a frequency ratio of 5. This makes the genator of the scale equal to 5^(1/4) or `1:4:5` in `tune` expression notation. To obtain a full scale, let's say ionian/major, you need to walk 5 generators/fifths upwards and one downwards which translates to the scale expression `rank2 1:4:5 5 1`.

The scale expression for the 31-EDO scale is `steps 1:31:2`, s.t. the full scale comparison command becomes:

```bash
tune scale ref-note 62 rank2 1:4:5 5 1 | tune diff 62 steps 1:31:2
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

### Equal-step tuning analysis

The `tune est` command prints basic information about any equal-step tuning. The step sizes and sharp values are derived based on the arithmetics of meantone tuning.

Example output of `tune est 1:17:2`:

```rust
---- Properties of 17-EDO (Meantone) ----

Number of cycles: 1
1 fifth = 10 EDO steps = +705.9c (pythagorean +3.9c)
1 primary step = 3 EDO steps
1 secondary step = 1 EDO steps
1 sharp = 2 EDO steps

-- Val (13-limit) --
[17, 27, 39, 48, 59, 63]

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

The [Scala scale file format](http://www.huygens-fokker.org/scala/scl_format.html) defines a scale in terms of relative pitches. It does not reveal any information about the root pitch of a scale.

* Equal temperament
  ```bash
  tune scl steps --help      # Print help for the `steps` subcommand
  tune scl steps 1:12:2      # 12-EDO
  tune scl steps 100c        # 12-EDO
  tune scl steps 1:36:2      # Sixth-tone
  tune scl steps '(100/3)c'  # Sixth-tone
  tune scl steps 1:13:3      # Bohlen-Pierce
  ```

* Meantone temperament
  ```bash
  tune scl rank2 --help      # Print help for the `rank2` subcommand
  tune scl rank2 3/2 6       # Pythagorean (lydian)
  tune scl rank2 1.5 6 6     # Pythagorean (12-note)
  tune scl rank2 1:4:5 5 1   # quarter-comma meantone (major)
  tune scl rank2 18:31:2 3 3 # 31-EDO meantone (dorian)
  ```

* Harmonic series
  ```bash
  tune scl harm --help       # Print help for the `harm` subcommand
  tune scl harm 8            # 8:9:10:11:12:13:14:15:16 scale
  tune scl harm --sub 8      # ¹/₁₆:¹/₁₅:¹/₁₄:¹/₁₃:¹/₁₂:¹/₁₁:¹/₁₀:¹/₉:¹/₈ scale
  ```

* Imported scale
  ```bash
  tune scl import --help       # Print help for the `import` subcommand
  tune scl import my_scale.scl # Import the
  ```

* Name the scale
  ```bash
  tune scl --name "Just intonation" steps 9/8 5/4 4/3 3/2 5/3 15/8 2
  ```

* Write the scale to a file
  ```bash
  tune --of edo-22.scl scl steps 1:22:2
  ```

### Steps Syntax

Ordered by precedence:

1. `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
1. `<num>/<denom>` evaluates to `num/denom`
1. `<cents>c` evaluates to `2^(cents/1200)`
1. `(<expr>)` evaluates to `expr`

## Create kbm Files / Keyboard Mapping Expressions

[Keyboard mappings](http://www.huygens-fokker.org/scala/help.htm#mappings) define roots and reference pitches of microtonal scales. In general, the format allows for mapping several MIDI notes to the same or no pitch. `tune-cli`, however, only has support for linear scales at the moment.

* Print help for the `kbm` subcommand
  ```bash
  tune kbm --help
  ```

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
  tune kbm --root 60 69@450Hz
  ```

* Write the keyboard mapping to a file
  ```bash
  tune --of root-at-d4.kbm kbm 62
  ```

## YAML Output

`tune` uses YAML as an explicit scale format. You can use `tune`'s output as an input for an external application or the other way around. It is possible to export a scale first, then modify it and, finally use it as in input parameter for another `tune` command.

### Example Usage

```bash
tune scale ref-note 62 --lo-key 61 --up-key 64 steps 1:7:2
```
**Output**

```yml
---
Scale:
  root_key_midi_number: 62
  root_pitch_in_hz: 293.6647679174076
  items:
    - key_midi_number: 61
      pitch_in_hz: 265.9791296633641
    - key_midi_number: 62
      pitch_in_hz: 293.6647679174076
    - key_midi_number: 63
      pitch_in_hz: 324.23219079306349
```

