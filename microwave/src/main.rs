#![allow(clippy::manual_clamp, clippy::too_many_arguments, clippy::unit_arg)]

mod app;
mod audio;
mod backend;
mod bench;
mod control;
mod fluid;
mod keypress;
mod lumatone;
mod magnetron;
mod midi;
mod piano;
mod pipeline;
mod portable;
mod profile;
mod recorder;
mod synth;
mod toggle;
mod tunable;
mod tuning_layout;

use std::any::Any;
use std::str::FromStr;

use app::PhysicalKeyboardLayout;
use async_std::task;
use bevy::color::palettes::css;
use bevy::prelude::*;
use clap::Parser;
use clap::Subcommand;
use clap::builder::ValueParserFactory;
use control::LiveParameter;
use control::LiveParameterMapper;
use control::LiveParameterStorage;
use control::ParameterValue;
use piano::PianoEngine;
use profile::MicrowaveProfile;
use tune_cli::CliError;
use tune_cli::CliResult;
use tune_cli::shared;
use tune_cli::shared::error::ResultExt;
use tune_cli::shared::midi::MidiInArgs;

use crate::pipeline::AudioPipeline;
use crate::toggle::Toggle;
use crate::tuning_layout::TuningLayout;

#[derive(Parser)]
#[command(version)]
struct MainCommand {
    #[command(flatten)]
    options: RunOptions,

    #[command(subcommand)]
    subcommand: Option<SubCommand>,
}

#[derive(Subcommand)]
enum SubCommand {
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
    /// Syncs the currently selected layout to the specified Lumatone MIDI device.
    ///
    /// If this is parameter is set, `midi-in` defaults to the same device.
    /// Other midi-in settings will have no effect.
    #[arg(long = "luma-ctrl")]
    lumatone_device: Option<String>,

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
}

#[derive(Clone, Parser)]
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
struct KeyColors(Vec<Srgba>);

impl FromStr for KeyColors {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.chars()
            .map(|c| match c {
                'w' => Ok(css::WHITE),
                'r' => Ok(css::MAROON),
                'g' => Ok(css::GREEN),
                'b' => Ok(css::BLUE),
                'c' => Ok(css::TEAL),
                'm' => Ok(Srgba::rgb(0.5, 0.0, 1.0)),
                'y' => Ok(css::YELLOW),
                'k' => Ok(css::WHITE * 0.2),
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

    match MainCommand::try_parse_from(&args) {
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
        match self.subcommand {
            None => self.options.run().await,
            Some(SubCommand::Devices) => {
                let mut message = Vec::new();
                shared::midi::print_midi_devices(&mut message, "microwave")
                    .debug_err::<CliError>("Could not print MIDI devices")?;
                portable::print(String::from_utf8_lossy(&message));
                Ok(())
            }
            Some(SubCommand::Bench { analyze }) => {
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
    async fn run(self) -> CliResult {
        // Track resources (e.g. audio contexts) that need to be kept alive.
        let mut resources = Vec::new();

        let profile = MicrowaveProfile::load(&self.profile_location).await?;

        let parsed_scales = profile.parse_scales()?;

        if parsed_scales.is_empty() {
            return Err("No scales defined in profile".to_owned().into());
        }

        let tuning_layouts = Toggle::with_initial_index(
            parsed_scales
                .into_iter()
                .map(|(scl, kbm)| {
                    TuningLayout::new(
                        scl,
                        kbm,
                        self.custom_keyboard.clone(),
                        &profile.color_palette,
                    )
                })
                .collect(),
            profile.default_scale.unwrap_or_default(),
        );

        let stream_params =
            audio::get_output_stream_params(self.audio.buffer_size, self.audio.sample_rate);

        let mut initial_storage = LiveParameterStorage::default();
        initial_storage.set_parameter(LiveParameter::Volume, 100u8.as_f64());
        initial_storage.set_parameter(LiveParameter::Balance, 0.5);
        initial_storage.set_parameter(LiveParameter::Pan, 0.5);
        initial_storage.set_parameter(LiveParameter::Legato, 1.0);

        let (pipeline, backends, storage_updates, events) = AudioPipeline::create(
            &mut resources,
            stream_params.buffer_size,
            stream_params.sample_rate,
            profile,
            initial_storage.clone(),
        )
        .await?;

        audio::start_context(&mut resources, &stream_params, pipeline);

        let lumatone_send = self
            .lumatone_device
            .as_ref()
            .map(|port_name| lumatone::connect_lumatone(port_name))
            .transpose()
            .debug_err::<CliError>("Could not connect to Lumatone")?;

        let engine = PianoEngine::new(
            tuning_layouts,
            backends,
            self.control_change.to_parameter_mapper(),
            initial_storage,
            storage_updates,
            lumatone_send.clone(),
        );

        let midi_source = match self.lumatone_device.is_some() {
            true => None,
            false => Some(self.midi_in.get_midi_source()?),
        };

        if let Some(midi_in_device) = self.midi_in_device.or(self.lumatone_device) {
            midi::connect_to_in_device(engine.clone(), midi_in_device, midi_source)?;
        }

        app::start(
            engine,
            self.physical_layout,
            self.odd_limit,
            events,
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

pub type Resources = Vec<Box<dyn Any>>;
