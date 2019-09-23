Create synthesizer tuning files for microtonal scales.

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
  tune scl rank2 3/2 7
  tune scl rank2 1.5 7
  ```
* 7-note quarter-comma meantone (major mode)
  ```bash
  tune scl rank2 1:4:5 6 1
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
  tune dump 62 rank2 3/2 4 3
  ```
  **Output:**
  ```bash
  ..
  62 | 293.665 Hz | MIDI 62 | D     4
  63 | 330.373 Hz | MIDI 64 | E     4 | +3.910c
  64 | 348.047 Hz | MIDI 65 | F     4 | -5.865c
  65 | 391.553 Hz | MIDI 67 | G     4 | -1.955c
  66 | 440.497 Hz | MIDI 69 | A     4 | +1.955c
  67 | 495.559 Hz | MIDI 71 | B     4 | +5.865c
  68 | 522.071 Hz | MIDI 72 | C     5 | -3.910c
  69 | 587.330 Hz | MIDI 74 | D     5
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
