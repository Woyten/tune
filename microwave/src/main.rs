use fluidlite_lib as _;

mod audio;
mod effects;
mod keypress;
mod midi;
mod model;
mod view;
mod wave;

use audio::Audio;
use model::{Model, PianoEngine, SynthMode};
use nannou::app::App;
use std::{path::PathBuf, sync::Arc};
use structopt::StructOpt;
use tune::{
    key::{Keyboard, PianoKey},
    ratio::Ratio,
    scale::{self, Scale},
    temperament::{EqualTemperament, TemperamentPreference},
};

#[derive(StructOpt)]
pub struct Config {
    /// MIDI source device
    #[structopt(long = "ms")]
    midi_source: Option<usize>,

    /// Enable fluidlite using the soundfont file at the given location
    #[structopt(long = "sf")]
    soundfont_file_location: Option<PathBuf>,

    /// Program number that should be selected at startup
    #[structopt(long = "pg")]
    program_number: Option<u32>,

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
    scale: Option<ScaleCommand>,
}

#[derive(StructOpt)]
enum ScaleCommand {
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
    nannou::app(model).run();
}

fn model(app: &App) -> Model {
    let config = Config::from_args();

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

    let mut keyboard = Keyboard::root_at(PianoKey::from_midi_number(0));

    if let Some(ScaleCommand::EqualTemperament { step_size }) = config.scale {
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

    let audio = Audio::new(config.soundfont_file_location, config.buffer_size);

    let engine = Arc::new(PianoEngine::new(
        synth_mode,
        config.scale.map(create_scale),
        config.program_number.unwrap_or(0).min(127),
        audio,
    ));

    let midi_in = config
        .midi_source
        .map(|midi_source| midi::connect_to_midi_device(midi_source, engine.clone()));

    Model::new(engine, keyboard, midi_in)
}

fn create_scale(command: ScaleCommand) -> Scale {
    match command {
        ScaleCommand::EqualTemperament { step_size } => {
            scale::create_equal_temperament_scale(step_size)
        }
        ScaleCommand::Rank2Temperament {
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        } => scale::create_rank2_temperament_scale(
            generator,
            num_pos_generations,
            num_neg_generations,
            period,
        ),
        ScaleCommand::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => scale::create_harmonics_scale(
            u32::from(lowest_harmonic),
            u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
            subharmonics,
        ),
        ScaleCommand::Custom { items, name } => {
            create_custom_scale(items, name.unwrap_or_else(|| "Custom scale".to_string()))
        }
    }
}

fn create_custom_scale(items: Vec<Ratio>, name: String) -> Scale {
    let mut scale = Scale::with_name(name);
    for item in items {
        scale.push_ratio(item);
    }
    scale.build()
}
