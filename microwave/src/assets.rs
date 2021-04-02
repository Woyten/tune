use std::{fs::File, path::Path};

use tune_cli::{CliError, CliResult};

use crate::{
    magnetron::{
        filter::{Filter, FilterKind, RingModulator},
        oscillator::{Modulation, Oscillator, OscillatorKind, StringSim},
        source::{LfSource, LfSourceExpr, Property},
        spec::{EnvelopeSpec, StageSpec, WaveformSpec, WaveformsSpec},
        waveform::{Destination, OutBuffer, Source},
    },
    synth::SynthControl,
};

pub fn load_waveforms(location: &Path) -> CliResult<WaveformsSpec<SynthControl>> {
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

fn get_builtin_waveforms() -> WaveformsSpec<SynthControl> {
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
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sine³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin3,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Clipped Sine".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Clip {
                        limit: LfSource::Value(0.5),
                    },
                    source: Source::Buffer(0),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Triangle,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Triangle³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Pow3,
                    source: Source::Buffer(0),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Square".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Square,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(1.0 / 4.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sawtooth".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sawtooth,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(1.0 / 2.0),
                },
            })],
        },
        WaveformSpec {
            name: "Fat Sawtooth 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(0.995) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.005) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 4.0),
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
                    frequency: LfSource::Value(0.995) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(2.0 * 1.005) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 4.0),
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
                    frequency: LfSource::Value(2.0) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(8.0) * LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-1.0 / 15.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(6.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-1.0 / 15.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(4.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-1.0 / 15.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::from(LfSourceExpr::WaveformPitch)
                        * LfSourceExpr::Oscillator {
                            kind: OscillatorKind::Sin,
                            phase: 0.0,
                            frequency: LfSource::Value(5.0).into(),
                            baseline: LfSource::Value(1.0).into(),
                            amplitude: LfSource::from(LfSourceExpr::Time {
                                start: LfSource::Value(0.0).into(),
                                end: LfSource::Value(2.0).into(),
                                from: LfSource::Value(0.0).into(),
                                to: LfSource::Value(0.01).into(),
                            })
                            .into(),
                        }
                        .into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSourceExpr::Property {
                            kind: Property::Velocity,
                            from: LfSource::Value(220.0).into(),
                            to: LfSource::Value(880.0).into(),
                        }
                        .into(),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(4400.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::ByFrequency(Source::Buffer(0)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 2.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(3.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(9.0) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 31.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.9966) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0394) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.1272) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.9797) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0 / 31.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Plucked String - Foot for color".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::String(StringSim {
                buffer_size_secs: 0.1,
                frequency: LfSourceExpr::WaveformPitch.into(),
                cutoff: LfSourceExpr::Property {
                    kind: Property::Velocity,
                    from: LfSource::Value(2000.0).into(),
                    to: LfSource::Value(5000.0).into(),
                }
                .into(),
                feedback: LfSource::Value(1.0),
                pluck_location: LfSourceExpr::Control {
                    controller: SynthControl::Foot,
                    from: LfSource::Value(0.0).into(),
                    to: LfSource::Value(0.9).into(),
                }
                .into(),
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(10.0),
                },
            })],
        },
        WaveformSpec {
            name: "Ring Modulation 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(1.5) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(1),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer(0), Source::Buffer(1)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.5) * LfSourceExpr::WaveformPitch.into(),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(1),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer(0), Source::Buffer(1)),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass {
                        cutoff: LfSource::from(LfSourceExpr::WaveformPitch)
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0).into(),
                                end: LfSource::Value(2.0).into(),
                                from: LfSource::Value(0.0).into(),
                                to: LfSource::Value(10.0).into(),
                            }
                            .into(),
                    },
                    source: Source::Buffer(0),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass2 {
                        resonance: LfSource::from(LfSourceExpr::WaveformPitch)
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0).into(),
                                end: LfSource::Value(2.0).into(),
                                from: LfSource::Value(1.0).into(),
                                to: LfSource::Value(32.0).into(),
                            }
                            .into(),
                        quality: LfSource::Value(5.0),
                    },
                    source: Source::Buffer(0),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
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
                    frequency: LfSourceExpr::WaveformPitch.into(),
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer(0),
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::HighPass {
                        cutoff: LfSource::from(LfSourceExpr::WaveformPitch)
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0).into(),
                                end: LfSource::Value(200.0).into(),
                                from: LfSource::Value(1.0).into(),
                                to: LfSource::Value(1000.0).into(),
                            }
                            .into(),
                    },
                    source: Source::Buffer(0),
                    destination: Destination {
                        buffer: OutBuffer::audio_out(),
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Audio-in".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageSpec::Filter(Filter {
                kind: FilterKind::LowPass2 {
                    resonance: LfSourceExpr::WaveformPitch.into(),
                    quality: LfSource::Value(100.0),
                },
                source: Source::audio_in(),
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
                    intensity: LfSource::Value(0.25),
                },
            })],
        },
    ];

    WaveformsSpec {
        envelopes,
        waveforms,
    }
}
