use fluidlite_lib as _;

mod audio;
mod effects;
mod fluid;
mod keypress;
mod midi;
pub mod model;
pub mod piano;
mod synth;
mod wave;

use audio::AudioModel;
use fluid::FluidSynth;
use model::Model;
use piano::{PianoEngine, SynthMode};
use std::{io, path::PathBuf, process, sync::mpsc};
use structopt::StructOpt;
use synth::WaveformSynth;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    scala::Scl,
    temperament::{EqualTemperament, TemperamentPreference},
};
use tune_cli::shared::{self, SclCommand};

#[derive(StructOpt)]
enum MainCommand {
    /// Start the microwave GUI
    #[structopt(name = "run")]
    Run(RunOptions),

    /// List MIDI devices
    #[structopt(name = "devices")]
    Devices,
}

#[derive(StructOpt)]
struct RunOptions {
    /// MIDI source device
    #[structopt(long = "ms")]
    midi_source: Option<usize>,

    /// MIDI channel (0-based) to listen to
    #[structopt(long = "mc", default_value = "0")]
    midi_channel: u8,

    /// Damper pedal control number (waveform synth only)
    #[structopt(long = "dampcn", default_value = "64")]
    damper_control_number: u8,

    /// Pitch wheel sensivity (waveform synth only)
    #[structopt(long = "pwsens", default_value = "200c")]
    pitch_wheel_sensivity: Ratio,

    /// Enable logging
    #[structopt(long = "log")]
    logging: bool,

    /// Enable fluidlite using the soundfont file at the given location
    #[structopt(long = "sf", env = "MICROWAVE_SF")]
    soundfont_file_location: Option<PathBuf>,

    /// Delay time (s)
    #[structopt(long = "deltm", default_value = "0.5")]
    delay_time: f32,

    /// Delay feedback
    #[structopt(long = "delfb", default_value = "0.6")]
    delay_feedback: f32,

    /// Delay feedback rotation angle (degrees clock-wise)
    #[structopt(long = "delrot", default_value = "135")]
    delay_feedback_rotation: f32,

    /// Program number that should be selected at startup (fluidlite synth only)
    #[structopt(long = "pg")]
    program_number: Option<u8>,

    /// Audio buffer size in frames (frame rate = 44100 Hz)
    #[structopt(long = "bs", default_value = "1024")]
    buffer_size: usize,

    /// Use porcupine layout when possible
    #[structopt(long = "porcupine")]
    use_porcupine: bool,

    /// Primary step width (right direction) when playing on the computer keyboard
    #[structopt(long = "ps")]
    primary_step: Option<i16>,

    /// Secondary step width (down/right direction) when playing on the computer keyboard
    #[structopt(long = "ss")]
    secondary_step: Option<i16>,

    /// Integer limit for frequency ratio indicators
    #[structopt(long = "lim", default_value = "11")]
    limit: u16,

    #[structopt(subcommand)]
    command: Option<SclCommand>,
}

pub fn create_model_from_args(args: impl IntoIterator<Item = String>) -> Result<Model, String> {
    let options = MainCommand::from_iter_safe(args).map_err(|err| err.message)?;
    match options {
        MainCommand::Run(run_options) => create_model(run_options),
        MainCommand::Devices => {
            let stdout = io::stdout();
            shared::print_midi_devices(stdout.lock(), "microwave").unwrap();
            process::exit(1);
        }
    }
}

fn create_model(config: RunOptions) -> Result<Model, String> {
    let synth_mode = match &config.soundfont_file_location {
        Some(_) => SynthMode::Fluid,
        None => SynthMode::OnlyWaveform,
    };

    let scale = config
        .command
        .as_ref()
        .map(|command| command.to_scl(None))
        .transpose()
        .map_err(|x| format!("error ({:?})", x))?
        .unwrap_or_else(|| {
            Scl::builder()
                .push_ratio(Ratio::from_semitones(1))
                .build()
                .unwrap()
        });

    let keyboard = create_keyboard(&scale, &config);

    let (send_updates, receive_updates) = mpsc::channel();

    let fluid_synth = FluidSynth::new(config.soundfont_file_location, send_updates);
    let waveform_synth = WaveformSynth::new(config.pitch_wheel_sensivity);

    let (engine, engine_snapshot) = PianoEngine::new(
        synth_mode,
        scale,
        config.program_number.unwrap_or(0).min(127),
        fluid_synth.messages(),
        waveform_synth.messages(),
        config.damper_control_number,
    );

    let audio = AudioModel::new(
        fluid_synth,
        waveform_synth,
        config.buffer_size,
        config.delay_time,
        config.delay_feedback,
        config.delay_feedback_rotation.to_radians(),
    );

    let (midi_channel, midi_logging) = (config.midi_channel, config.logging);
    let midi_in = config.midi_source.map(|midi_source| {
        midi::connect_to_midi_device(midi_source, engine.clone(), midi_channel, midi_logging)
    });

    Ok(Model::new(
        audio,
        engine,
        engine_snapshot,
        keyboard,
        config.limit,
        midi_in,
        receive_updates,
    ))
}

fn create_keyboard(scl: &Scl, config: &RunOptions) -> Keyboard {
    let preference = if config.use_porcupine {
        TemperamentPreference::Porcupine
    } else {
        TemperamentPreference::PorcupineWhenMeantoneIsBad
    };

    let average_step_size = scl.period().divided_into_equal_steps(scl.size() as f64);

    let temperament = EqualTemperament::find()
        .with_preference(preference)
        .by_step_size(average_step_size);

    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&temperament)
        .coprime();

    let primary_step = config
        .primary_step
        .unwrap_or_else(|| keyboard.primary_step());
    let secondary_step = config
        .secondary_step
        .unwrap_or_else(|| keyboard.secondary_step());

    keyboard.with_steps(primary_step, secondary_step)
}
