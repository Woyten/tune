use std::{fs::File, path::Path};

use tune_cli::{CliError, CliResult};

use crate::{
    magnetron::{
        filter::{Filter, FilterKind, RingModulator},
        oscillator::{Modulation, Oscillator, OscillatorKind},
        signal::{SignalKind, SignalSpec},
        source::{LfSource, LfSourceExpr, LfSourceUnit},
        waveguide::{Reflectance, WaveguideSpec},
        EnvelopeSpec, InBufferSpec, OutBufferSpec, OutSpec, StageSpec, WaveformSpec, WaveformsSpec,
    },
    synth::LiveParameter,
};

pub fn load_waveforms(location: &Path) -> CliResult<WaveformsSpec<LfSource<LiveParameter>>> {
    if location.exists() {
        println!("[INFO] Loading waveforms file `{}`", location.display());
        let file = File::open(location)?;
        serde_yaml::from_reader(file)
            .map_err(|err| CliError::CommandError(format!("Could not deserialize file: {}", err)))
    } else {
        println!(
            "[INFO] Waveforms file not found. Creating `{}`",
            location.display()
        );
        let waveforms = get_builtin_waveforms();
        let file = File::create(location)?;
        serde_yaml::to_writer(file, &waveforms)
            .map_err(|err| CliError::CommandError(format!("Could not serialize file: {}", err)))?;
        Ok(waveforms)
    }
}

pub fn get_builtin_waveforms() -> WaveformsSpec<LfSource<LiveParameter>> {
    let envelopes = vec![
        EnvelopeSpec {
            name: "Organ".to_owned(),
            attack_time: 0.01,
            release_time: 0.01,
            decay_rate: 0.0,
        },
        EnvelopeSpec {
            name: "Piano".to_owned(),
            attack_time: 0.01,
            release_time: 0.25,
            decay_rate: 1.0,
        },
        EnvelopeSpec {
            name: "Pad".to_owned(),
            attack_time: 0.1,
            release_time: 2.0,
            decay_rate: 0.0,
        },
        EnvelopeSpec {
            name: "Bell".to_owned(),
            attack_time: 0.001,
            release_time: 10.0,
            decay_rate: 0.3,
        },
    ];
    let waveforms = vec![
        WaveformSpec {
            name: "Sine".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sine³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin3,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Clipped Sine".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Clip {
                        limit: LfSource::Value(0.5),
                    },
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Triangle,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Triangle³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Pow3,
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Square".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Square,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0 / 4.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sawtooth".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sawtooth,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0 / 2.0),
                },
            })],
        },
        WaveformSpec {
            name: "Fat Sawtooth 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(0.995) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.005) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Fat Sawtooth 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(0.995) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(2.0 * 1.005) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Chiptune".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 1".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 2".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Clavinet".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Funky Clavinet".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(1),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::HighPass2 {
                        quality: LfSource::Value(5.0),
                        resonance: LfSourceUnit::WaveformPitch.wrap()
                            * LfSourceExpr::Envelope {
                                name: "Piano".to_owned(),
                                from: LfSource::Value(2.0),
                                to: LfSource::Value(4.0),
                            }
                            .wrap(),
                    },
                    in_buffer: InBufferSpec::Buffer(1),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(8.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(6.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Pipe Organ".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(4.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Brass".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Oboe".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap()
                        * LfSourceExpr::Oscillator {
                            kind: OscillatorKind::Sin,
                            phase: 0.0,
                            frequency: LfSource::Value(5.0),
                            baseline: LfSource::Value(1.0),
                            amplitude: LfSourceExpr::Time {
                                start: LfSource::Value(0.0),
                                end: LfSource::Value(2.0),
                                from: LfSource::Value(0.0),
                                to: LfSource::Value(0.01),
                            }
                            .wrap(),
                        }
                        .wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Sax".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Velocity {
                            from: LfSource::Value(220.0),
                            to: LfSource::Value(880.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bagpipes".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Distortion".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(4400.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::ByFrequency {
                        mod_buffer: InBufferSpec::Buffer(0),
                    },
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 1".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(3.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(9.0) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 31.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 2 (12-EDO)".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.9966) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0394) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.1272) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.9797) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 31.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Soft Plucked String (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: LfSourceUnit::WaveformPeriod.wrap(),
                            end: LfSourceUnit::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSourceExpr::Controller {
                        kind: LiveParameter::Breath,
                        from: LfSource::Value(2000.0),
                        to: LfSource::Value(5000.0),
                    }
                    .wrap(),
                    reflectance: Reflectance::Negative,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Hard Plucked String (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Signal(SignalSpec {
                    kind: SignalKind::Noise,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: LfSourceUnit::WaveformPeriod.wrap(),
                            end: LfSourceUnit::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSourceExpr::Controller {
                        kind: LiveParameter::Breath,
                        from: LfSource::Value(2000.0),
                        to: LfSource::Value(5000.0),
                    }
                    .wrap(),
                    reflectance: Reflectance::Negative,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Blown Bottle (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Signal(SignalSpec {
                    kind: SignalKind::Noise,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(0.3),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSourceExpr::Controller {
                        kind: LiveParameter::Breath,
                        from: LfSource::Value(2000.0),
                        to: LfSource::Value(5000.0),
                    }
                    .wrap(),
                    reflectance: Reflectance::Negative,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Fretless Bass (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: LfSourceUnit::WaveformPeriod.wrap(),
                            end: LfSourceUnit::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSourceExpr::Controller {
                        kind: LiveParameter::Breath,
                        from: LfSource::Value(2000.0),
                        to: LfSource::Value(5000.0),
                    }
                    .wrap(),
                    reflectance: Reflectance::Positive,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Dulcimer".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Signal(SignalSpec {
                    kind: SignalKind::Noise,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: LfSourceUnit::WaveformPeriod.wrap(),
                            end: LfSourceUnit::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSource::Value(2500.0)
                        + LfSource::Value(5.0) * LfSourceUnit::WaveformPitch.wrap(),
                    reflectance: Reflectance::Positive,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Strings (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Signal(SignalSpec {
                    kind: SignalKind::Noise,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(0.3),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSourceExpr::Controller {
                        kind: LiveParameter::Breath,
                        from: LfSource::Value(2000.0),
                        to: LfSource::Value(6000.0),
                    }
                    .wrap(),
                    reflectance: Reflectance::Positive,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(1),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass2 {
                        resonance: LfSource::Value(4.0) * LfSourceUnit::WaveformPitch.wrap(),
                        quality: LfSource::Value(1.0),
                    },
                    in_buffer: InBufferSpec::Buffer(1),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Clarinet (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            from: LfSource::Value(0.2),
                            to: LfSource::Value(1.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    cutoff: LfSource::Value(5000.0),
                    reflectance: Reflectance::Negative,
                    feedback: LfSource::Value(1.0),
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(0.5),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(1.5) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(1),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    in_buffers: (InBufferSpec::Buffer(0), InBufferSpec::Buffer(1)),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.5) * LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(1),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    in_buffers: (InBufferSpec::Buffer(0), InBufferSpec::Buffer(1)),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bright Pad".to_owned(),
            envelope: "Pad".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass {
                        cutoff: LfSourceUnit::WaveformPitch.wrap()
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0),
                                end: LfSource::Value(2.0),
                                from: LfSource::Value(0.0),
                                to: LfSource::Value(10.0),
                            }
                            .wrap(),
                    },
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Resonance Pad".to_owned(),
            envelope: "Pad".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass2 {
                        resonance: LfSourceUnit::WaveformPitch.wrap()
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0),
                                end: LfSource::Value(2.0),
                                from: LfSource::Value(1.0),
                                to: LfSource::Value(32.0),
                            }
                            .wrap(),
                        quality: LfSource::Value(5.0),
                    },
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle Harp".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceUnit::WaveformPitch.wrap(),
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::HighPass {
                        cutoff: LfSourceUnit::WaveformPitch.wrap()
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0),
                                end: LfSource::Value(200.0),
                                from: LfSource::Value(1.0),
                                to: LfSource::Value(1000.0),
                            }
                            .wrap(),
                    },
                    in_buffer: InBufferSpec::Buffer(0),
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Audio-in".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Waveguide(WaveguideSpec {
                buffer_size: 4096,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                cutoff: LfSourceExpr::Controller {
                    kind: LiveParameter::Breath,
                    from: LfSource::Value(2000.0),
                    to: LfSource::Value(5000.0),
                }
                .wrap(),
                reflectance: Reflectance::Negative,
                feedback: LfSource::Value(1.0),
                in_buffer: InBufferSpec::audio_in(),
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            })],
        },
    ];

    WaveformsSpec {
        envelopes,
        waveforms,
    }
}
