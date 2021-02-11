//! Code to be shared with other CLIs. At the moment, this module is not intended to become a stable API.

use std::{error::Error, io, path::PathBuf};

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use structopt::StructOpt;
use tune::{
    pitch::{Ratio, RatioExpression, RatioExpressionVariant},
    scala::{self, Scl, SclBuildError},
};

use crate::CliError;

#[derive(StructOpt)]
pub enum SclCommand {
    /// Scale with custom step sizes
    #[structopt(name = "steps")]
    Steps {
        /// Steps of the scale
        #[structopt(require_delimiter = true)]
        items: Vec<RatioExpression>,
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
        #[structopt(long = "per", default_value = "2")]
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
        #[structopt(long = "sub")]
        subharmonics: bool,
    },

    /// Import scl file
    #[structopt(name = "import")]
    Import {
        /// The location of the file to import
        file_name: PathBuf,
    },
}

impl SclCommand {
    pub fn to_scl(&self, description: Option<String>) -> Result<Scl, CliError> {
        Ok(match self {
            SclCommand::Steps { items } => create_custom_scale(description, items)?,
            &SclCommand::Rank2Temperament {
                generator,
                num_pos_generations,
                num_neg_generations,
                period,
            } => scala::create_rank2_temperament_scale(
                description,
                generator,
                num_pos_generations,
                num_neg_generations,
                period,
            )?,
            &SclCommand::HarmonicSeries {
                lowest_harmonic,
                number_of_notes,
                subharmonics,
            } => scala::create_harmonics_scale(
                description,
                lowest_harmonic,
                number_of_notes.unwrap_or(lowest_harmonic),
                subharmonics,
            )?,
            SclCommand::Import { file_name } => {
                let mut scale = crate::import_scl_file(&file_name)?;
                if let Some(description) = description {
                    scale.set_description(description)
                }
                scale
            }
        })
    }
}

fn create_custom_scale(
    description: impl Into<Option<String>>,
    items: &[RatioExpression],
) -> Result<Scl, SclBuildError> {
    let mut builder = Scl::builder();
    for item in items {
        match item.variant() {
            RatioExpressionVariant::Float { float_value } => {
                if let Some(float_value) = as_int(float_value) {
                    builder = builder.push_int(float_value);
                    continue;
                }
            }
            RatioExpressionVariant::Fraction { numer, denom } => {
                if let (Some(numer), Some(denom)) = (as_int(numer), as_int(denom)) {
                    builder = builder.push_fraction(numer, denom);
                    continue;
                }
            }
            _ => {}
        }
        builder = builder.push_ratio(item.ratio());
    }

    match description.into() {
        Some(description) => builder.build_with_description(description),
        None => builder.build(),
    }
}

fn as_int(float: f64) -> Option<u32> {
    let rounded = float.round();
    if (float - rounded).abs() < 1e-6 {
        Some(rounded as u32)
    } else {
        None
    }
}

pub type MidiResult<T> = Result<T, MidiError>;

#[derive(Clone, Debug)]
pub enum MidiError {
    MidiDeviceNotFound(usize),
    Other(String),
}

impl<T: Error> From<T> for MidiError {
    fn from(error: T) -> Self {
        MidiError::Other(error.to_string())
    }
}

impl From<MidiError> for CliError {
    fn from(v: MidiError) -> Self {
        CliError::CommandError(format!("Could not connect to MIDI device ({:?})", v))
    }
}

pub fn print_midi_devices(mut dst: impl io::Write, client_name: &str) -> MidiResult<()> {
    let midi_input = MidiInput::new(client_name)?;
    writeln!(dst, "Readable MIDI devices:")?;
    for (index, port) in midi_input.ports().iter().enumerate() {
        let port_name = midi_input.port_name(port)?;
        writeln!(dst, "({}) {}", index, port_name)?;
    }

    let midi_output = MidiOutput::new(client_name)?;
    writeln!(dst, "Writable MIDI devices:")?;
    for (index, port) in midi_output.ports().iter().enumerate() {
        let port_name = midi_output.port_name(port)?;
        writeln!(dst, "({}) {}", index, port_name)?;
    }

    Ok(())
}

pub fn connect_to_in_device(
    client_name: &str,
    target_port: usize,
    mut callback: impl FnMut(&[u8]) + Send + 'static,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    let midi_input = MidiInput::new(client_name)?;
    match midi_input.ports().get(target_port) {
        Some(port) => Ok((
            midi_input.port_name(port)?,
            midi_input.connect(
                port,
                "Input Connection",
                move |_, message, _| callback(message),
                (),
            )?,
        )),
        None => Err(MidiError::MidiDeviceNotFound(target_port)),
    }
}

pub fn connect_to_out_device(
    client_name: &str,
    target_port: usize,
) -> MidiResult<(String, MidiOutputConnection)> {
    let midi_output = MidiOutput::new(client_name)?;
    match midi_output.ports().get(target_port) {
        Some(port) => Ok((
            midi_output.port_name(port)?,
            midi_output.connect(port, "Output Connection")?,
        )),
        None => Err(MidiError::MidiDeviceNotFound(target_port)),
    }
}
