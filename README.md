Create synthesizer tuning files for microtonal scales.

# Installation

```bash
cargo install -f tune
```

# Usage

## Examples

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
  tune scl equal {5/3}:12:2
  tunc scl equal {500/3}c
  ```
* 7-note Pythagorean
  ```bash
  tune scl rank2 3/2 7
  tune scl rank2 1.5 7
  ```
* 7-note quarter-comma meantone
  ```bash
  tune scl rank2 1:4:5 7
  ```
* 8-note Harmonic series
  ```bash
  tune scl harm 8
  ```

## Expressions

* `<num>/<denom>` evaluates to `num/denom`
* `<num>:<denom>:<int>` evaluates to `int^(num/denom)`
* `<cents>c` evaluates to `2^(cents/1200)`
* `{<expr>}` evaluates to `expr`
