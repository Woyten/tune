mod ratio;

use crate::ratio::Ratio;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
enum Mode {
    /// Create a scale file
    #[structopt(name = "scl")]
    Scale(Scale),
}

#[derive(Debug, StructOpt)]
enum Scale {
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

fn main() {
    let mode = Mode::from_args();
    match mode {
        Mode::Scale(Scale::EqualTemperament { step_size }) => {
            print_equal_temperament_file(step_size.as_float())
        }
        Mode::Scale(Scale::HarmonicSeries {
            lowest_harmonic,
            number_of_notes,
            subharmonics,
        }) => print_harmonics_file(
            lowest_harmonic,
            number_of_notes.unwrap_or(lowest_harmonic),
            subharmonics,
        ),
        Mode::Scale(Scale::Rank2Temperament {
            generator,
            number_of_notes,
            offset,
            period,
        }) => print_rank2_temperament_file(
            generator.as_float(),
            number_of_notes,
            offset,
            period.as_float(),
        ),
    }
}

fn print_equal_temperament_file(step_size: f64) {
    assert!(step_size >= 1.0);

    let step_size_in_cents = step_size.log2() * 1200.0;

    println!("equal steps of ratio {}", step_size);
    println!("1");
    println!("{:.3}", step_size_in_cents);
}

fn print_rank2_temperament_file(generator: f64, number_of_notes: u16, offset: i16, period: f64) {
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

    println!(
        "{} generations of generator {} with period {}",
        number_of_notes, generator, period
    );
    println!("{}", number_of_notes);
    for note in notes {
        println!("{:.3}", note * 1200.0);
    }
}

fn print_harmonics_file(lowest_harmonic: u16, number_of_notes: u16, subharmonics: bool) {
    assert!(lowest_harmonic > 0);

    let debug_text = if subharmonics {
        "subharmonics"
    } else {
        "harmonics"
    };
    println!(
        "{} {} starting with {}",
        number_of_notes, debug_text, lowest_harmonic
    );
    println!("{}", number_of_notes);
    let highest_harmonic = lowest_harmonic + number_of_notes;
    if subharmonics {
        for harmonic in (lowest_harmonic..highest_harmonic).rev() {
            println!("{}/{}", highest_harmonic, harmonic);
        }
    } else {
        for harmonic in (lowest_harmonic + 1)..=highest_harmonic {
            println!("{}/{}", harmonic, lowest_harmonic);
        }
    }
}
