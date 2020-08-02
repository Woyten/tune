use fluidlite_lib as _;

mod audio;
mod effects;
mod fluid;
mod keypress;
mod midi;
mod model;
mod piano;
mod synth;
mod tuner;
mod view;
mod wave;

use audio::AudioModel;
use fluid::FluidSynth;
use model::Model;
use nannou::app::App;
use piano::{PianoEngine, SynthMode};
use std::{path::PathBuf, process, sync::mpsc};
use structopt::StructOpt;
use synth::WaveformSynth;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    scala::Scl,
    temperament::{EqualTemperament, TemperamentPreference},
};
use tune_cli::shared::SclCommand;

#[derive(StructOpt)]
enum MainCommand {
    /// Start the microwave GUI
    #[structopt(name = "run")]
    Run(RunOptions),

    /// List MIDI devices
    #[structopt(name = "list")]
    ListMidiDevices,
}

#[derive(StructOpt)]
struct RunOptions {
    /// MIDI source device
    #[structopt(long = "ms")]
    midi_source: Option<usize>,

    /// MIDI channel (0-based) to listen to
    #[structopt(long = "mc", default_value = "0")]
    midi_channel: u8,

    /// Enable logging of MIDI messages
    #[structopt(long = "lg")]
    midi_logging: bool,

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

    /// Program number that should be selected at startup
    #[structopt(long = "pg")]
    program_number: Option<u8>,

    /// Audio buffer size in frames
    #[structopt(long = "bs", default_value = "64")]
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

fn main() {
    nannou::app(try_model).update(model::update).run();
}

fn try_model(app: &App) -> Model {
    model(app).unwrap_or_else(|err| {
        eprintln!("Could not start application / {}", err);
        process::exit(1);
    })
}

fn model(app: &App) -> Result<Model, String> {
    match MainCommand::from_args() {
        MainCommand::Run(run_options) => start(app, run_options),
        MainCommand::ListMidiDevices => {
            midi::print_midi_devices();
            process::exit(1);
        }
    }
}

fn start(app: &App, config: RunOptions) -> Result<Model, String> {
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
    let waveform_synth = WaveformSynth::new();

    let (engine, engine_snapshot) = PianoEngine::new(
        synth_mode,
        scale,
        config.program_number.unwrap_or(0).min(127),
        fluid_synth.messages(),
        waveform_synth.messages(),
    );

    let audio = AudioModel::new(
        fluid_synth,
        waveform_synth,
        config.buffer_size,
        config.delay_time,
        config.delay_feedback,
        config.delay_feedback_rotation.to_radians(),
    );

    let (midi_channel, midi_logging) = (config.midi_channel, config.midi_logging);
    let midi_in = config.midi_source.map(|midi_source| {
        midi::connect_to_midi_device(midi_source, engine.clone(), midi_channel, midi_logging)
    });

    app.new_window()
        .maximized(true)
        .title("Microwave - Microtonal Waveform Synthesizer by Woyten")
        .raw_event(model::event)
        .key_pressed(model::key_pressed)
        .mouse_pressed(model::mouse_pressed)
        .mouse_moved(model::mouse_moved)
        .mouse_released(model::mouse_released)
        .mouse_wheel(model::mouse_wheel)
        .touch(model::touch)
        .view(view::view)
        .build()
        .unwrap();

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
