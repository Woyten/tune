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
        serde_json::from_reader(file).map_err(|err| {
            CliError::CommandError(format!("Could not deserialize JSON file: {}", err))
        })
    } else {
        println!(
            "[INFO] Waveforms file not found. Creating `{}`",
            location.display()
        );
        let waveforms = get_builtin_waveforms();
        let file = File::create(location)?;
        serde_json::to_writer_pretty(file, &waveforms).map_err(|err| {
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
                frequency: 1.0,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: 1.0,
                },
            })],
        },
        WaveformSpec {
            name: "Sine³".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin3,
                frequency: 1.0,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: 1.0,
                },
            })],
        },
        WaveformSpec {
            name: "Clipped Sine".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Clip { limit: 0.5 },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Triangle,
                frequency: 1.0,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: 1.0,
                },
            })],
        },
        WaveformSpec {
            name: "Triangle³".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Pow3,
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Square".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Square,
                frequency: 1.0,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: 1.0 / 4.0,
                },
            })],
        },
        WaveformSpec {
            name: "Sawtooth".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sawtooth,
                frequency: 1.0,
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: 1.0 / 2.0,
                },
            })],
        },
        WaveformSpec {
            name: "Fat Sawtooth 1".to_owned(),
            envelope_type: EnvelopeType::Organ,
            stages: vec![
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: 1.0 / 1.005,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 4.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: 1.005,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 4.0,
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
                    frequency: 1.0 / 1.005,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 4.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sawtooth,
                    frequency: 2.0 * 1.005,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 4.0,
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
                    frequency: 2.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 440.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 440.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 880.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Triangle,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 440.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 8.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 2.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -4.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 4.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 2.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 8.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -1.0 / 15.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 8.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 2.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -4.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 4.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 2.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 6.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -1.0 / 15.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 8.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 2.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -4.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 4.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 2.0 / 15.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 8.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -1.0 / 15.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 440.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 440.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 880.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 880.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 1.0,
                    modulation: Modulation::ByFrequency {
                        source: Source::Buffer0,
                        normalization_in_hz: 4400.0,
                    },
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 2.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 16.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 3.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -8.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 5.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 4.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 7.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -2.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 9.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 31.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 16.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 2.9966,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -8.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 5.0394,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 4.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 7.1272,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: -2.0 / 31.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin3,
                    frequency: 8.9797,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0 / 31.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 1.5,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer1,
                        intensity: 1.0,
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer0, Source::Buffer1),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0,
                    },
                }),
                StageSpec::Oscillator(Oscillator {
                    kind: OscillatorKind::Sin,
                    frequency: 2.5,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer1,
                        intensity: 1.0,
                    },
                }),
                StageSpec::RingModulator(RingModulator {
                    sources: (Source::Buffer0, Source::Buffer1),
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0 / 2.0,
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass {
                        cutoff: LfSource::Slope {
                            from: 0.0,
                            to: 10.0,
                            change_per_s: 5.0,
                        },
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
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
                    frequency: 1.0,
                    modulation: Modulation::None,
                    destination: Destination {
                        buffer: OutBuffer::Buffer0,
                        intensity: 1.0 / 2.0,
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::Resonance {
                        cutoff: LfSource::Slope {
                            from: 1.0,
                            to: 32.0,
                            change_per_s: 16.0,
                        },
                        damping: LfSource::Value(0.2),
                    },
                    source: Source::Buffer0,
                    destination: Destination {
                        buffer: OutBuffer::AudioOut,
                        intensity: 1.0,
                    },
                }),
            ],
        },
    ]
}
