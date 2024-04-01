#![allow(clippy::too_many_arguments)]

mod app;
mod assets;
mod audio;
mod backend;
mod bench;
mod control;
mod fluid;
mod keypress;
mod magnetron;
mod midi;
mod piano;
mod portable;
mod profile;
mod synth;
#[cfg(test)]
mod test;
mod tunable;

use std::{cmp::Ordering, collections::HashMap, io, path::PathBuf, str::FromStr};

use ::magnetron::creator::Creator;
use app::{PhysicalKeyboardLayout, VirtualKeyboardLayout};
use async_std::task;
use bevy::render::color::Color;
use clap::{builder::ValueParserFactory, Parser};
use control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue};
use crossbeam::channel;
use log::{error, warn};
use piano::PianoEngine;
use profile::MicrowaveProfile;
use tune::{
    layout::{EqualTemperament, IsomorphicKeyboard},
    note::NoteLetter,
    pitch::Ratio,
    scala::{Kbm, Scl},
};
use tune_cli::{
    shared::{self, midi::MidiInArgs, KbmOptions, SclCommand},
    CliResult,
};

#[derive(Parser)]
#[command(version)]
enum MainCommand {
    /// Start the microwave GUI
    #[command(name = "run")]
    Run(RunOptions),

    /// Use a keyboard mapping with the given reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm: KbmOptions,

        #[command(flatten)]
        options: RunOptions,
    },

    /// Use a kbm file
    #[command(name = "kbm-file")]
    UseKbmFile {
        /// The location of the kbm file to import
        kbm_file_location: PathBuf,

        #[command(flatten)]
        options: RunOptions,
    },

    /// List MIDI devices
    #[command(name = "devices")]
    Devices,

    /// Run benchmark
    #[command(name = "bench")]
    Bench {
        /// Analyze benchmark
        #[arg(long = "analyze")]
        analyze: bool,
    },
}

#[derive(Parser)]
struct RunOptions {
    /// MIDI input device
    #[arg(long = "midi-in")]
    midi_in_device: Option<String>,

    #[command(flatten)]
    midi_in: MidiInArgs,

    /// Profile location
    #[arg(
        short = 'p',
        long = "profile",
        env = "MICROWAVE_PROFILE",
        default_value = "microwave.yml"
    )]
    profile_location: String,

    #[command(flatten)]
    control_change: ControlChangeOptions,

    /// Enable logging
    #[arg(long = "log")]
    logging: bool,

    #[command(flatten)]
    audio: AudioOptions,

    /// Program number that should be selected at startup
    #[arg(long = "pg", default_value = "0")]
    program_number: u8,

    #[command(flatten)]
    virtual_layout: VirtualKeyboardOptions,

    /// Physical keyboard layout.
    /// [ansi] Large backspace key, horizontal enter key, large left shift key.
    /// [var] Subdivided backspace key, large enter key, large left shift key.
    /// [iso] Large backspace key, vertical enter key, subdivided left shift key.
    #[arg(long = "keyb", default_value = "iso")]
    physical_layout: PhysicalKeyboardLayout,

    /// Odd limit for frequency ratio indicators
    #[arg(long = "lim", default_value = "11")]
    odd_limit: u16,

    #[command(subcommand)]
    scl: Option<SclCommand>,
}

#[derive(Parser)]
struct ControlChangeOptions {
    /// Modulation control number - generic controller
    #[arg(long = "modulation-ccn", default_value = "1")]
    modulation_ccn: u8,

    /// Breath control number - triggered by vertical mouse movement
    #[arg(long = "breath-ccn", default_value = "2")]
    breath_ccn: u8,

    /// Foot switch control number - controls recording
    #[arg(long = "foot-ccn", default_value = "4")]
    foot_ccn: u8,

    /// Volume control number - generic controller
    #[arg(long = "volume-ccn", default_value = "7")]
    volume_ccn: u8,

    /// Balance control number - generic controller
    #[arg(long = "balance-ccn", default_value = "8")]
    balance_ccn: u8,

    /// Panning control number - generic controller
    #[arg(long = "pan-ccn", default_value = "10")]
    pan_ccn: u8,

    /// Expression control number - generic controller
    #[arg(long = "expression-ccn", default_value = "11")]
    expression_ccn: u8,

    /// Damper pedal control number - generic controller
    #[arg(long = "damper-ccn", default_value = "64")]
    damper_ccn: u8,

    /// Sostenuto pedal control number - generic controller
    #[arg(long = "sostenuto-ccn", default_value = "66")]
    sostenuto_ccn: u8,

    /// Soft pedal control number - generic controller
    #[arg(long = "soft-ccn", default_value = "67")]
    soft_ccn: u8,

    /// Legato switch control number - controls legato option
    #[arg(long = "legato-ccn", default_value = "68")]
    legato_ccn: u8,

    /// Sound 1 control number. Triggered by F1 key
    #[arg(long = "sound-1-ccn", default_value = "70")]
    sound_1_ccn: u8,

    /// Sound 2 control number. Triggered by F2 key
    #[arg(long = "sound-2-ccn", default_value = "71")]
    sound_2_ccn: u8,

    /// Sound 3 control number. Triggered by F3 key
    #[arg(long = "sound-3-ccn", default_value = "72")]
    sound_3_ccn: u8,

    /// Sound 4 control number. Triggered by F4 key
    #[arg(long = "sound-4-ccn", default_value = "73")]
    sound_4_ccn: u8,

    /// Sound 5 control number. Triggered by F5 key
    #[arg(long = "sound-5-ccn", default_value = "74")]
    sound_5_ccn: u8,

    /// Sound 6 control number. Triggered by F6 key
    #[arg(long = "sound-6-ccn", default_value = "75")]
    sound_6_ccn: u8,

    /// Sound 7 control number. Triggered by F7 key
    #[arg(long = "sound-7-ccn", default_value = "76")]
    sound_7_ccn: u8,

    /// Sound 8 control number. Triggered by F8 key
    #[arg(long = "sound-8-ccn", default_value = "77")]
    sound_8_ccn: u8,

    /// Sound 9 control number. Triggered by F9 key
    #[arg(long = "sound-9-ccn", default_value = "78")]
    sound_9_ccn: u8,

    /// Sound 10 control number. Triggered by F10 key
    #[arg(long = "sound-10-ccn", default_value = "79")]
    sound_10_ccn: u8,
}

#[derive(Parser)]
struct AudioOptions {
    /// Audio-out buffer size in frames
    #[arg(long = "out-buf", default_value = "1024")]
    buffer_size: u32,

    /// Sample rate [Hz]. If no value is specified the audio device's preferred value will be used
    #[arg(long = "s-rate")]
    sample_rate: Option<u32>,

    /// Prefix for wav file recordings
    #[arg(long = "wav-prefix", default_value = "microwave")]
    wav_file_prefix: String,
}

#[derive(Parser)]
struct VirtualKeyboardOptions {
    /// Primary step width (east direction) of the custom isometric layout (computer keyboard and on-screen keyboard)
    #[arg(long = "p-step", default_value = "4", value_parser = u16::value_parser().range(1..100))]
    primary_step_width: u16,

    /// Secondary step width (south-east direction) of the custom isometric layout (computer keyboard and on-screen keyboard)
    #[arg(long = "s-step", default_value = "1", value_parser = u16::value_parser().range(0..100))]
    secondary_step_width: u16,

    /// Number of primary steps (east direction) of the custom isometric layout (on-screen keyboard)
    #[arg(long = "p-steps", default_value = "1", value_parser = u16::value_parser().range(1..100))]
    num_primary_steps: u16,

    /// Number of secondary steps (south-east direction) of the isometric layout (on-screen keyboard)
    #[arg(long = "s-steps", default_value = "0", value_parser = u16::value_parser().range(0..100))]
    num_secondary_steps: u16,

    /// Color schema of the custom isometric layout (on-screen keyboard, e.g. wgrwwgrwgrwgrwwgr for 17-EDO)
    #[arg(long = "colors", default_value = "wrgbkcmy")]
    colors: KeyColors,
}

#[derive(Clone)]
struct KeyColors(Vec<Color>);

impl FromStr for KeyColors {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.chars()
            .map(|c| match c {
                'w' => Ok(Color::WHITE),
                'r' => Ok(Color::MAROON),
                'g' => Ok(Color::DARK_GREEN),
                'b' => Ok(Color::BLUE),
                'c' => Ok(Color::TEAL),
                'm' => Ok(Color::rgb(0.5, 0.0, 1.0)),
                'y' => Ok(Color::YELLOW),
                'k' => Ok(Color::WHITE * 0.2),
                c => Err(format!(
                    "Received an invalid character '{c}'. Only wrgbcmyk are allowed."
                )),
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|key_colors| {
                if key_colors.is_empty() {
                    Err("Color schema must not be empty".to_owned())
                } else {
                    Ok(KeyColors(key_colors))
                }
            })
    }
}

fn main() {
    portable::init_environment();

    let args = portable::get_args();

    let command = if args.len() < 2 {
        let executable_name = &args[0];
        warn!("Use a subcommand, e.g. `{executable_name} run` to start microwave properly");
        MainCommand::parse_from([executable_name, "run"])
    } else {
        MainCommand::parse_from(&args)
    };

    task::block_on(async {
        if let Err(err) = command.run().await {
            error!("{err:?}");
        }
    })
}

impl MainCommand {
    async fn run(self) -> CliResult {
        match self {
            MainCommand::Run(options) => {
                options
                    .run(Kbm::builder(NoteLetter::D.in_octave(4)).build()?)
                    .await
            }
            MainCommand::WithRefNote { kbm, options } => options.run(kbm.to_kbm()?).await,
            MainCommand::UseKbmFile {
                kbm_file_location,
                options,
            } => {
                options
                    .run(shared::import_kbm_file(&kbm_file_location)?)
                    .await
            }
            MainCommand::Devices => {
                let stdout = io::stdout();
                Ok(shared::midi::print_midi_devices(
                    stdout.lock(),
                    "microwave",
                )?)
            }
            MainCommand::Bench { analyze } => {
                if analyze {
                    bench::analyze_benchmark()
                } else {
                    bench::run_benchmark()
                }
            }
        }
    }
}

impl RunOptions {
    async fn run(self, kbm: Kbm) -> CliResult {
        let scl = self
            .scl
            .as_ref()
            .map(|command| command.to_scl(None))
            .transpose()
            .map_err(|x| format!("error ({x:?})"))?
            .unwrap_or_else(|| {
                Scl::builder()
                    .push_ratio(Ratio::from_semitones(1))
                    .build()
                    .unwrap()
            });

        let virtual_layouts = self.virtual_layout.find_layouts(&scl);

        let profile = MicrowaveProfile::load(&self.profile_location).await?;

        let creator = Creator::new(HashMap::new());

        let globals = profile
            .globals
            .into_iter()
            .map(|spec| (spec.name, creator.create(spec.value)))
            .collect();

        let templates = profile
            .templates
            .into_iter()
            .map(|spec| (spec.name, spec.value))
            .collect();

        let envelopes = profile
            .envelopes
            .into_iter()
            .map(|spec| (spec.name, spec.spec))
            .collect();

        let output_stream_params =
            audio::get_output_stream_params(self.audio.buffer_size, self.audio.sample_rate);

        let (info_send, info_recv) = channel::unbounded();

        let mut backends = Vec::new();
        let mut stages = Vec::new();
        let mut resources = Vec::new();

        for stage in profile.stages {
            stage
                .create(
                    &creator,
                    self.audio.buffer_size,
                    output_stream_params.1.sample_rate,
                    &info_send,
                    &templates,
                    &envelopes,
                    &mut backends,
                    &mut stages,
                    &mut resources,
                )
                .await?;
        }

        let mut storage = LiveParameterStorage::default();
        storage.set_parameter(LiveParameter::Volume, 100u8.as_f64());
        storage.set_parameter(LiveParameter::Balance, 0.5);
        storage.set_parameter(LiveParameter::Pan, 0.5);
        storage.set_parameter(LiveParameter::Legato, 1.0);

        let (storage_send, storage_recv) = channel::unbounded();

        let (engine, engine_state) = PianoEngine::new(
            scl,
            kbm,
            backends,
            self.program_number,
            self.control_change.to_parameter_mapper(),
            storage.clone(),
            storage_send,
        );

        resources.push(Box::new(audio::start_context(
            output_stream_params,
            self.audio.buffer_size,
            profile.num_buffers,
            profile.audio_buffers,
            stages,
            self.audio.wav_file_prefix,
            storage,
            storage_recv,
            globals,
        )));

        self.midi_in_device
            .map(|midi_in_device| {
                midi::connect_to_midi_device(
                    engine.clone(),
                    &midi_in_device,
                    &self.midi_in,
                    self.logging,
                )
                .map(|(_, connection)| resources.push(Box::new(connection)))
            })
            .transpose()?;

        app::start(
            engine,
            engine_state,
            self.physical_layout,
            virtual_layouts,
            self.odd_limit,
            info_recv,
            resources,
        );

        Ok(())
    }
}

impl VirtualKeyboardOptions {
    fn find_layouts(self, scl: &Scl) -> Vec<VirtualKeyboardLayout> {
        let average_step_size = if scl.period().is_negligible() {
            Ratio::from_octaves(1)
        } else {
            scl.period()
        }
        .divided_into_equal_steps(scl.num_items());

        EqualTemperament::find()
            .by_step_size(average_step_size)
            .iter()
            .map(|temperament| {
                let keyboard = temperament.get_keyboard();

                let period = average_step_size.repeated(
                    i32::from(temperament.num_primary_steps()) * i32::from(keyboard.primary_step)
                        + i32::from(temperament.num_secondary_steps())
                            * i32::from(keyboard.secondary_step),
                );

                let description = format!(
                    "{}{}",
                    temperament.prototype(),
                    if temperament.alt_tritave() {
                        " (b-val)"
                    } else {
                        ""
                    }
                );

                VirtualKeyboardLayout {
                    description,
                    keyboard,
                    num_primary_steps: temperament.num_primary_steps(),
                    num_secondary_steps: temperament.num_secondary_steps(),
                    period,
                    colors: generate_colors(temperament),
                }
            })
            .chain([{
                let keyboard = IsomorphicKeyboard {
                    primary_step: self.primary_step_width,
                    secondary_step: self.secondary_step_width,
                };

                let period = average_step_size.repeated(
                    i32::from(self.num_primary_steps) * i32::from(self.primary_step_width)
                        + i32::from(self.num_secondary_steps)
                            * i32::from(self.secondary_step_width),
                );

                VirtualKeyboardLayout {
                    description: "Custom".to_owned(),
                    keyboard,
                    num_primary_steps: self.num_primary_steps,
                    num_secondary_steps: self.num_secondary_steps,
                    period,
                    colors: self.colors.0,
                }
            }])
            .collect()
    }
}

fn generate_colors(temperament: &EqualTemperament) -> Vec<Color> {
    let color_indexes = temperament.get_colors();

    let colors = match temperament.sharpness() >= 0 {
        true => [
            SHARP_COLOR,
            FLAT_COLOR,
            DOUBLE_SHARP_COLOR,
            DOUBLE_FLAT_COLOR,
            TRIPLE_SHARP_COLOR,
            TRIPLE_FLAT_COLOR,
        ],
        false => [
            FLAT_COLOR,
            SHARP_COLOR,
            DOUBLE_FLAT_COLOR,
            DOUBLE_SHARP_COLOR,
            TRIPLE_FLAT_COLOR,
            TRIPLE_SHARP_COLOR,
        ],
    };

    (0..temperament.pergen().period())
        .map(|index| {
            const CYCLE_DARKNESS_FACTOR: f32 = 0.5;

            let generation = temperament.pergen().get_generation(index);
            let degree = generation.degree;
            let color_index = color_indexes[usize::from(degree)];

            // The shade logic combines two requirements:
            // - High contrast in the sharp (north-east) direction => Alternation
            // - High contrast in the secondary (south-east) direction => Exception to the alternation rule for the middle cycle
            let cycle_darkness = match (generation.cycle.unwrap_or_default() * 2 + 1)
                .cmp(&temperament.pergen().num_cycles())
            {
                Ordering::Less => {
                    CYCLE_DARKNESS_FACTOR * f32::from(generation.cycle.unwrap_or_default() % 2 != 0)
                }
                Ordering::Equal => CYCLE_DARKNESS_FACTOR / 2.0,
                Ordering::Greater => {
                    CYCLE_DARKNESS_FACTOR
                        * f32::from(
                            (temperament.pergen().num_cycles()
                                - generation.cycle.unwrap_or_default())
                                % 2
                                != 0,
                        )
                }
            };

            (match color_index {
                0 => NATURAL_COLOR,
                x => colors[(x - 1) % colors.len()],
            }) * (1.0 - cycle_darkness)
        })
        .collect()
}

const NATURAL_COLOR: Color = Color::WHITE;
const SHARP_COLOR: Color = Color::rgb(0.5, 0.0, 1.0);
const FLAT_COLOR: Color = Color::rgb(0.5, 1.0, 0.5);
const DOUBLE_SHARP_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const DOUBLE_FLAT_COLOR: Color = Color::rgb(0.0, 0.5, 0.5);
const TRIPLE_SHARP_COLOR: Color = Color::rgb(0.5, 0.0, 0.5);
const TRIPLE_FLAT_COLOR: Color = Color::rgb(1.0, 0.0, 0.5);

impl ControlChangeOptions {
    fn to_parameter_mapper(&self) -> LiveParameterMapper {
        let mut mapper = LiveParameterMapper::new();
        mapper.push_mapping(LiveParameter::Modulation, self.modulation_ccn);
        mapper.push_mapping(LiveParameter::Breath, self.breath_ccn);
        mapper.push_mapping(LiveParameter::Foot, self.foot_ccn);
        mapper.push_mapping(LiveParameter::Volume, self.volume_ccn);
        mapper.push_mapping(LiveParameter::Balance, self.balance_ccn);
        mapper.push_mapping(LiveParameter::Pan, self.pan_ccn);
        mapper.push_mapping(LiveParameter::Expression, self.expression_ccn);
        mapper.push_mapping(LiveParameter::Damper, self.damper_ccn);
        mapper.push_mapping(LiveParameter::Sostenuto, self.sostenuto_ccn);
        mapper.push_mapping(LiveParameter::Soft, self.soft_ccn);
        mapper.push_mapping(LiveParameter::Legato, self.legato_ccn);
        mapper.push_mapping(LiveParameter::Sound1, self.sound_1_ccn);
        mapper.push_mapping(LiveParameter::Sound2, self.sound_2_ccn);
        mapper.push_mapping(LiveParameter::Sound3, self.sound_3_ccn);
        mapper.push_mapping(LiveParameter::Sound4, self.sound_4_ccn);
        mapper.push_mapping(LiveParameter::Sound5, self.sound_5_ccn);
        mapper.push_mapping(LiveParameter::Sound6, self.sound_6_ccn);
        mapper.push_mapping(LiveParameter::Sound7, self.sound_7_ccn);
        mapper.push_mapping(LiveParameter::Sound8, self.sound_8_ccn);
        mapper.push_mapping(LiveParameter::Sound9, self.sound_9_ccn);
        mapper.push_mapping(LiveParameter::Sound10, self.sound_10_ccn);
        mapper
    }
}
