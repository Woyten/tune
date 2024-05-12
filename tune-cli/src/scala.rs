use std::{
    fs::File,
    path::{Path, PathBuf},
};

use clap::Parser;
use tune::{
    key::PianoKey,
    pitch::{Ratio, RatioExpression, RatioExpressionVariant},
    scala::{self, Kbm, KbmImportError, KbmRoot, Scl, SclBuildError, SclImportError, SegmentType},
};

use crate::{error::ResultExt, App, CliError, CliResult};

#[derive(Parser)]
pub(crate) struct SclOptions {
    /// Name of the scale
    #[arg(long = "name")]
    name: Option<String>,

    #[command(subcommand)]
    scl: SclCommand,
}

impl SclOptions {
    pub fn run(self, app: &mut App) -> CliResult {
        Ok(app.write(format_args!("{}", self.scl.to_scl(self.name)?.export()))?)
    }
}

#[derive(Parser)]
pub enum SclCommand {
    /// Scale with custom step sizes
    #[command(name = "steps")]
    Steps {
        /// Steps of the scale
        #[arg(use_value_delimiter = true)]
        items: Vec<RatioExpression>,
    },

    /// Rank-2 temperament
    #[command(name = "rank2")]
    Rank2Temperament {
        /// First generator (finite), e.g. 3/2
        generator: Ratio,

        /// Number of positive generations using the first generator, e.g. 6
        num_pos_generations: u16,

        /// Number of negative generations using the first generator, e.g. 1
        #[arg(default_value = "0")]
        num_neg_generations: u16,

        /// Second generator (infinite)
        #[arg(long = "per", default_value = "2")]
        period: Ratio,
    },

    /// Harmonic series
    #[command(name = "harm")]
    HarmonicSeries {
        /// Create undertonal harmonic series
        #[arg(short = 'u')]
        utonal: bool,

        /// Start of the harmonic segment, usually the lowest harmonic, e.g. 8
        segment_start: u16,

        /// Size of the harmonic segment (i.e. the number of of notes) if unequal to the segment start number
        segment_size: Option<u16>,

        #[arg(long = "neji")]
        /// Create a near-equal JI scale of the given harmonic segment
        neji_divisions: Option<u16>,
    },

    /// Import scl file
    #[command(name = "scl-file")]
    UseSclFile {
        /// The location of the file to import
        scl_file_location: PathBuf,
    },
}

impl SclCommand {
    pub fn to_scl(&self, description: Option<String>) -> Result<Scl, CliError> {
        match self {
            SclCommand::Steps { items } => create_custom_scale(description, items)
                .handle_error("Could not create steps-based scale"),
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
            )
            .handle_error("Could not create rank2 scale"),
            &SclCommand::HarmonicSeries {
                utonal,
                segment_start,
                segment_size,
                neji_divisions,
            } => {
                let segment_size = segment_size.unwrap_or(segment_start);
                let segment_type = match utonal {
                    false => SegmentType::Otonal,
                    true => SegmentType::Utonal,
                };
                scala::create_harmonics_scale(
                    description,
                    segment_type,
                    segment_start,
                    segment_size,
                    neji_divisions,
                )
                .handle_error("Could not create harmonic scale")
            }
            SclCommand::UseSclFile { scl_file_location } => {
                let mut scale = import_scl_file(scl_file_location)?;
                if let Some(description) = description {
                    scale.set_description(description)
                }
                Ok(scale)
            }
        }
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

#[derive(Parser)]
pub(crate) enum KbmCommand {
    /// Provide a reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm: KbmOptions,
    },
}

impl KbmCommand {
    pub fn run(&self, app: &mut App) -> CliResult {
        let KbmCommand::WithRefNote { kbm } = self;
        Ok(app.write(format_args!("{}", kbm.to_kbm()?.export()))?)
    }
}

#[derive(Parser)]
pub struct KbmOptions {
    #[command(flatten)]
    kbm_root: KbmRootOptions,

    /// Lower key bound (inclusive)
    #[arg(long = "lo-key", default_value = "21")]
    lower_key_bound: i32,

    /// Upper key bound (exclusive)
    #[arg(long = "up-key", default_value = "109")]
    upper_key_bound: i32,

    /// Keyboard mapping entries, e.g. 0,x,1,x,2,3,x,4,x,5,x,6
    #[arg(long = "key-map", use_value_delimiter = true, value_parser = parse_item)]
    items: Option<Vec<Item>>,

    /// The formal octave of the keyboard mapping, e.g. n in n-EDO
    #[arg(long = "octave")]
    formal_octave: Option<i16>,
}

#[derive(Clone)]
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
        builder
            .build()
            .handle_error("Could not create keyboard mapping")
    }
}

#[derive(Parser)]
pub struct KbmRootOptions {
    /// Reference note that should sound at its original or a custom pitch, e.g. 69@440Hz
    ref_note: KbmRoot,

    /// root note / "middle note" of the scale if different from reference note
    #[arg(long = "root")]
    root_note: Option<i16>,
}

impl KbmRootOptions {
    pub fn to_kbm_root(&self) -> KbmRoot {
        match self.root_note {
            Some(root_note) => KbmRoot {
                root_offset: i32::from(root_note) - self.ref_note.ref_key.midi_number(),
                ..self.ref_note
            },
            None => self.ref_note,
        }
    }
}

fn import_scl_file(file_name: &Path) -> Result<Scl, String> {
    File::open(file_name)
        .map_err(SclImportError::IoError)
        .and_then(Scl::import)
        .map_err(|err| match err {
            SclImportError::IoError(err) => {
                format!("Could not read scl file {file_name:#?}: {err}")
            }
            SclImportError::ParseError { line_number, kind } => {
                format!("Could not parse scl file {file_name:#?} at line {line_number}: {kind:#?}")
            }
            SclImportError::StructuralError(err) => {
                format!("Malformed scl file {file_name:#?}: {err:#?}")
            }
            SclImportError::BuildError(err) => {
                format!("Unsupported scl file {file_name:#?}: {err:#?}")
            }
        })
}

pub fn import_kbm_file(file_name: &Path) -> Result<Kbm, String> {
    File::open(file_name)
        .map_err(KbmImportError::IoError)
        .and_then(Kbm::import)
        .map_err(|err| match err {
            KbmImportError::IoError(err) => {
                format!("Could not read kbm file {file_name:#?}: {err}")
            }
            KbmImportError::ParseError { line_number, kind } => {
                format!("Could not parse kbm file {file_name:#?} at line {line_number}: {kind:#?}")
            }
            KbmImportError::StructuralError(err) => {
                format!("Malformed kbm file {file_name:#?}: {err:#?}")
            }
            KbmImportError::BuildError(err) => {
                format!("Unsupported kbm file {file_name:#?}: {err:#?}")
            }
        })
}
