#![allow(clippy::manual_clamp, clippy::too_many_arguments, clippy::unit_arg)]

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

use std::{collections::HashMap, path::PathBuf, str::FromStr};

use ::magnetron::automation::AutomationFactory;
use app::{PhysicalKeyboardLayout, VirtualKeyboardResource};
use async_std::task;
use bevy::render::color::Color;
use clap::{builder::ValueParserFactory, Parser};
use control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue};
use piano::PianoEngine;
use profile::MicrowaveProfile;
use tune::{
    note::NoteLetter,
    pitch::Ratio,
    scala::{Kbm, Scl},
};
use tune_cli::{
    shared::{
        self,
        error::ResultExt,
        midi::MidiInArgs,
        scala::{KbmOptions, SclCommand},
    },
    CliError, CliResult,
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

    #[command(flatten)]
    audio: AudioOptions,

    /// Program number that should be selected at startup
    #[arg(long = "pg", default_value = "0")]
    program_number: u8,

    #[command(flatten)]
    custom_keyboard: CustomKeyboardOptions,

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
struct CustomKeyboardOptions {
    /// Name of the custom isometric layout
    #[arg(long = "cust-layout", default_value = "PC Keyboard")]
    layout_name: String,

    /// Primary step width (east direction) of the custom isometric layout (computer keyboard and on-screen keyboard)
    #[arg(long = "p-step", default_value = "4", value_parser = u16::value_parser().range(1..100))]
    primary_step: u16,

    /// Secondary step width (south-east direction) of the custom isometric layout (computer keyboard and on-screen keyboard)
    #[arg(long = "s-step", default_value = "1", value_parser = u16::value_parser().range(0..100))]
    secondary_step: u16,

    /// Number of primary steps (east direction) of the custom isometric layout (on-screen keyboard)
    #[arg(long = "p-steps", default_value = "1", value_parser = u16::value_parser().range(1..100))]
    num_primary_steps: u16,

    /// Number of secondary steps (south-east direction) of the custom isometric layout (on-screen keyboard)
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
        log::warn!("Use a subcommand, e.g. `{executable_name} run` to start microwave properly");
        MainCommand::try_parse_from([executable_name, "run"])
    } else {
        MainCommand::try_parse_from(&args)
    };

    match command {
        Ok(command) => task::block_on(async {
            if let Err(err) = command.run().await {
                log::error!("{err}");
            }
        }),
        Err(err) => {
            if err.use_stderr() {
                portable::eprintln(err);
            } else {
                portable::println(err);
            }
        }
    }
}

impl MainCommand {
    async fn run(self) -> CliResult {
        match self {
            MainCommand::Run(options) => {
                options
                    .run(Kbm::builder(NoteLetter::D.in_octave(4)).build().unwrap())
                    .await
            }
            MainCommand::WithRefNote { kbm, options } => options.run(kbm.to_kbm()?).await,
            MainCommand::UseKbmFile {
                kbm_file_location,
                options,
            } => {
                options
                    .run(shared::scala::import_kbm_file(&kbm_file_location)?)
                    .await
            }
            MainCommand::Devices => {
                let mut message = Vec::new();
                shared::midi::print_midi_devices(&mut message, "microwave")
                    .handle_error::<CliError>("Could not print MIDI devices")?;
                portable::print(String::from_utf8_lossy(&message));
                Ok(())
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
            .transpose()?
            .unwrap_or_else(|| {
                Scl::builder()
                    .push_ratio(Ratio::from_semitones(1))
                    .build()
                    .unwrap()
            });

        let virtual_keyboard = VirtualKeyboardResource::new(&scl, self.custom_keyboard);

        let profile = MicrowaveProfile::load(&self.profile_location).await?;

        let mut factory = AutomationFactory::new(HashMap::new());

        let globals = profile
            .globals
            .into_iter()
            .map(|spec| (spec.name, factory.automate(spec.value)))
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

        let (info_send, info_recv) = flume::unbounded();

        let mut backends = Vec::new();
        let mut stages = Vec::new();
        let mut resources = Vec::new();

        for stage in profile.stages {
            stage
                .create(
                    &mut factory,
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

        let (storage_send, storage_recv) = flume::unbounded();

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

        if let Some(midi_in_device) = self.midi_in_device {
            midi::connect_to_in_device(engine.clone(), midi_in_device, &self.midi_in)?;
        }

        app::start(
            engine,
            engine_state,
            self.physical_layout,
            virtual_keyboard,
            self.odd_limit,
            info_recv,
            resources,
        );

        Ok(())
    }
}

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
