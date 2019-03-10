use std::fs::File;
use std::io;
use std::path::PathBuf;
use structopt::StructOpt;
use tune::ratio::Ratio;
use tune::scale;
use tune::scale::Scale;

#[derive(StructOpt)]
enum Options {
    /// Create a scale file
    #[structopt(name = "scl")]
    Scale(ScaleOptions),

    // Dump pitches of a scale
    #[structopt(name = "dump")]
    Dump(ScaleCommand),
}

#[derive(StructOpt)]
struct ScaleOptions {
    /// Write output to file
    #[structopt(short)]
    output_file: Option<PathBuf>,

    #[structopt(subcommand)]
    command: ScaleCommand,
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
        Options::Dump(command) => {
            dump_scale(command);
            Ok(())
        }
    }
}

fn execute_scale_command(output_file: Option<PathBuf>, command: ScaleCommand) -> io::Result<()> {
    let scale = create_scale(command);
    if let Some(output_file) = output_file {
        File::create(output_file).and_then(|file| scale.write_scl(file))
    } else {
        scale.write_scl(io::stdout().lock())
    }
}

fn dump_scale(command: ScaleCommand) {
    let scale = create_scale(command);
    for i in 0..128 {
        println!("{} | {}", i, scale.pitch(i).describe(Default::default()));
    }
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
    }
}
