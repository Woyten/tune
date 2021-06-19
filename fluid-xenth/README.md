Stop making music with notes. Use pitches.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [API documentation](https://docs.rs/fluid-xenth)

# Overview

`fluid-xenth` is a microtonal wrapper around [FluidLite](https://crates.io/crates/fluidlite). It uses the JIT live-retuning concept implemented in [tune](https://github.com/Woyten/tune) to enable arbitrary-pitch playback.

# Getting Started

Inspect and run the demo example:

```bash
cargo run --example demo <location-to-your-soundfont-file>
```

The demo example should create a file named `demo.wav`.

# License

This code is licensed under the MIT license. Note, however, that the required FluidLite library is LGPL-2.1 licensed.