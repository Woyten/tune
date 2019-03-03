use std::fs::File;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use structopt::StructOpt;
use tune::ratio::Ratio;

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

fn main() -> Result<(), io::Error> {
    match Options::from_args() {
        Options::Scale(ScaleOptions {
            output_file,
            command,
        }) => write_scale(output_file, command),
    }
}

fn write_scale(output_file: Option<PathBuf>, command: ScaleCommand) -> Result<(), io::Error> {
    if let Some(output_file) = output_file {
        let file = File::create(output_file)?;
        write_scale_to(file, command)
    } else {
        write_scale_to(io::stdout().lock(), command)
    }
}

fn write_scale_to<W: Write>(target: W, command: ScaleCommand) -> Result<(), io::Error> {
    match command {
        ScaleCommand::EqualTemperament { step_size } => {
            write_equal_temperament_scale(target, step_size.as_float())?
        }
        ScaleCommand::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        } => write_harmonics_scale(
            target,
            lowest_harmonic,
            number_of_notes.unwrap_or(lowest_harmonic),
            subharmonics,
        )?,
        ScaleCommand::Rank2Temperament {
            generator,
            number_of_notes,
            offset,
            period,
        } => write_rank2_temperament_scale(
            target,
            generator.as_float(),
            number_of_notes,
            offset,
            period.as_float(),
        )?,
    }

    Ok(())
}

fn write_equal_temperament_scale<W: Write>(mut target: W, step_size: f64) -> Result<(), io::Error> {
    assert!(step_size >= 1.0);

    let step_size_in_cents = step_size.log2() * 1200.0;

    writeln!(target, "equal steps of ratio {}", step_size)?;
    writeln!(target, "1")?;
    writeln!(target, "{:.3}", step_size_in_cents)?;

    Ok(())
}

fn write_rank2_temperament_scale<W: Write>(
    mut target: W,
    generator: f64,
    number_of_notes: u16,
    offset: i16,
    period: f64,
) -> Result<(), io::Error> {
    assert!(generator > 0.0);
    assert!(period > 1.0);

    let generator_log = generator.log2();
    let period_log = period.log2();

    let mut notes = (0..number_of_notes)
        .map(|generation| {
            let exponent = i32::from(generation) + i32::from(offset);
            if exponent == 0 {
                return period_log;
            }

            let generated_note = f64::from(exponent) * generator_log;
            let note_in_period_interval = generated_note % period_log;

            if note_in_period_interval <= 0.0 {
                note_in_period_interval + period_log
            } else {
                note_in_period_interval
            }
        })
        .collect::<Vec<_>>();
    notes.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("Comparison yielded an invalid result")
    });

    writeln!(
        target,
        "{} generations of generator {} with period {}",
        number_of_notes, generator, period
    )?;
    writeln!(target, "{}", number_of_notes)?;
    for note in notes {
        writeln!(target, "{:.3}", note * 1200.0)?;
    }

    Ok(())
}

fn write_harmonics_scale<W: Write>(
    mut target: W,
    lowest_harmonic: u16,
    number_of_notes: u16,
    subharmonics: bool,
) -> Result<(), io::Error> {
    assert!(lowest_harmonic > 0);

    let debug_text = if subharmonics {
        "subharmonics"
    } else {
        "harmonics"
    };
    writeln!(
        target,
        "{} {} starting with {}",
        number_of_notes, debug_text, lowest_harmonic
    )?;
    writeln!(target, "{}", number_of_notes)?;
    let highest_harmonic = lowest_harmonic + number_of_notes;
    if subharmonics {
        for harmonic in (lowest_harmonic..highest_harmonic).rev() {
            writeln!(target, "{}/{}", highest_harmonic, harmonic)?;
        }
    } else {
        for harmonic in (lowest_harmonic + 1)..=highest_harmonic {
            writeln!(target, "{}/{}", harmonic, lowest_harmonic)?;
        }
    }

    Ok(())
}
