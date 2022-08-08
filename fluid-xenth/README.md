Stop making music with notes. Use pitches.

# Resources

- [Changelog](https://github.com/Woyten/tune/releases)
- [API documentation](https://docs.rs/fluid-xenth)

# Overview

`fluid-xenth` is a microtonal wrapper around [OxiSynth](https://crates.io/crates/oxisynth). It uses the AOT / JIT live-retuning concepts implemented in [tune](https://github.com/Woyten/tune) to enable arbitrary-pitch playback.

# Getting Started

Inspect and run the demo examples:

```bash
cargo run --example demo_aot <location-to-your-soundfont-file>
cargo run --example demo_jit <location-to-your-soundfont-file>
```

The demo examples should create two files named `demo_aot.wav` and `demo_jit.wav`.

# License

This code is licensed under the MIT license. Note, however, that the required OxiSynth library is LGPL-2.1 licensed.