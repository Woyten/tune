//! Code to be shared with other CLIs. At the moment, this module is not intended to become a stable API.

use crate::CliError;
use std::{fs::File, path::PathBuf};
use structopt::StructOpt;
use tune::{
    ratio::{Ratio, RatioExpression, RatioExpressionVariant},
    scala::{self, Scl, SclBuildError, SclImportError},
};

#[derive(StructOpt)]
pub enum SclCommand {
    /// Scale with custom step sizes
    #[structopt(name = "steps")]
    Steps {
        /// Steps of the scale
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
                u32::from(lowest_harmonic),
                u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
                subharmonics,
            )?,
            SclCommand::Import { file_name } => {
                let mut scale = import_scl_file(&file_name)?;
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

fn import_scl_file(file_name: &PathBuf) -> Result<Scl, String> {
    let file =
        File::open(file_name).map_err(|io_err| format!("Could not read scl file: {}", io_err))?;

    Scl::import(file).map_err(|err| match err {
        SclImportError::IoError(err) => format!("Could not read scl file: {}", err),
        SclImportError::ParseError { line_number, kind } => format!(
            "Could not parse scl file at line {} ({:?})",
            line_number, kind
        ),
        SclImportError::StructuralError(err) => format!("Malformed scl file ({:?})", err),
        SclImportError::BuildError(err) => format!("Unsupported scl file ({:?})", err),
    })
}
