mod audio;
mod effects;
mod fluid;
mod keypress;
mod midi;
mod model;
mod piano;
mod synth;
mod view;
mod wave;

use std::{io, path::PathBuf, process, sync::mpsc, sync::Arc};

use audio::AudioModel;
use effects::{DelayOptions, RotaryOptions};
use fluid::FluidSynth;
use model::Model;
use nannou::app::App;
use piano::{PianoEngine, SynthMode};
use structopt::StructOpt;
use synth::WaveformSynth;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    scala::Scl,
    temperament::{EqualTemperament, TemperamentPreference},
};
use tune_cli::{
    shared::{self, SclCommand},
    CliResult,
};

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
    /// MIDI target device
    #[structopt(long = "midi-out")]
    midi_target: Option<usize>,

    /// MIDI source device
    #[structopt(long = "midi-in")]
    midi_source: Option<usize>,

    /// MIDI channel (0-based) to listen to
    #[structopt(long = "in-chan", default_value = "0")]
    midi_channel: u8,

    /// Damper pedal control number (waveform synth)
    #[structopt(long = "dampcn", default_value = "64")]
    damper_control_number: u8,

    /// Pitch wheel sensivity (waveform synth)
    #[structopt(long = "pwsens", default_value = "200c")]
    pitch_wheel_sensivity: Ratio,

    /// Enable logging
    #[structopt(long = "log")]
    logging: bool,

    /// Enable fluidlite using the soundfont file at the given location
    #[structopt(long = "sf", env = "MICROWAVE_SF")]
    soundfont_file_location: Option<PathBuf>,

    #[structopt(flatten)]
    delay: DelayParameters,

    #[structopt(flatten)]
    rotary: RotaryParameters,

    /// Program number that should be selected at startup
    #[structopt(long = "pg", default_value = "0")]
    program_number: u8,

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

#[derive(StructOpt)]
struct DelayParameters {
    /// Delay time (s)
    #[structopt(long = "del-tm", default_value = "0.5")]
    delay_time: f32,

    /// Delay feedback
    #[structopt(long = "del-fb", default_value = "0.6")]
    delay_feedback: f32,

    /// Delay feedback rotation angle (degrees clock-wise)
    #[structopt(long = "del-rot", default_value = "135")]
    delay_feedback_rotation: f32,
}

#[derive(StructOpt)]
struct RotaryParameters {
    /// Rotary speaker radius (cm)
    #[structopt(long = "rot-rad", default_value = "20")]
    pub rotation_radius: f32,

    /// Rotary speaker minimum speed (revolutions per s)
    #[structopt(long = "rot-min", default_value = "1")]
    pub rotation_min_frequency: f32,

    /// Rotary speaker maximum speed (revolutions per s)
    #[structopt(long = "rot-max", default_value = "7")]
    pub rotation_max_frequency: f32,

    /// Rotary speaker acceleration time (s)
    #[structopt(long = "rot-acc", default_value = "1")]
    pub rotation_acceleration: f32,

    /// Rotary speaker deceleration time (s)
    #[structopt(long = "rot-dec", default_value = "0.5")]
    pub rotation_deceleration: f32,
}

fn main() {
    nannou::app(try_model).update(model::update).run();
}

fn try_model(app: &App) -> Model {
    model(app).unwrap_or_else(|err| {
        eprintln!("{:?}", err);
        process::exit(1);
    })
}

fn model(app: &App) -> CliResult<Model> {
    match MainCommand::from_args() {
        MainCommand::Run(options) => start(app, options),
        MainCommand::Devices => {
            let stdout = io::stdout();
            shared::print_midi_devices(stdout.lock(), "microwave").unwrap();
            process::exit(1);
        }
    }
}

fn start(app: &App, config: RunOptions) -> CliResult<Model> {
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

    let waveform_synth = WaveformSynth::new(config.pitch_wheel_sensivity);
    let fluid_synth = FluidSynth::new(&config.soundfont_file_location, send_updates);
    let connection_result = config
        .midi_target
        .map(|device| shared::connect_to_out_device("microwave", device))
        .transpose()?;
    let (device, midi_out) = match connection_result {
        Some((device, midi_out)) => (Some(device), Some(midi_out)),
        None => (None, None),
    };

    let mut available_synth_modes = Vec::new();
    if let Some(device) = device {
        available_synth_modes.push(SynthMode::MidiOut {
            device,
            curr_program: 0,
        })
    }
    if let Some(soundfont_file_location) = config.soundfont_file_location {
        available_synth_modes.push(SynthMode::Fluid {
            soundfont_file_location,
        });
    }
    available_synth_modes.push(SynthMode::Waveform {
        curr_waveform: 0,
        waveforms: Arc::new(wave::all_waveforms()),
        envelope_type: None,
        continuous: false,
    });

    let (engine, engine_snapshot) = PianoEngine::new(
        scale,
        available_synth_modes,
        waveform_synth.messages(),
        config.damper_control_number,
        fluid_synth.messages(),
        midi_out,
        config.program_number,
    );

    let audio = AudioModel::new(
        fluid_synth,
        waveform_synth,
        config.buffer_size,
        config.delay.to_options(),
        config.rotary.to_options(),
    );

    let (midi_channel, midi_logging) = (config.midi_channel, config.logging);
    let midi_in = config
        .midi_source
        .map(|midi_source| {
            midi::connect_to_midi_device(midi_source, engine.clone(), midi_channel, midi_logging)
        })
        .transpose()?
        .map(|(_, connection)| connection);

    app.new_window()
        .maximized(true)
        .title("Microwave - Microtonal Waveform Synthesizer by Woyten")
        .raw_event(model::raw_event)
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

impl DelayParameters {
    fn to_options(&self) -> DelayOptions {
        DelayOptions {
            delay_time_in_s: self.delay_time,
            feedback_intensity: self.delay_feedback,
            feedback_rotation: self.delay_feedback_rotation.to_radians(),
        }
    }
}

impl RotaryParameters {
    fn to_options(&self) -> RotaryOptions {
        RotaryOptions {
            rotation_radius_in_cm: self.rotation_radius,
            min_frequency_in_hz: self.rotation_min_frequency,
            max_frequency_in_hz: self.rotation_max_frequency,
            acceleration_time_in_s: self.rotation_acceleration,
            deceleration_time_in_s: self.rotation_deceleration,
        }
    }
}
