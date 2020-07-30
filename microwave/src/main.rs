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
use std::{path::PathBuf, sync::mpsc};
use structopt::StructOpt;
use synth::WaveformSynth;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    scala::{self, Scl},
    temperament::{EqualTemperament, TemperamentPreference},
};

#[derive(StructOpt)]
pub struct Config {
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
    #[structopt(long = "sf")]
    soundfont_file_location: Option<PathBuf>,

    /// Delay duration (s)
    #[structopt(long = "dd", default_value = "1")]
    delay_secs: f32,

    /// Delay feedback
    #[structopt(long = "df", default_value = "0")]
    delay_feedback: f32,

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

    #[structopt(subcommand)]
    command: Option<Command>,
}

#[derive(StructOpt)]
enum Command {
    /// List MIDI devices
    #[structopt(name = "list")]
    ListMidiDevices,

    /// Equal temperament
    #[structopt(name = "equal")]
    EqualTemperament {
        /// Step size, e.g. 1:12:2
        step_size: Ratio,
    },

    /// Rank-2 temperament
    #[structopt(name = "rank2")]
    Rank2Temperament {
        /// First generator (finite), e.g. 3/2
        generator: Ratio,

        /// Number of positive generations using the first generator, e.g. 6
        num_pos_generations: u16,

        /// Number of negative generations using the first generator, e.g. 1
        #[structopt(default_value = "0")]
        num_neg_generations: u16,

        /// Second generator (infinite)
        #[structopt(short = "p", default_value = "2")]
        period: Ratio,
    },

    /// Harmonic series
    #[structopt(name = "harm")]
    HarmonicSeries {
        /// The lowest harmonic, e.g. 8
        lowest_harmonic: u16,

        /// Number of of notes, e.g. 8
        #[structopt(short = "n")]
        number_of_notes: Option<u16>,

        /// Build subharmonic series
        #[structopt(short = "s")]
        subharmonics: bool,
    },

    /// Custom Scale
    #[structopt(name = "cust")]
    Custom {
        /// Items of the scale
        items: Vec<Ratio>,

        /// Name of the scale
        #[structopt(short = "n")]
        name: Option<String>,
    },
}

fn main() {
    nannou::app(model).update(model::update).run();
}

fn model(app: &App) -> Model {
    let config = Config::from_args();

    let mut keyboard = Keyboard::root_at(PianoKey::from_midi_number(0));

    if let Some(Command::EqualTemperament { step_size }) = config.command {
        let preference = if config.use_porcupine {
            TemperamentPreference::Porcupine
        } else {
            TemperamentPreference::PorcupineWhenMeantoneIsBad
        };

        let temperament = EqualTemperament::find()
            .with_preference(preference)
            .by_step_size(step_size);

        keyboard = keyboard.with_steps_of(&temperament).coprime()
    }

    let primary_step = config
        .primary_step
        .unwrap_or_else(|| keyboard.primary_step());
    let secondary_step = config
        .secondary_step
        .unwrap_or_else(|| keyboard.secondary_step());
    keyboard = keyboard.with_steps(primary_step, secondary_step);

    let synth_mode = match &config.soundfont_file_location {
        Some(_) => SynthMode::Fluid,
        None => SynthMode::OnlyWaveform,
    };

    let (send_updates, receive_updates) = mpsc::channel();

    let fluid_synth = FluidSynth::new(config.soundfont_file_location, send_updates);
    let waveform_synth = WaveformSynth::new();

    let (engine, engine_snapshot) = PianoEngine::new(
        synth_mode,
        config.command.map(create_scale),
        config.program_number.unwrap_or(0).min(127),
        fluid_synth.messages(),
        waveform_synth.messages(),
    );

    let audio = AudioModel::new(
        fluid_synth,
        waveform_synth,
        config.buffer_size,
        config.delay_secs,
        config.delay_feedback,
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

    Model::new(
        audio,
        engine,
        engine_snapshot,
        keyboard,
        midi_in,
        receive_updates,
    )
}

fn create_scale(command: Command) -> Scl {
    match command {
        Command::ListMidiDevices => {
            midi::print_midi_devices();
            std::process::exit(0)
        }
        Command::EqualTemperament { step_size } => scala::create_equal_temperament_scale(step_size),
        Command::Rank2Temperament {
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        } => scala::create_rank2_temperament_scale(
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        ),
        Command::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => scala::create_harmonics_scale(
            u32::from(lowest_harmonic),
            u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
            subharmonics,
        ),
        Command::Custom { items, name } => {
            create_custom_scale(items, name.unwrap_or_else(|| "Custom scale".to_string()))
        }
    }
}

fn create_custom_scale(items: Vec<Ratio>, name: String) -> Scl {
    let mut scale = Scl::with_name(name);
    for item in items {
        scale.push_ratio(item);
    }
    scale.build().unwrap()
}
