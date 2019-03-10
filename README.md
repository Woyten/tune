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

## Dump pitches of a scale

* 7-note Pythagorean (minor mode)
  ```bash
  tune dump rank2 3/2 3 4
  ```
  **Output:**
  ```bash
  ...
  69 | 440.000 Hz | MIDI 69 | A     5
  70 | 495.000 Hz | MIDI 71 | B     5 | +3.910c
  71 | 521.481 Hz | MIDI 72 | C     6 | -5.865c
  72 | 586.667 Hz | MIDI 74 | D     6 | -1.955c
  73 | 660.000 Hz | MIDI 76 | E     6 | +1.955c
  74 | 695.309 Hz | MIDI 77 | F     6 | -7.820c
  75 | 782.222 Hz | MIDI 79 | G     6 | -3.910c
  76 | 880.000 Hz | MIDI 81 | A     6
  ...
  ```

## Expressions

Ordered by precedence:

1. `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
1. `<num>/<denom>` evaluates to `num/denom`
1. `<cents>c` evaluates to `2^(cents/1200)`
1. `{<expr>}` evaluates to `expr`
