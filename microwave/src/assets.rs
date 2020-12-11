use std::{fs::File, path::PathBuf};

use tune_cli::{CliError, CliResult};

use crate::waveform::{
    Destination, EnvelopeType, Filter, FilterKind, LfSource, Modulation, Oscillator,
    OscillatorKind, OutBuffer, RingModulator, Source, StageSpec, WaveformSpec,
};

pub fn load_waveforms(location: &PathBuf) -> CliResult<Vec<WaveformSpec>> {
    if location.as_path().exists() {
        println!("[INFO] Loading waveforms file `{}`", location.display());
        let file = File::open(location)?;
        serde_yaml::from_reader(file).map_err(|err| {
            CliError::CommandError(format!("Could not deserialize JSON file: {}", err))
        })
    } else {
        println!(
            "[INFO] Waveforms file not found. Creating `{}`",
            location.display()
        );
        let waveforms = get_builtin_waveforms();
        let file = File::create(location)?;
        serde_yaml::to_writer(file, &waveforms).map_err(|err| {
            CliError::CommandError(format!("Could not serialize JSON file: {}", err))
        })?;
        Ok(waveforms)
    }
}

fn get_builtin_waveforms() -> Vec<WaveformSpec> {
    vec![
        WaveformSpec {
            name: "Sine".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::WaveformPitch,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sine³".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin3,
                frequency: LfSource::WaveformPitch,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Clipped Sine".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Clip {
                        limit: LfSource::Value(0.5),
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Triangle,
                frequency: LfSource::WaveformPitch,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            })],
        },
        WaveformSpec {
            name: "Triangle³".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Pow3,
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Square".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Square,
                frequency: LfSource::WaveformPitch,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0 / 4.0),
                },
            })],
        },
        WaveformSpec {
            name: "Sawtooth".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sawtooth,
                frequency: LfSource::WaveformPitch,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0 / 2.0),
                },
            })],
        },
        WaveformSpec {
            name: "Fat Sawtooth 1".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.0 / 1.005) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.005) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Fat Sawtooth 2".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.0 / 1.005) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(2.0 * 1.005) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 4.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Chiptune".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 1".to_owned(),
            envelope_type: EnvelopeType::Piano,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 2".to_owned(),
            envelope_type: EnvelopeType::Piano,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Clavinet".to_owned(),
            envelope_type: EnvelopeType::Piano,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 1".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(8.0) * LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 2".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch * LfSource::Value(2.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch * LfSource::Value(4.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch * LfSource::Value(6.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Pipe Organ".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(2.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(4.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(8.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-1.0 / 15.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Brass".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Oboe".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Oscillator {
                        kind: OscillatorKind::Sin,
                        phase: 0.0,
                        frequency: LfSource::Value(5.0).into(),
                        baseline: LfSource::Value(1.0).into(),
                        amplitude: LfSource::Time {
                            start: LfSource::Value(0.0).into(),
                            end: LfSource::Value(2.0).into(),
                            from: LfSource::Value(0.0).into(),
                            to: LfSource::Value(0.01).into(),
                        }
                        .into(),
                    } * LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Sax".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bagpipes".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Distortion".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(4400.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::ByFrequency(Source::Buffer0),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 2.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 1".to_owned(),
            envelope_type: EnvelopeType::Bell,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(3.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(5.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(7.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(9.0),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 31.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 2 (12-EDO)".to_owned(),
            envelope_type: EnvelopeType::Bell,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(2.9966),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(5.0394),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(7.1272),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch * LfSource::Value(8.9797),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0 / 31.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 1".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch * LfSource::Value(1.5),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer1,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer0, Source::Buffer1),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 2".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::WaveformPitch * LfSource::Value(2.5),

                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer1,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer0, Source::Buffer1),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Bright Pad".to_owned(),
            envelope_type: EnvelopeType::Pad,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass {
                        cutoff: LfSource::Time {
                            start: LfSource::Value(0.0).into(),
                            end: LfSource::Value(2.0).into(),
                            from: LfSource::Value(0.0).into(),
                            to: LfSource::Value(10.0).into(),
                        } * LfSource::WaveformPitch,
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Resonance Pad".to_owned(),
            envelope_type: EnvelopeType::Pad,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Resonance {
                        resonance: LfSource::Time {
                            start: LfSource::Value(0.0).into(),
                            end: LfSource::Value(2.0).into(),
                            from: LfSource::Value(1.0).into(),
                            to: LfSource::Value(32.0).into(),
                        } * LfSource::WaveformPitch,
                        damping: LfSource::Value(0.2),
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle Harp".to_owned(),
            envelope_type: EnvelopeType::Bell,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: LfSource::WaveformPitch,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::HighPass {
                        cutoff: LfSource::Time {
                            start: LfSource::Value(0.0).into(),
                            end: LfSource::Value(200.0).into(),
                            from: LfSource::Value(1.0).into(),
                            to: LfSource::Value(1000.0).into(),
                        } * LfSource::WaveformPitch,
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: LfSource::Value(1.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Audio-in".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Filter(Filter {
                kind: FilterKind::Resonance {
                    resonance: LfSource::WaveformPitch,
                    damping: LfSource::Value(0.01),
                },
                source: Source::AudioIn,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(0.25),
                },
            })],
        },
    ]
}
