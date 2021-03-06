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

```bash
tune dump ref-note 62 --lo-key 61 --up-key 71 steps 1:7:2
```

This instructs `tune` to print the frequencies and approximate notes of a 7-EDO scale starting at D4 (MIDI number 62). Output:

```
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

## MIDI Tuning Standard

If you do not want to retune your electric piano manually you can instruct `tune-cli` to send a MIDI Tuning Standard (MTS) message to your synthesizer. To do so, locate your target MIDI device first:

```bash
tune devices
```

This will list all available MIDI devices:

```
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

Moreover, the command will print the tuning message to `stdout`:

```
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

The Scale/Octave Tuning message is of very limited use: It can only slightly detune the 12 note letters within an octave which means that it is impossible to squeeze more than 12 notes into an octave or to model a non-octave-based tuning like Bohlen-Pierce or a stretched EDO.

To overcome this limitation, synthesizers can respond to the *Single Note Tuning Change* message. It provides full control over the pitch of each individual MIDI note s.t. any tuning scenario becomes achievable. Unfortunately, many synthesizers do not respond to this tuning message.

To send a Single Note Tuning Change message to a synthesizer use:

```bash
tune mts --send-to 1 full ref-note 62 steps 1:7:2
```

Output:

```
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

Some notes are reported to be out of range. This is because 7-EDO has a stronger per-step increase in frequency than 12-EDO does s.t. some (inaudible) frequencies become unmappable.

### Keyboard Mappings

Unlike the octave-based mapping, the full keybord mapping by default maps adjacent keys to adjacent degrees of your tuning. For 7-EDO, however, it would be convenient to skip/ignore the black keys in the mapping.

To specify a white-key-only keyboard mapping use the following syntax:

```bash
tune mts --send-to 1 full ref-note 62 --key-map 0,x,1,2,x,3,x,4,x,5,6,x --octave 7 steps 1:7:2
```

The `--key-map` parameter specifies that key D is mapped to degree 0, key D# is unmapped, E is mapped to degree 1, F is mapped to degree 2 and so on. The parameter `--octave` tells us that the 12th keyboard degree (D plus one octave) should be mapped to scale degree 7 (one octave in 7-EDO).

## Live Retuning

The risk is high that you are not satisfied with your synth's tuning capabilities because:

- Your synth supports Single Note Tuning Change messages but it selects the sound sample based on the MIDI note number and not on the desired pitch (Slow-motion or time-lapse effect &ndash; sad but true!)
- Your synth has Scale/Octave Tuning support but you need more than 12 notes in an octave and/or your tuning isn't octave-based
- Your synth has no MTS support at all

The *Live Retuning* feature is where `tune-cli` shines. `tune-cli` can apply a couple of workarounds to make even a very basic keyboard with a pitch-bend wheel play Bohlen-Pierce scales.

This, of course comes, at some cost. Your virtual instrument will either consume multiple MIDI channels instead of only one or you have to accept that simultaneously played notes can get in a conflict situation.

To understand what live retuning does, have a look at the CLI help of the `live` subcommand:

```bash
tune live --help
```

### Ahead-of-Time Live Retuning

The following command enables 31-EDO *ahead-of-time live retuning* with Scale/Octave tuning messages:

```bash
tune live --midi-in 0 --midi-out 0 aot octave ref-note 62 steps 1:31:2
```

Example Output:

```
Receiving MIDI data from Midi Through:Midi Through Port-0 14:0
Sending MIDI data to Midi Through:Midi Through Port-0 14:0
in-channel 0 -> out-channels [0..3)
```

The term "ahead-of-time" reflects the fact that several channels will be retuned in a first stage where the number of MIDI channels is fixed and depends on the selected tuning and tuning method (`tune live aot --help` for more info). In our case, 3 channels (0, 1 and 2) are used. Note that `tune-cli` uses 0-based channels and right-exclusive ranges &ndash; a convention which effectively avoids programming errors.

The second stage is the live performance stage. No further tuning message will be sent. Instead, each incoming MIDI message will be transformed into another message or a batch of outgoing MIDI messages on the channels that have the appropriate tuning applied.

Ahead-of-time live retuning always allocates enough channels s.t. any combination of notes can be played simultaneously.

### Just-in-Time Live retuning

If you want to allocate fewer channels than `aot` does (let's say two instead of three) you can apply *just-in-time live retuning*:

```bash
tune live --midi-in 0 --midi-out 0 jit --out-chans 2 octave ref-note 62 steps 1:31:2
```

Example Output:

```
Receiving MIDI data from Midi Through:Midi Through Port-0 14:0
Sending MIDI data to Midi Through:Midi Through Port-0 14:0
in-channel 0 -> out-channels [0..2)
```

On the surface, `jit` just looks very similar to `aot`. However, there is a big difference in its implementation: While `aot` uses a fixed mapping with a fixed number of channels, `jit` uses a dynamic mapping that gets updated whenever a new note is triggered.

In the given example we decided to use two `jit` channels instead of three `aot` channels. This means some combinations of three notes cannot be played simultaneously in the correct tuning. Although this sounds like a hard limitation, in our case it isn't. The reason is that in order for a clash of three notes to occur, all notes must map to the same note letter. This would be the case for the notes 61, 62 and 63, all of which are an 31-EDO-step apart. Usually, the limitation only comes into play when a very dissonant note cluster is pressed.


### Whole Channel Live Retuning

If your synthesizer has no support for complex tuning messages at all chances are that your synth understands one of the following message types:

- Channel Fine Tuning message
- Pitch-bend message

The above messages have an effect on all notes in a channel. This means, when your tuning contains *m* different deviations from 12-EDO, the corresponding `aot` live retuning command will allocate *m* channels. 16-EDO has 4 different deviations from 12-EDO s.t. the `aot` command works reasonably well:


```bash
tune live --midi-in 0 --midi-out 0 aot channel ref-note 62 steps 1:16:2
tune live --midi-in 0 --midi-out 0 aot pitch-bend ref-note 62 steps 1:16:2
```

Example Output:

```
Receiving MIDI data from Midi Through:Midi Through Port-0 14:0
Sending MIDI data to Midi Through:Midi Through Port-0 14:0
in-channel 0 -> out-channels 0..4
```

In general, the number of `aot` channels can grow quite large as is the case for 17-EDO. In that case, use `jit`.

```bash
tune live --midi-in 0 --midi-out 0 jit --out-chans 8 channel ref-note 62 steps 1:17:2
tune live --midi-in 0 --midi-out 0 jit --out-chans 8 pitch-bend ref-note 62 steps 1:17:2
```

In the whole-channel tuning scenario `--out-chans` can be directly associated with the degree of polyphony.

### What Tuning Method Should I Use?

It is completely up to you to set the balance between channel consumption and tuning conflict prevention. The rules of thumb are:

- More advanced tuning features of your synth &rArr; Less channels/conflicts
- Simpler tuning (octave-based, shares some intervals with with 12-EDO) &rArr; Less channels/conflicts
- Less keys to map &rArr; Less channels/conflicts
- More channels &rArr; Less conflicts
- Less conflicts &rArr; Better polyphony

Tips:

- Prefer `aot/jit full` over `aot/jit octave`.
- Prefer `aot/jit octave` over `aot/jit channel`.
- Prefer `aot/jit channel` over `aot/jit pitch-bend`.
- When `aot full/octave` allocates more than 3 channels: Consider using `jit` with `--out-chans=3`.
- But before: Check if excluding keys (`ref-note --lo-key/--up-key/--key-map` / YAML scale) is an option.
- You only benefit from `jit` if you select less channels than `aot` would use.
- `aot channel/pitch-bend` works well for *n*-EDOs where gcd(*n*, 12) is large.
- `aot channel/pitch-bend` can work for ED1900cents (quasi-EDTs) e.g. `steps 1:13:1900c`.
- `jit` will always work in some way. Configure your polyphony options with the `--out-chans` and `--clash` parameters.

## Scala File Format

An alternative tuning method, mostly on software-based synhesizes, is to upload an scl and kbm file to your synthesizer.

### Create scl Files / Scale Expressions

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

#### Steps Syntax

Ordered by precedence:

1. `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
1. `<num>/<denom>` evaluates to `num/denom`
1. `<cents>c` evaluates to `2^(cents/1200)`
1. `(<expr>)` evaluates to `expr`

### Create kbm Files / Keyboard Mapping Expressions

[Keyboard mappings](http://www.huygens-fokker.org/scala/help.htm#mappings) specify the roots and reference pitches of microtonal scales. In addition, the format defines a mapping between (physical) keys and the scale degree to use for the given key. If no such mapping is provided a linear mapping is used as a default.

* Print help for the `kbm` subcommand
  ```bash
  tune kbm ref-note --help
  ```

* Start scale at C4 at its usual frequency
  ```bash
  tune kbm ref-note 60
  ```

* Start scale at C4, 20 cents higher than usual
  ```bash
  tune kbm ref-note 60+20c
  ```

* Start scale at A4 at 450 Hz
  ```bash
  tune kbm ref-note 69@450Hz
  ```

* Start scale at C4, A4 should sound at 450 Hz
  ```bash
  tune kbm ref-note 69@450Hz --root 60
  ```

* Start scale at C4, use D4 as a reference note, white keys only
  ```bash
  tune kbm ref-note 62 --root 60 --key-map 0,x,1,x,2,3,x,4,x,5,x,6 --octave 7
  ```

* Write the keyboard mapping to a file
  ```bash
  tune --of root-at-d4.kbm kbm ref-note 62
  ```

## Tuning Analysis

### Approximate Ratios

The `dump` command provides information about the qualities of a scale. Let's have a look at the 19-EDO scale:

```bash
dump ref-note 62 --lo-key 62 --up-key 69 steps 1:19:2
```

The output reveals that some rational intervals are well approximated. Especially the just minor third (6/5) which is approximated by less than than 1¢ and, therefore, displayed as 0¢:

```
  ----------Source Scale----------- ‖ ----Pitch----- ‖ --------Target Scale--------
   61 | IDX   -1 |  2/1   -63¢  -1o ‖     283.145 Hz ‖   61 |  C#/Db  4 |  +36.842¢
>  62 | IDX    0 |  1/1    +0¢  +0o ‖     293.665 Hz ‖   62 |      D  4 |   +0.000¢
   63 | IDX    1 |  1/1   +63¢  +0o ‖     304.576 Hz ‖   63 |  D#/Eb  4 |  -36.842¢
   64 | IDX    2 | 12/11  -24¢  +0o ‖     315.892 Hz ‖   63 |  D#/Eb  4 |  +26.316¢
   65 | IDX    3 | 10/9    +7¢  +0o ‖     327.629 Hz ‖   64 |      E  4 |  -10.526¢
   66 | IDX    4 |  7/6   -14¢  +0o ‖     339.803 Hz ‖   65 |      F  4 |  -47.368¢
   67 | IDX    5 |  6/5    +0¢  +0o ‖     352.428 Hz ‖   65 |      F  4 |  +15.789¢
   68 | IDX    6 |  5/4    -7¢  +0o ‖     365.522 Hz ‖   66 |  F#/Gb  4 |  -21.053¢
   69 | IDX    7 |  9/7    +7¢  +0o ‖     379.103 Hz ‖   66 |  F#/Gb  4 |  +42.105¢
   70 | IDX    8 |  4/3    +7¢  +0o ‖     393.189 Hz ‖   67 |      G  4 |   +5.263¢
```

The ratio approximation algorithm is not very advanced yet and does not use prime numbers.

### Compare Scales

Imagine, you want to know how well quarter-comma meantone is represented in 31-EDO. All you need to do is create the quarter-comma meantone scale (`tune scale`) and `tune diff` it against the 31-EDO scale.

In quarter-comma meantone the fifths are tempered in such a way that four of them match up a frequency ratio of 5. This makes the genator of the scale equal to 5^(1/4) or `1:4:5` in `tune` expression notation. To obtain a full scale, let's say ionian/major, you need to walk 5 generators/fifths upwards and one downwards which translates to the scale expression `rank2 1:4:5 5 1`.

The scale expression for the 31-EDO scale is `steps 1:31:2`, s.t. the full scale comparison command becomes:

```bash
tune scale ref-note 62 --lo-key 61 --up-key 71 rank2 1:4:5 5 1 | tune diff stdin ref-note 62 steps 1:31:2
```

This will print:

```
==== Properties of 17-EDO ====

-- Patent val (13-limit) --
val: <17, 27, 39, 48, 59, 63|
errors (absolute): [+0.0c, +3.9c, -33.4c, +19.4c, +13.4c, +6.5c]
errors (relative): [+0.0%, +5.6%, -47.3%, +27.5%, +19.0%, +9.3%]
TE simple badness: 55.915‰
subgroup: 2.3.7.11.13

== Meantone notation ==

-- Step sizes --
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

