use std::{fs::File, path::Path};

use tune_cli::{CliError, CliResult};

use crate::{
    control::LiveParameter,
    magnetron::{
        effects::{EchoSpec, EffectSpec, RotarySpeakerSpec, SchroederReverbSpec},
        filter::{Filter, FilterKind, RingModulator},
        oscillator::{Modulation, OscillatorKind, OscillatorSpec},
        signal::{SignalKind, SignalSpec},
        source::{LfSource, LfSourceExpr},
        waveguide::{Reflectance, WaveguideSpec},
        AudioSpec, EnvelopeSpec, InBufferSpec, OutBufferSpec, OutSpec, StageSpec, WaveformProperty,
        WaveformSpec,
    },
};

pub fn load_waveforms(location: &Path) -> CliResult<AudioSpec> {
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

pub fn get_builtin_waveforms() -> AudioSpec {
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
            stages: vec![StageSpec::Oscillator(OscillatorSpec {
                kind: OscillatorKind::Sin,
                frequency: WaveformProperty::WaveformPitch.wrap(),
                phase: None,
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
            stages: vec![StageSpec::Oscillator(OscillatorSpec {
                kind: OscillatorKind::Sin3,
                frequency: WaveformProperty::WaveformPitch.wrap(),
                phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
            stages: vec![StageSpec::Oscillator(OscillatorSpec {
                kind: OscillatorKind::Triangle,
                frequency: WaveformProperty::WaveformPitch.wrap(),
                phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
            stages: vec![StageSpec::Oscillator(OscillatorSpec {
                kind: OscillatorKind::Square,
                frequency: WaveformProperty::WaveformPitch.wrap(),
                phase: None,
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
            stages: vec![StageSpec::Oscillator(OscillatorSpec {
                kind: OscillatorKind::Sawtooth,
                frequency: WaveformProperty::WaveformPitch.wrap(),
                phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(0.995) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(1.005) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(0.995) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: LfSource::Value(2.0 * 1.005)
                        * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(1.0 / 4.0),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Expressive Sawtooth (KeyPressure vor color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass2 {
                        resonance: LfSourceExpr::Linear {
                            input: WaveformProperty::KeyPressure.wrap(),
                            from: LfSource::Value(500.0),
                            to: LfSource::Value(10000.0),
                        }
                        .wrap(),
                        quality: LfSource::Value(3.0),
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
            name: "Chiptune".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                        resonance: WaveformProperty::WaveformPitch.wrap()
                            * LfSourceExpr::Time {
                                start: LfSource::Value(0.0),
                                end: LfSource::Value(0.1),
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(8.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(4.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(6.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(8.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-4.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(4.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(2.0 / 15.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(440.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap()
                        * LfSourceExpr::Oscillator {
                            kind: OscillatorKind::Sin,
                            frequency: LfSource::Value(5.0),
                            phase: None,
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
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Linear {
                            input: WaveformProperty::Velocity.wrap(),
                            from: LfSource::Value(220.0),
                            to: LfSource::Value(880.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(880.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(4400.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(3.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(9.0) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(16.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(2.9966) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-8.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(5.0394) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(4.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(7.1272) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::audio_out(),
                        out_level: LfSource::Value(-2.0 / 31.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: LfSource::Value(8.9797) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: WaveformProperty::WaveformPeriod.wrap(),
                            end: WaveformProperty::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                            start: WaveformProperty::WaveformPeriod.wrap(),
                            end: WaveformProperty::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSourceExpr::Time {
                            start: WaveformProperty::WaveformPeriod.wrap(),
                            end: WaveformProperty::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                            start: WaveformProperty::WaveformPeriod.wrap(),
                            end: WaveformProperty::WaveformPeriod.wrap(),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    },
                }),
                StageSpec::Waveguide(WaveguideSpec {
                    buffer_size: 4096,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    cutoff: LfSource::Value(2500.0)
                        + LfSource::Value(5.0) * WaveformProperty::WaveformPitch.wrap(),
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
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                        resonance: LfSource::Value(4.0) * WaveformProperty::WaveformPitch.wrap(),
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                    frequency: WaveformProperty::WaveformPitch.wrap(),
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(1.5) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin3,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sin,
                    frequency: LfSource::Value(2.5) * WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass {
                        cutoff: WaveformProperty::WaveformPitch.wrap()
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Sawtooth,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0 / 2.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::LowPass2 {
                        resonance: WaveformProperty::WaveformPitch.wrap()
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
                StageSpec::Oscillator(OscillatorSpec {
                    kind: OscillatorKind::Triangle,
                    frequency: WaveformProperty::WaveformPitch.wrap(),
                    phase: None,
                    modulation: Modulation::None,
                    out_spec: OutSpec {
                        out_buffer: OutBufferSpec::Buffer(0),
                        out_level: LfSource::Value(1.0),
                    },
                }),
                StageSpec::Filter(Filter {
                    kind: FilterKind::HighPass {
                        cutoff: WaveformProperty::WaveformPitch.wrap()
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
                frequency: WaveformProperty::WaveformPitch.wrap(),
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

    let effects = vec![
        EffectSpec::Echo(EchoSpec {
            buffer_size: 100000,
            gain: LfSourceExpr::Controller {
                kind: LiveParameter::Sound7,
                from: LfSource::Value(0.0),
                to: LfSource::Value(1.0),
            }
            .wrap(),
            delay_time: LfSource::Value(0.5),
            feedback: LfSource::Value(0.6),
            feedback_rotation: LfSource::Value(135.0),
        }),
        EffectSpec::SchroederReverb(SchroederReverbSpec {
            buffer_size: 100000,
            gain: LfSourceExpr::Controller {
                kind: LiveParameter::Sound8,
                from: LfSource::Value(0.0),
                to: LfSource::Value(0.5),
            }
            .wrap(),
            allpasses: vec![
                LfSource::Value(5.10),
                LfSource::Value(7.73),
                LfSource::Value(10.00),
                LfSource::Value(12.61),
            ],
            allpass_feedback: LfSource::Value(0.5),
            combs: vec![
                (LfSource::Value(25.31), LfSource::Value(25.83)),
                (LfSource::Value(26.94), LfSource::Value(27.46)),
                (LfSource::Value(28.96), LfSource::Value(29.48)),
                (LfSource::Value(30.75), LfSource::Value(31.27)),
                (LfSource::Value(32.24), LfSource::Value(32.76)),
                (LfSource::Value(33.81), LfSource::Value(34.33)),
                (LfSource::Value(35.31), LfSource::Value(35.83)),
                (LfSource::Value(36.67), LfSource::Value(37.19)),
            ],
            comb_feedback: LfSource::Value(0.95),
            cutoff: LfSource::Value(5600.0),
        }),
        EffectSpec::RotarySpeaker(RotarySpeakerSpec {
            buffer_size: 100000,
            gain: LfSourceExpr::Controller {
                kind: LiveParameter::Sound9,
                from: LfSource::Value(0.0),
                to: LfSource::Value(0.5),
            }
            .wrap(),
            rotation_radius: LfSource::Value(20.0),
            speed: LfSourceExpr::Controller {
                kind: LiveParameter::Sound10,
                from: LfSource::Value(1.0),
                to: LfSource::Value(7.0),
            }
            .wrap(),
            acceleration: LfSource::Value(7.0),
            deceleration: LfSource::Value(14.0),
        }),
    ];

    AudioSpec {
        envelopes,
        waveforms,
        effects,
    }
}
