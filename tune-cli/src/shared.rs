//! Code to be shared with other CLIs. At the moment, this module is not intended to become a stable API.

use std::{
    error::Error,
    fs::File,
    io,
    path::{Path, PathBuf},
};

use midir::{MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    pitch::{Ratio, RatioExpression, RatioExpressionVariant},
    scala::{self, Kbm, KbmImportError, KbmRoot, Scl, SclBuildError, SclImportError},
};

use crate::{CliError, CliResult};

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
        scl_file_location: PathBuf,
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
            SclCommand::Import { scl_file_location } => {
                let mut scale = import_scl_file(&scl_file_location)?;
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

#[derive(StructOpt)]
pub struct KbmRootOptions {
    /// Reference note that should sound at its original or a custom pitch, e.g. 69@440Hz
    ref_note: KbmRoot,

    /// root note / "middle note" of the scale if different from reference note
    #[structopt(long = "root")]
    root_note: Option<i16>,
}

impl KbmRootOptions {
    pub fn to_kbm_root(&self) -> KbmRoot {
        match self.root_note {
            Some(root_note) => self
                .ref_note
                .shift_origin_by(i32::from(root_note) - self.ref_note.origin.midi_number()),
            None => self.ref_note,
        }
    }
}

#[derive(StructOpt)]
pub struct KbmOptions {
    #[structopt(flatten)]
    kbm_root: KbmRootOptions,

    /// Lower key bound (inclusve)
    #[structopt(long = "lo-key", default_value = "21")]
    lower_key_bound: i32,

    /// Upper key bound (exclusive)
    #[structopt(long = "up-key", default_value = "109")]
    upper_key_bound: i32,

    /// Keyboard mapping entries, e.g. 0,x,1,x,2,3,x,4,x,5,x,6
    #[structopt(long = "key-map", require_delimiter = true, parse(try_from_str=parse_item))]
    items: Option<Vec<Item>>,

    /// The formal octave of the keyboard mapping, e.g. n in n-EDO
    #[structopt(long = "octave")]
    formal_octave: Option<i16>,
}

enum Item {
    Mapped(i16),
    Unmapped,
}

fn parse_item(s: &str) -> Result<Item, &'static str> {
    if ["x", "X"].contains(&s) {
        return Ok(Item::Unmapped);
    }
    if let Ok(parsed) = s.parse() {
        return Ok(Item::Mapped(parsed));
    }
    Err("Invalid keyboard mapping entry. Should be x, X or an 16-bit signed integer")
}

impl KbmOptions {
    pub fn to_kbm(&self) -> CliResult<Kbm> {
        let mut builder = Kbm::builder(self.kbm_root.to_kbm_root()).range(
            PianoKey::from_midi_number(self.lower_key_bound)
                ..PianoKey::from_midi_number(self.upper_key_bound),
        );
        if let Some(items) = &self.items {
            for item in items {
                match item {
                    &Item::Mapped(scale_degree) => {
                        builder = builder.push_mapped_key(scale_degree);
                    }
                    Item::Unmapped => {
                        builder = builder.push_unmapped_key();
                    }
                }
            }
        }
        if let Some(formal_octave) = self.formal_octave {
            builder = builder.formal_octave(formal_octave);
        }
        Ok(builder.build()?)
    }
}

pub fn import_scl_file(file_name: &Path) -> Result<Scl, String> {
    File::open(file_name)
        .map_err(SclImportError::IoError)
        .and_then(Scl::import)
        .map_err(|err| match err {
            SclImportError::IoError(err) => format!("Could not read scl file: {}", err),
            SclImportError::ParseError { line_number, kind } => format!(
                "Could not parse scl file at line {} ({:?})",
                line_number, kind
            ),
            SclImportError::StructuralError(err) => format!("Malformed scl file ({:?})", err),
            SclImportError::BuildError(err) => format!("Unsupported scl file ({:?})", err),
        })
}

pub fn import_kbm_file(file_name: &Path) -> Result<Kbm, String> {
    File::open(file_name)
        .map_err(KbmImportError::IoError)
        .and_then(Kbm::import)
        .map_err(|err| match err {
            KbmImportError::IoError(err) => format!("Could not read kbm file: {}", err),
            KbmImportError::ParseError { line_number, kind } => format!(
                "Could not parse kbm file at line {} ({:?})",
                line_number, kind
            ),
            KbmImportError::StructuralError(err) => format!("Malformed kbm file ({:?})", err),
            KbmImportError::BuildError(err) => format!("Unsupported kbm file ({:?})", err),
        })
}

pub type MidiResult<T> = Result<T, MidiError>;

#[derive(Clone, Debug)]
pub enum MidiError {
    DeviceNotFound {
        wanted: String,
        available: Vec<String>,
    },
    AmbiguousDevice {
        wanted: String,
        matches: Vec<String>,
    },
    Other(String),
}

impl<T: Error> From<T> for MidiError {
    fn from(error: T) -> Self {
        MidiError::Other(error.to_string())
    }
}

impl From<MidiError> for CliError {
    fn from(v: MidiError) -> Self {
        CliError::CommandError(format!("Could not connect to MIDI device ({:#?})", v))
    }
}

pub fn print_midi_devices(mut dst: impl io::Write, client_name: &str) -> MidiResult<()> {
    let midi_input = MidiInput::new(client_name)?;
    writeln!(dst, "Readable MIDI devices:")?;
    for port in midi_input.ports() {
        writeln!(dst, "- {}", midi_input.port_name(&port)?)?;
    }

    let midi_output = MidiOutput::new(client_name)?;
    writeln!(dst, "Writable MIDI devices:")?;
    for port in midi_output.ports() {
        writeln!(dst, "- {}", midi_output.port_name(&port)?)?;
    }

    Ok(())
}

pub fn connect_to_in_device(
    client_name: &str,
    target_port: &str,
    mut callback: impl FnMut(&[u8]) + Send + 'static,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    let midi_input = MidiInput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_input, target_port)?;

    Ok((
        port_name,
        midi_input.connect(
            &port,
            "Input Connection",
            move |_, message, _| callback(message),
            (),
        )?,
    ))
}

pub fn connect_to_out_device(
    client_name: &str,
    target_port: &str,
) -> MidiResult<(String, MidiOutputConnection)> {
    let midi_output = MidiOutput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_output, target_port)?;

    Ok((port_name, midi_output.connect(&port, "Output Connection")?))
}

fn find_port_by_name<IO: MidiIO>(
    midi_io: &IO,
    target_port: &str,
) -> MidiResult<(String, IO::Port)> {
    let target_port_lowercase = target_port.to_lowercase();

    let mut matching_ports = midi_io
        .ports()
        .into_iter()
        .filter_map(|port| {
            midi_io
                .port_name(&port)
                .ok()
                .filter(|port_name| port_name.to_lowercase().contains(&target_port_lowercase))
                .map(|port_name| (port_name, port))
        })
        .collect::<Vec<_>>();

    match matching_ports.len() {
        0 => Err(MidiError::DeviceNotFound {
            wanted: target_port_lowercase,
            available: midi_io
                .ports()
                .iter()
                .filter_map(|port| midi_io.port_name(port).ok())
                .collect(),
        }),
        1 => Ok(matching_ports.pop().unwrap()),
        _ => Err(MidiError::AmbiguousDevice {
            wanted: target_port_lowercase,
            matches: matching_ports
                .into_iter()
                .map(|(port_name, _)| port_name)
                .collect(),
        }),
    }
}
