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

use std::{io, path::PathBuf, str::FromStr};

use ::magnetron::creator::Creator;
use app::{PhysicalKeyboardLayout, VirtualKeyboardLayout};
use async_std::task;
use clap::{builder::ValueParserFactory, Parser};
use control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue};
use crossbeam::channel;
use log::{error, warn};
use piano::PianoEngine;
use profile::MicrowaveProfile;
use tune::{
    key::{Keyboard, PianoKey},
    note::NoteLetter,
    pitch::Ratio,
    scala::{Kbm, Scl},
    temperament::{EqualTemperament, TemperamentPreference, TemperamentType},
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

    /// Color schema of the scale-specific keyboard (e.g. wgrwwgrwgrwgrwwgr for 17-EDO)
    #[arg(long = "kb2", default_value = "wbwwbwbwbwwb")]
    scale_keyboard_colors: KeyColors,

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
    /// Use porcupine layout for the isomorphic when possible
    #[arg(long = "porcupine")]
    use_porcupine: bool,

    /// Override primary step width (east direction) of the isometric keyboard
    #[arg(long = "p-width", value_parser = u8::value_parser().range(1..100))]
    primary_step_width: Option<u8>,

    /// Override secondary step width (south-east direction) of the isometric keyboard
    #[arg(long = "s-width", value_parser = u8::value_parser().range(0..100))]
    secondary_step_width: Option<u8>,

    /// Override number of primary steps (east direction) of the isometric keyboard
    #[arg(long = "p-steps", value_parser = u8::value_parser().range(1..100))]
    num_primary_steps: Option<u8>,

    /// Override number of secondary steps (south-east direction) of the isometric keyboard
    #[arg(long = "s-steps", value_parser = u8::value_parser().range(0..100))]
    num_secondary_steps: Option<u8>,
}

#[derive(Clone)]
struct KeyColors(Vec<KeyColor>);

#[derive(Clone, Copy)]
pub enum KeyColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
}

impl FromStr for KeyColors {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.chars()
            .map(|c| match c {
                'w' => Ok(KeyColor::White),
                'r' => Ok(KeyColor::Red),
                'g' => Ok(KeyColor::Green),
                'b' => Ok(KeyColor::Blue),
                'c' => Ok(KeyColor::Cyan),
                'm' => Ok(KeyColor::Magenta),
                'y' => Ok(KeyColor::Yellow),
                'k' => Ok(KeyColor::Black),
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

        let virtual_layout = self.virtual_layout.evaluate(&scl);

        let profile = MicrowaveProfile::load(&self.profile_location).await?;

        let waveform_templates = profile
            .waveform_templates
            .into_iter()
            .map(|spec| (spec.name, spec.value))
            .collect();

        let waveform_envelopes = profile
            .waveform_envelopes
            .into_iter()
            .map(|spec| (spec.name, spec.spec))
            .collect();

        let effect_templates = profile
            .effect_templates
            .into_iter()
            .map(|spec| (spec.name, spec.value))
            .collect();

        let creator = Creator::new(effect_templates);

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
                    &waveform_templates,
                    &waveform_envelopes,
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
            storage,
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
            self.scale_keyboard_colors.0,
            self.physical_layout,
            virtual_layout,
            self.odd_limit,
            info_recv,
            resources,
        );

        Ok(())
    }
}

impl VirtualKeyboardOptions {
    fn evaluate(self, scl: &Scl) -> VirtualKeyboardLayout {
        let preference = if self.use_porcupine {
            TemperamentPreference::Porcupine
        } else {
            TemperamentPreference::PorcupineWhenMeantoneIsBad
        };

        let average_step_size = if scl.period().is_negligible() {
            Ratio::from_octaves(1)
        } else {
            scl.period()
        }
        .divided_into_equal_steps(scl.num_items());

        let temperament = EqualTemperament::find()
            .with_preference(preference)
            .by_step_size(average_step_size);

        let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
            .with_steps_of(&temperament)
            .coprime();

        let primary_step = self
            .primary_step_width
            .map(i16::from)
            .unwrap_or_else(|| keyboard.primary_step());

        let secondary_step = self
            .secondary_step_width
            .map(i16::from)
            .unwrap_or_else(|| keyboard.secondary_step());

        let num_primary_steps =
            self.num_primary_steps
                .unwrap_or_else(|| match temperament.temperament_type() {
                    TemperamentType::Meantone => 5,
                    TemperamentType::Porcupine => 6,
                });

        let num_secondary_steps =
            self.num_secondary_steps
                .unwrap_or_else(|| match temperament.temperament_type() {
                    TemperamentType::Meantone => 2,
                    TemperamentType::Porcupine => 1,
                });

        let period = average_step_size.repeated(
            i32::from(primary_step) * i32::from(num_primary_steps)
                + i32::from(secondary_step) * i32::from(num_secondary_steps),
        );

        VirtualKeyboardLayout {
            keyboard: keyboard.with_steps(primary_step, secondary_step),
            num_primary_steps,
            num_secondary_steps,
            period,
        }
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
