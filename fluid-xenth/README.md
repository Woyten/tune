Stop making music with notes. Use pitches.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [API documentation](https://crates.io/crates/fluid-xenth)

# Overview

`fluid-xenth` is a microtonal wrapper around [FluidLite](https://crates.io/crates/fluidlite). It uses the JIT live-retuning concept implemented in [tune](https://crates.io/crates/tune) to enable arbitrary-pitch playback.

# Getting Started

Inspect and run the demo example:

```bash
cargo run --example demo <loction-to-your-soundfont-file>
```

The demo example should create file named `demo.wav`.

# License

This code is licensed under the MIT license. Note, however, that the required Fluidlite library is LGPL-2.1 licensed.