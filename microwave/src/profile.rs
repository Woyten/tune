use std::collections::BTreeSet;
use std::iter;

use bevy::prelude::*;
use clap::Parser;
use serde::Deserialize;
use serde::Serialize;
use shlex::Shlex;
use tune::note::Note;
use tune::pitch::Ratio;
use tune::scala::Kbm;
use tune::scala::Scl;
use tune_cli::CliError;
use tune_cli::CliResult;
use tune_cli::shared::error::ResultExt;
use tune_cli::shared::midi::TuningMethod;
use tune_cli::shared::scala::KbmCommand;
use tune_cli::shared::scala::SclCommand;

use crate::audio::AudioInSpec;
use crate::backend::NoteInput;
use crate::control::LiveParameter;
use crate::fluid::FluidSpec;
use crate::magnetron::FragmentSpec;
use crate::magnetron::GeneratorSpec;
use crate::magnetron::GeneratorType;
use crate::magnetron::MergeProcessorSpec;
use crate::magnetron::MergeProcessorType;
use crate::magnetron::ProcessorSpec;
use crate::magnetron::ProcessorType;
use crate::magnetron::StageType;
use crate::magnetron::StereoProcessorSpec;
use crate::magnetron::StereoProcessorType;
use crate::magnetron::effects::EffectSpec;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::filter::FilterSpec;
use crate::magnetron::filter::FilterType;
use crate::magnetron::noise::NoiseSpec;
use crate::magnetron::noise::NoiseType;
use crate::magnetron::oscillator::ModOscillatorSpec;
use crate::magnetron::oscillator::Modulation;
use crate::magnetron::oscillator::OscillatorSpec;
use crate::magnetron::oscillator::OscillatorType;
use crate::magnetron::source::LfSource;
use crate::magnetron::source::LfSourceExpr;
use crate::magnetron::source::NoAccess;
use crate::magnetron::waveform::NamedEnvelopeSpec;
use crate::magnetron::waveform::WaveformProperty;
use crate::magnetron::waveform::WaveformSpec;
use crate::magnetron::waveguide::Reflectance;
use crate::magnetron::waveguide::WaveguideSpec;
use crate::midi::Bank;
use crate::midi::MidiOutSpec;
use crate::pipeline::PipelineStageSpec;
use crate::portable;
use crate::recorder::WavRecorderSpec;
use crate::synth::MagnetronSpec;

#[derive(Deserialize, Serialize)]
pub struct MicrowaveProfile {
    pub scales: Vec<ScaleSpec>,
    pub default_scale: Option<usize>,
    pub num_buffers: usize,
    pub audio_buffers: (usize, usize),
    pub globals: Vec<FragmentSpec<PipelineParam>>,
    pub templates: Vec<FragmentSpec<WaveformParam>>,
    pub envelopes: Vec<NamedEnvelopeSpec<WaveformParam>>,
    pub stages: Vec<PipelineStageSpec>,
    pub color_palette: ColorPalette,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScaleSpec {
    Explicit {
        scl: String,
        kbm: Option<String>,
    },
    Edos {
        edos: EdosRange,
        kbm: Option<String>,
    },
}

#[derive(Deserialize, Serialize)]
pub struct EdosRange {
    pub from: u16,
    pub to: u16,
}

pub type PipelineParam = LfSource<NoAccess, LiveParameter>;
pub type WaveformParam = LfSource<WaveformProperty, LiveParameter>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorPalette {
    pub natural_color: Srgba,
    pub sharp_colors: Vec<Srgba>,
    pub flat_colors: Vec<Srgba>,
    pub enharmonic_colors: Vec<Srgba>,
}

impl Default for ColorPalette {
    fn default() -> Self {
        ColorPalette {
            natural_color: Srgba::rgb(1.0, 1.0, 1.0),
            sharp_colors: vec![
                Srgba::rgb(0.5, 0.0, 1.0),
                Srgba::rgb(0.0, 0.0, 1.0),
                Srgba::rgb(0.0, 0.5, 1.0),
                Srgba::rgb(0.5, 0.5, 1.0),
            ],
            flat_colors: vec![
                Srgba::rgb(0.5, 1.0, 0.0),
                Srgba::rgb(0.0, 1.0, 0.0),
                Srgba::rgb(0.0, 1.0, 0.5),
                Srgba::rgb(0.5, 1.0, 0.5),
            ],
            enharmonic_colors: vec![
                Srgba::rgb(0.0, 0.5, 0.5),
                Srgba::rgb(1.0, 0.5, 0.5),
                Srgba::rgb(1.0, 0.0, 1.0),
                Srgba::rgb(1.0, 0.5, 1.0),
            ],
        }
    }
}

impl MicrowaveProfile {
    pub async fn load(file_name: &str) -> CliResult<Self> {
        if let Some(data) = portable::read_file(file_name).await? {
            log::info!("Loading config file `{file_name}`");
            serde_yaml::from_reader(data).display_err("Could not deserialize file")
        } else {
            log::info!("Config file not found. Creating `{file_name}`");
            let profile = get_default_profile();
            let file = portable::write_file(file_name).await?;
            serde_yaml::to_writer(file, &profile)
                .map(|()| profile)
                .display_err("Could not serialize file")
        }
    }

    pub fn parse_scales(&self) -> CliResult<Vec<(Scl, Kbm)>> {
        let mut scales = Vec::new();

        for scale in &self.scales {
            match scale {
                ScaleSpec::Explicit { scl, kbm } => {
                    let scl_result = parse_scl(scl)?;
                    scales.push((scl_result, parse_kbm(kbm)?));
                }
                ScaleSpec::Edos { edos, kbm } => {
                    let kbm_result = parse_kbm(kbm)?;
                    for num_steps in edos.from..=edos.to {
                        let scl = Scl::builder()
                            .push_ratio(Ratio::octave().divided_into_equal_steps(num_steps))
                            .build()
                            .map_err(|e| format!("{e:?}"))?;
                        scales.push((scl, kbm_result.clone()));
                    }
                }
            }
        }

        Ok(scales)
    }
}

fn parse_scl(scl: &str) -> Result<Scl, CliError> {
    parse_cli_str::<SclCommand>(scl)?.to_scl(None)
}

fn parse_kbm(kbm: &Option<String>) -> Result<Kbm, CliError> {
    match kbm {
        Some(kbm_str) => parse_cli_str::<KbmCommand>(kbm_str)?.to_kbm(),
        None => Ok(Kbm::builder(Note::from_midi_number(62)).build().unwrap()),
    }
}

pub fn get_default_profile() -> MicrowaveProfile {
    let scales = vec![
        ScaleSpec::Edos {
            edos: EdosRange { from: 5, to: 5 },
            kbm: None,
        },
        ScaleSpec::Edos {
            edos: EdosRange { from: 7, to: 31 },
            kbm: None,
        },
        ScaleSpec::Explicit {
            scl: "steps 1/13:3".to_owned(),
            kbm: None,
        },
        ScaleSpec::Explicit {
            scl: "rank2 3/2 3 3".to_owned(),
            kbm: Some("ref-note 62 --key-map 0,x,1,2,x,3,x,4,x,5,6,x --octave 7".to_owned()),
        },
        ScaleSpec::Explicit {
            scl: "harm 8 8".to_owned(),
            kbm: Some("ref-note 60 --key-map 0,x,1,x,2,x,3,4,5,x,6,7 --octave 8".to_owned()),
        },
    ];

    let globals = vec![FragmentSpec {
        name: "AlternatingOctave".to_owned(),
        value: LfSourceExpr::Oscillator {
            kind: OscillatorType::Square,
            frequency: LfSource::Value(16.0),
            phase: None,
            baseline: LfSource::Value(1.5),
            amplitude: LfSource::Value(0.5),
        }
        .wrap(),
    }];

    let templates = vec![
        FragmentSpec {
            name: "WaveformPitch".to_owned(),
            value: LfSourceExpr::Property(WaveformProperty::WaveformPitch).wrap()
                * LfSourceExpr::Semitones(
                    LfSourceExpr::Controller {
                        kind: LiveParameter::PitchBend,
                        map0: LfSource::Value(0.0),
                        map1: LfSource::Value(2.0),
                    }
                    .wrap(),
                )
                .wrap(),
        },
        FragmentSpec {
            name: "WaveformPeriod".to_owned(),
            value: LfSourceExpr::Property(WaveformProperty::WaveformPeriod).wrap()
                * LfSourceExpr::Semitones(
                    LfSourceExpr::Controller {
                        kind: LiveParameter::PitchBend,
                        map0: LfSource::Value(0.0),
                        map1: LfSource::Value(-2.0),
                    }
                    .wrap(),
                )
                .wrap(),
        },
        FragmentSpec {
            name: "Fadeout".to_owned(),
            value: LfSourceExpr::Controller {
                kind: LiveParameter::Damper,
                map0: LfSourceExpr::Property(WaveformProperty::OffVelocitySet).wrap(),
                map1: LfSource::Value(0.0),
            }
            .wrap(),
        },
        FragmentSpec {
            // Total output: -18 dBFS = -6dBFS (pan) - 12dBFS (volume)
            name: "EnvelopeL".to_owned(),
            value: LfSourceExpr::Controller {
                kind: LiveParameter::Pan,
                map0: LfSourceExpr::Property(WaveformProperty::Velocity).wrap(),
                map1: LfSource::Value(0.0),
            }
            .wrap()
                * LfSourceExpr::Controller {
                    kind: LiveParameter::Volume,
                    map0: LfSource::Value(0.0),
                    map1: LfSource::Value(0.25),
                }
                .wrap(),
        },
        FragmentSpec {
            // Total output: -18 dBFS = -6dBFS (pan) - 12dBFS (volume)
            name: "EnvelopeR".to_owned(),
            value: LfSourceExpr::Controller {
                kind: LiveParameter::Pan,
                map0: LfSource::Value(0.0),
                map1: LfSourceExpr::Property(WaveformProperty::Velocity).wrap(),
            }
            .wrap()
                * LfSourceExpr::Controller {
                    kind: LiveParameter::Volume,
                    map0: LfSource::Value(0.0),
                    map1: LfSource::Value(0.25),
                }
                .wrap(),
        },
    ];

    let envelopes = vec![
        NamedEnvelopeSpec {
            name: "Organ".to_owned(),
            spec: EnvelopeSpec {
                in_buffer: 7,
                out_buffers: (0, 1),
                out_levels: Some((
                    LfSource::template("EnvelopeL"),
                    LfSource::template("EnvelopeR"),
                )),
                fadeout: LfSource::template("Fadeout"),
                attack_time: LfSource::Value(0.01),
                decay_rate: LfSource::Value(0.0),
                release_time: LfSource::Value(0.01),
            },
        },
        NamedEnvelopeSpec {
            name: "Piano".to_owned(),
            spec: EnvelopeSpec {
                in_buffer: 7,
                out_buffers: (0, 1),
                out_levels: Some((
                    LfSource::template("EnvelopeL"),
                    LfSource::template("EnvelopeR"),
                )),
                fadeout: LfSource::template("Fadeout"),
                attack_time: LfSource::Value(0.01),
                decay_rate: LfSource::Value(1.0),
                release_time: LfSource::Value(0.25),
            },
        },
        NamedEnvelopeSpec {
            name: "Pad".to_owned(),
            spec: EnvelopeSpec {
                in_buffer: 7,
                out_buffers: (0, 1),
                out_levels: Some((
                    LfSource::template("EnvelopeL"),
                    LfSource::template("EnvelopeR"),
                )),
                fadeout: LfSource::template("Fadeout"),
                attack_time: LfSource::Value(0.1),
                decay_rate: LfSource::Value(0.0),
                release_time: LfSource::Value(2.0),
            },
        },
        NamedEnvelopeSpec {
            name: "Bell".to_owned(),
            spec: EnvelopeSpec {
                in_buffer: 7,
                out_buffers: (0, 1),
                out_levels: Some((
                    LfSource::template("EnvelopeL"),
                    LfSource::template("EnvelopeR"),
                )),
                fadeout: LfSource::template("Fadeout"),
                attack_time: LfSource::Value(0.001),
                decay_rate: LfSource::Value(0.3),
                release_time: LfSource::Value(10.0),
            },
        },
    ];

    let stages = vec![
        PipelineStageSpec::Reset(
            LfSourceExpr::Controller {
                kind: LiveParameter::Sound1,
                map0: LfSource::Value(0.0),
                map1: LfSource::Value(1.0),
            }
            .wrap(),
        ),
        PipelineStageSpec::AudioIn(AudioInSpec {
            out_buffers: (12, 13),
            out_levels: None,
        }),
        PipelineStageSpec::Magnetron(get_default_magnetron_spec()),
        PipelineStageSpec::Fluid(FluidSpec {
            note_input: NoteInput::Foreground,
            out_buffers: (0, 1),
            out_levels: None,
            soundfont_location: "soundfont.sf2".to_owned(),
            default_program: Some(0),
        }),
        PipelineStageSpec::MidiOut(MidiOutSpec {
            note_input: NoteInput::Foreground,
            out_device: "<midi-device>".to_owned(),
            out_args: Default::default(),
            tuning_method: TuningMethod::Octave1,
            banks: BTreeSet::from([
                Bank { msb: 0, lsb: 0 },
                Bank { msb: 0, lsb: 1 },
                Bank { msb: 1, lsb: 0 },
                Bank { msb: 1, lsb: 1 },
                Bank { msb: 2, lsb: 0 },
                Bank { msb: 2, lsb: 1 },
                Bank { msb: 3, lsb: 0 },
                Bank { msb: 3, lsb: 1 },
                Bank { msb: 4, lsb: 0 },
                Bank { msb: 4, lsb: 1 },
                Bank { msb: 5, lsb: 0 },
                Bank { msb: 5, lsb: 1 },
            ]),
            default_bank: None,
            default_program: None,
        }),
        PipelineStageSpec::NoAudio,
        PipelineStageSpec::StereoProcessor(StereoProcessorSpec {
            in_buffers: (0, 1),
            in_external: None,
            out_buffers: (2, 3),
            out_levels: None,
            processor_type: StereoProcessorType::Effect(EffectSpec::Echo {
                buffer_size: 100000,
                gain: LfSourceExpr::Controller {
                    kind: LiveParameter::Sound7,
                    map0: LfSource::Value(0.0),
                    map1: LfSource::Value(1.0),
                }
                .wrap(),
                delay_time: LfSource::Value(0.5),
                feedback: LfSource::Value(0.6),
                feedback_rotation: LfSource::Value(135.0),
            }),
        }),
        PipelineStageSpec::StereoProcessor(StereoProcessorSpec {
            in_buffers: (2, 3),
            in_external: None,
            out_buffers: (4, 5),
            out_levels: None,
            processor_type: StereoProcessorType::Effect(EffectSpec::SchroederReverb {
                buffer_size: 100000,
                gain: LfSourceExpr::Controller {
                    kind: LiveParameter::Sound8,
                    map0: LfSource::Value(0.0),
                    map1: LfSource::Value(0.5),
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
        }),
        PipelineStageSpec::StereoProcessor(StereoProcessorSpec {
            in_buffers: (4, 5),
            in_external: None,
            out_buffers: (14, 15),
            out_levels: None,
            processor_type: StereoProcessorType::Effect(EffectSpec::RotarySpeaker {
                buffer_size: 100000,
                gain: LfSourceExpr::Controller {
                    kind: LiveParameter::Sound9,
                    map0: LfSource::Value(0.0),
                    map1: LfSource::Value(0.5),
                }
                .wrap(),
                rotation_radius: LfSource::Value(20.0),
                speed: LfSourceExpr::Fader {
                    movement: LfSourceExpr::Controller {
                        kind: LiveParameter::Sound10,
                        map0: LfSource::Value(-2.0),
                        map1: LfSource::Value(1.0),
                    }
                    .wrap(),
                    map0: LfSource::Value(1.0),
                    map1: LfSource::Value(7.0),
                }
                .wrap(),
            }),
        }),
        PipelineStageSpec::WavRecorder(WavRecorderSpec {
            in_buffers: (14, 15),
            file_prefix: "microwave".to_owned(),
            recording_active: LfSourceExpr::Controller {
                kind: LiveParameter::Sound1,
                map0: LfSource::Value(0.0),
                map1: LfSource::Value(1.0),
            }
            .wrap(),
        }),
    ];

    MicrowaveProfile {
        scales,
        default_scale: Some(25),
        num_buffers: 16,
        audio_buffers: (14, 15),
        globals,
        templates,
        envelopes,
        stages,
        color_palette: ColorPalette::default(),
    }
}

pub fn parse_cli_str<T: Parser>(cli_str: &str) -> CliResult<T> {
    let args = iter::once("parse".to_owned()).chain(Shlex::new(cli_str));
    T::try_parse_from(args).display_err("Could not parse CLI string")
}

pub fn get_default_magnetron_spec() -> MagnetronSpec {
    let waveforms = vec![
        WaveformSpec {
            name: "Sine".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: None,
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Sin,
                    frequency: LfSource::template("WaveformPitch"),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Sine³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: None,
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Sin3,
                    frequency: LfSource::template("WaveformPitch"),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Clipped Sine".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Clip {
                        limit: LfSource::Value(0.5),
                    },
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: None,
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Triangle,
                    frequency: LfSource::template("WaveformPitch"),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Triangle³".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Triangle,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Pow3,
                }),
            ],
        },
        WaveformSpec {
            name: "Square".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: Some(LfSource::Value(1.0 / 4.0)),
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Square,
                    frequency: LfSource::template("WaveformPitch"),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Retro Square".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: Some(LfSource::Value(1.0 / 4.0)),
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Square,
                    frequency: LfSource::template("WaveformPitch")
                        * LfSourceExpr::Global("AlternatingOctave".to_owned()).wrap(),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Sawtooth".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![StageType::Generator(GeneratorSpec {
                out_buffer: 7,
                out_level: Some(LfSource::Value(1.0 / 2.0)),
                generator_type: GeneratorType::Oscillator(OscillatorSpec {
                    oscillator_type: OscillatorType::Sawtooth,
                    frequency: LfSource::template("WaveformPitch"),
                    phase: None,
                }),
            })],
        },
        WaveformSpec {
            name: "Fat Sawtooth 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 4.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::Value(0.995) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 4.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::Value(1.005) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Fat Sawtooth 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 4.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::Value(0.995) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 4.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::Value(2.0 * 1.005)
                            * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Expressive Sawtooth (KeyPressure for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(1.0 / 2.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::LowPass2 {
                            resonance: LfSourceExpr::Linear {
                                input: LfSourceExpr::Property(WaveformProperty::KeyPressure).wrap(),
                                map0: LfSource::Value(500.0),
                                map1: LfSource::Value(10000.0),
                            }
                            .wrap(),
                            quality: LfSource::Value(3.0),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Chiptune".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(2.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin3,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 1".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Electric Piano 2".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(880.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Clavinet".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Triangle,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Funky Clavinet".to_owned(),
            envelope: "Piano".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 1,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Triangle,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 1,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::HighPass2 {
                            quality: LfSource::Value(5.0),
                            resonance: LfSource::template("WaveformPitch")
                                * LfSourceExpr::Fader {
                                    movement: LfSource::Value(10.0),
                                    map0: LfSource::Value(2.0),
                                    map1: LfSource::Value(4.0),
                                }
                                .wrap(),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(8.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-4.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(2.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(2.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(4.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-1.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(8.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Rock Organ 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(8.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-4.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(2.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(2.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(4.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-1.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(6.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Pipe Organ".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(8.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-4.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(2.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(2.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(4.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-1.0 / 15.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(8.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Brass".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin3,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Oboe".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(440.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin3,
                            frequency: LfSource::template("WaveformPitch")
                                * LfSourceExpr::Oscillator {
                                    kind: OscillatorType::Sin,
                                    frequency: LfSource::Value(5.0),
                                    phase: None,
                                    baseline: LfSource::Value(1.0),
                                    amplitude: LfSourceExpr::Fader {
                                        movement: LfSource::Value(0.5),
                                        map0: LfSource::Value(0.0),
                                        map1: LfSource::Value(0.01),
                                    }
                                    .wrap(),
                                }
                                .wrap(),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Sax".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Linear {
                            input: LfSourceExpr::Property(WaveformProperty::Velocity).wrap(),
                            map0: LfSource::Value(220.0),
                            map1: LfSource::Value(880.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin3,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Bagpipes".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(880.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin3,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Distortion".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(4400.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 2.0)),
                    processor_type: ProcessorType::Oscillator(ModOscillatorSpec {
                        modulation: Modulation::ByFrequency,
                        spec: OscillatorSpec {
                            oscillator_type: OscillatorType::Sin,
                            frequency: LfSource::template("WaveformPitch"),
                            phase: None,
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 1".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(16.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-8.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(3.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(4.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(5.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-2.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(7.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(9.0) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Bell 2 (12-EDO)".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(16.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-8.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(2.9966) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(4.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(5.0394) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(-2.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(7.1272) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(1.0 / 31.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::Value(8.9797) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Soft Plucked String (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Time {
                            start: LfSource::template("WaveformPeriod"),
                            end: LfSource::template("WaveformPeriod"),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Triangle,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(5000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Negative,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Hard Plucked String (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Time {
                            start: LfSource::template("WaveformPeriod"),
                            end: LfSource::template("WaveformPeriod"),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Noise(NoiseSpec {
                        noise_type: NoiseType::White,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(5000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Negative,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Blown Bottle (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(0.3)),
                    generator_type: GeneratorType::Noise(NoiseSpec {
                        noise_type: NoiseType::White,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(5000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Negative,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Fretless Bass (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Time {
                            start: LfSource::template("WaveformPeriod"),
                            end: LfSource::template("WaveformPeriod"),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Triangle,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(5000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Positive,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Dulcimer".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Time {
                            start: LfSource::template("WaveformPeriod"),
                            end: LfSource::template("WaveformPeriod"),
                            from: LfSource::Value(1.0),
                            to: LfSource::Value(0.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Noise(NoiseSpec {
                        noise_type: NoiseType::White,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSource::Value(2500.0)
                            + LfSource::Value(5.0) * LfSource::template("WaveformPitch"),
                        reflectance: Reflectance::Positive,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Strings (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(0.3)),
                    generator_type: GeneratorType::Noise(NoiseSpec {
                        noise_type: NoiseType::White,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 1,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(6000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Positive,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 1,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::LowPass2 {
                            resonance: LfSource::Value(4.0) * LfSource::template("WaveformPitch"),
                            quality: LfSource::Value(1.0),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Clarinet (Breath for color)".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(
                        LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(0.2),
                            map1: LfSource::Value(1.0),
                        }
                        .wrap(),
                    ),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: Some(LfSource::Value(0.5)),
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSource::Value(5000.0),
                        reflectance: Reflectance::Negative,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 1".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 1,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(1.5) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::MergeProcessor(MergeProcessorSpec {
                    in_buffers: (0, 1),
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: MergeProcessorType::RingModulator,
                }),
            ],
        },
        WaveformSpec {
            name: "Ring Modulation 2".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin3,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Generator(GeneratorSpec {
                    out_buffer: 1,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sin,
                        frequency: LfSource::Value(2.5) * LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::MergeProcessor(MergeProcessorSpec {
                    in_buffers: (0, 1),
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: MergeProcessorType::RingModulator,
                }),
            ],
        },
        WaveformSpec {
            name: "Bright Pad".to_owned(),
            envelope: "Pad".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(1.0 / 2.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::LowPass {
                            cutoff: LfSource::template("WaveformPitch")
                                * LfSourceExpr::Fader {
                                    movement: LfSource::Value(0.5),
                                    map0: LfSource::Value(0.0),
                                    map1: LfSource::Value(10.0),
                                }
                                .wrap(),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Resonance Pad".to_owned(),
            envelope: "Pad".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: Some(LfSource::Value(1.0 / 2.0)),
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Sawtooth,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::LowPass2 {
                            resonance: LfSource::template("WaveformPitch")
                                * LfSourceExpr::Fader {
                                    movement: LfSource::Value(0.5),
                                    map0: LfSource::Value(1.0),
                                    map1: LfSource::Value(32.0),
                                }
                                .wrap(),
                            quality: LfSource::Value(5.0),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Triangle Harp".to_owned(),
            envelope: "Bell".to_owned(),
            stages: vec![
                StageType::Generator(GeneratorSpec {
                    out_buffer: 0,
                    out_level: None,
                    generator_type: GeneratorType::Oscillator(OscillatorSpec {
                        oscillator_type: OscillatorType::Triangle,
                        frequency: LfSource::template("WaveformPitch"),
                        phase: None,
                    }),
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 0,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Filter(FilterSpec {
                        filter_type: FilterType::HighPass {
                            cutoff: LfSource::template("WaveformPitch")
                                * LfSourceExpr::Fader {
                                    movement: LfSource::Value(0.005),
                                    map0: LfSource::Value(1.0),
                                    map1: LfSource::Value(1000.0),
                                }
                                .wrap(),
                        },
                    }),
                }),
            ],
        },
        WaveformSpec {
            name: "Audio-in".to_owned(),
            envelope: "Organ".to_owned(),
            stages: vec![
                StageType::Processor(ProcessorSpec {
                    in_buffer: 12,
                    in_external: Some(true),
                    out_buffer: 6,
                    out_level: Some(LfSource::Value(0.5)),
                    processor_type: ProcessorType::Pass,
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 13,
                    in_external: Some(true),
                    out_buffer: 6,
                    out_level: Some(LfSource::Value(0.5)),
                    processor_type: ProcessorType::Pass,
                }),
                StageType::Processor(ProcessorSpec {
                    in_buffer: 6,
                    in_external: None,
                    out_buffer: 7,
                    out_level: None,
                    processor_type: ProcessorType::Waveguide(WaveguideSpec {
                        buffer_size: 4096,
                        frequency: LfSource::template("WaveformPitch"),
                        cutoff: LfSourceExpr::Controller {
                            kind: LiveParameter::Breath,
                            map0: LfSource::Value(2000.0),
                            map1: LfSource::Value(5000.0),
                        }
                        .wrap(),
                        reflectance: Reflectance::Negative,
                        feedback: LfSource::Value(1.0),
                    }),
                }),
            ],
        },
    ];

    MagnetronSpec {
        note_input: NoteInput::Foreground,
        num_buffers: 8,
        waveforms,
        default_waveform: Some(7),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn serialize_builtin_profile() {
        serde_yaml::to_string(&get_default_profile()).unwrap();
    }

    #[test]
    fn deserialize_provided_profiles() {
        let yml_files = fs::read_dir(".").unwrap().filter_map(|entry| {
            let path = entry.unwrap().path();
            (path.is_file() && path.extension().unwrap_or_default() == "yml").then_some(path)
        });

        for file_path in yml_files {
            async_std::task::block_on(MicrowaveProfile::load(file_path.to_str().unwrap()))
                .map_err(|err| err.to_string())
                .unwrap();
        }
    }
}
