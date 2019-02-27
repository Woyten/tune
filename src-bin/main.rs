use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;
use tune::ratio::Ratio;
use tune::scale;

#[derive(Debug, StructOpt)]
enum Options {
    /// Create a scale file
    #[structopt(name = "scl")]
    Scale(ScaleOptions),
}

#[derive(Debug, StructOpt)]
struct ScaleOptions {
    /// Write output to file
    #[structopt(short)]
    output_file: Option<PathBuf>,

    #[structopt(subcommand)]
    command: ScaleCommand,
}

#[derive(Debug, StructOpt)]
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

        /// Number of notes to create by first generator, e.g. 7
        number_of_notes: u16,

        /// Offset
        #[structopt(short, default_value = "0")]
        offset: i16,

        /// Second generator (infinite)
        #[structopt(short, default_value = "2")]
        period: Ratio,
    },

    /// Harmonic series
    #[structopt(name = "harm")]
    HarmonicSeries {
        /// The lowest harmonic, e.g. 8
        lowest_harmonic: u16,

        /// Number of of notes, e.g. 8
        #[structopt(short)]
        number_of_notes: Option<u16>,

        /// Build subharmonic series
        #[structopt(short)]
        subharmonics: bool,
    },
}

fn main() -> io::Result<()> {
    match Options::from_args() {
        Options::Scale(ScaleOptions {
            output_file,
            command,
        }) => execute_scale_command(output_file, command),
    }
}

fn execute_scale_command(output_file: Option<PathBuf>, command: ScaleCommand) -> io::Result<()> {
    let scale = match command {
        ScaleCommand::EqualTemperament { step_size } => {
            scale::create_equal_temperament_scale(step_size)
        }
        ScaleCommand::Rank2Temperament {
            generator,
            number_of_notes,
            offset,
            period,
        } => scale::create_rank2_temperament_scale(generator, number_of_notes, offset, period),
        ScaleCommand::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => scale::create_harmonics_scale(
            u32::from(lowest_harmonic),
            u32::from(number_of_notes.unwrap_or(lowest_harmonic)),
            subharmonics,
        ),
    };

    if let Some(output_file) = output_file {
        let file = File::create(output_file)?;
        scale.write_scl(file)
    } else {
        scale.write_scl(io::stdout().lock())
    }
}
