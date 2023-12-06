use std::{io, iter, mem};

use clap::Parser;
use tune::{math, pergen::Mos, pitch::Ratio};

use crate::App;

#[derive(Parser)]
pub(crate) enum MosCommand {
    /// Find MOSes for a given generator
    #[command(name = "find")]
    FindMoses(FindMosesOptions),

    /// Find generators for a given MOS
    #[command(name = "gen")]
    FindGenerators(FindGeneratorsOptions),
}

impl MosCommand {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        match self {
            MosCommand::FindMoses(options) => options.run(app),
            MosCommand::FindGenerators(options) => options.run(app),
        }
    }
}

#[derive(Parser)]
pub(crate) struct FindMosesOptions {
    /// Period of the MOS
    #[arg(long = "per", default_value = "2.0")]
    period: Ratio,

    /// Generator of the MOS
    generator: Ratio,

    /// Chroma size below which the scale is considered an equal-step scale
    #[arg(long = "chroma", default_value = "0.5c")]
    threshold: Ratio,
}

impl FindMosesOptions {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        for mos in LegacyMos::new(self.generator.num_equal_steps_of_size(self.period)).children() {
            if mos.is_convergent() {
                app.write("* ")?;
            } else {
                app.write("  ")?;
            }
            if self.period.repeated(mos.chroma()) >= self.threshold {
                app.writeln(format_args!(
                    "num_notes = {}, {}L{}s, L = {:#.0}, s = {:#.0}",
                    mos.num_steps(),
                    mos.num_large_steps,
                    mos.num_small_steps,
                    self.period.repeated(mos.large_step_size),
                    self.period.repeated(mos.small_step_size),
                ))?;
            } else {
                app.writeln(format_args!(
                    "num_notes = {}, L = s = {:#.0}",
                    mos.num_steps(),
                    self.period.repeated(mos.large_step_size),
                ))?;

                break;
            }
        }

        app.writeln("(*) means convergent i.e. the best equal-step configuration so far")?;

        Ok(())
    }
}

#[derive(Parser)]
pub(crate) struct FindGeneratorsOptions {
    /// Period of the MOS
    #[arg(long = "per", default_value = "2.0")]
    period: Ratio,

    /// Number of large steps
    num_large_steps: u16,

    /// Number of small steps
    num_small_steps: u16,
}

impl FindGeneratorsOptions {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        let [large_gen, small_gen] = [
            Mos::new_primary_step_heavy(self.num_large_steps, self.num_small_steps),
            Mos::new_secondary_step_heavy(self.num_large_steps, self.num_small_steps),
        ]
        .map(|mos| mos.genesis().primary_step());

        app.writeln(format_args!(
            "{}L{}s ({}): \
            period={:#.0}, \
            equalized_gen = {}\\{} ({:#.0}), \
            proper_gen = {}\\{} ({:#.0}), \
            collapsed_gen = {}\\{} ({:#.0})",
            self.num_large_steps,
            self.num_small_steps,
            ls_pattern(
                large_gen + small_gen,
                self.num_large_steps,
                self.num_small_steps
            ),
            self.period,
            large_gen + small_gen,
            self.num_large_steps + self.num_small_steps,
            self.period
                .repeated(large_gen + small_gen)
                .divided_into_equal_steps(self.num_large_steps + self.num_small_steps),
            2 * large_gen + small_gen,
            2 * self.num_large_steps + self.num_small_steps,
            self.period
                .repeated(2 * large_gen + small_gen)
                .divided_into_equal_steps(2 * self.num_large_steps + self.num_small_steps),
            large_gen,
            self.num_large_steps,
            self.period
                .repeated(large_gen)
                .divided_into_equal_steps(self.num_large_steps),
        ))
    }
}

#[derive(Clone, Debug)]
struct LegacyMos {
    num_large_steps: u16,
    num_small_steps: u16,
    large_step_size: f64,
    small_step_size: f64,
}

impl LegacyMos {
    fn new(generator: f64) -> Self {
        LegacyMos {
            num_large_steps: 1,
            num_small_steps: 0,
            large_step_size: 1.0,
            small_step_size: generator.rem_euclid(1.0),
        }
    }

    fn children(&self) -> impl Iterator<Item = LegacyMos> {
        iter::successors(self.child(), |mos| mos.child())
    }

    fn child(&self) -> Option<LegacyMos> {
        let mut result = self.clone();

        result.num_small_steps = result.num_small_steps.checked_add(result.num_large_steps)?;
        result.large_step_size -= result.small_step_size;

        if result.small_step_size > result.large_step_size {
            mem::swap(&mut result.large_step_size, &mut result.small_step_size);
            mem::swap(&mut result.num_large_steps, &mut result.num_small_steps);
        }

        Some(result)
    }

    fn num_steps(&self) -> u32 {
        u32::from(self.num_large_steps) + u32::from(self.num_small_steps)
    }

    fn chroma(&self) -> f64 {
        self.large_step_size - self.small_step_size
    }

    fn is_convergent(&self) -> bool {
        self.large_step_size < 2.0 * self.small_step_size
    }
}

fn ls_pattern(generator: u16, num_large_steps: u16, num_small_steps: u16) -> String {
    let num_steps = u32::from(num_large_steps) + u32::from(num_small_steps);
    let num_periods = u32::from(math::gcd_u16(num_large_steps, num_small_steps));

    let ls_period = num_steps / num_periods;

    let mut pattern = String::new();
    let mut step_offset = 0;

    for step in 0..num_steps {
        if step == u32::from(generator) {
            pattern.push('|');
        }

        pattern.push(if step >= ls_period {
            '.'
        } else if step_offset < num_large_steps {
            step_offset += num_small_steps;
            'L'
        } else {
            step_offset -= num_large_steps;
            's'
        })
    }

    pattern
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn mos_generator_ranges() {
        let mut output = String::new();

        for num_notes in 2..=50 {
            writeln!(&mut output, "---- MOSes with {num_notes} notes ----").unwrap();
            for num_large_steps in 1..num_notes {
                let num_small_steps = num_notes - num_large_steps;

                let [large_gen, small_gen] = [
                    Mos::new_primary_step_heavy(num_large_steps, num_small_steps),
                    Mos::new_secondary_step_heavy(num_large_steps, num_small_steps),
                ]
                .map(|mos| mos.genesis().primary_step());

                writeln!(
                    &mut output,
                    "{}L{}s ({}): equalized_gen = {}\\{}, proper_gen = {}\\{}, collapsed_gen = {}\\{}",
                    num_large_steps,
                    num_small_steps,
                    ls_pattern(large_gen + small_gen, num_large_steps, num_small_steps),
                    large_gen + small_gen,
                    num_large_steps + num_small_steps,
                    2 * large_gen + small_gen,
                    2 * num_large_steps + num_small_steps,
                    large_gen,
                    num_large_steps,
                )
                .unwrap();
            }
        }

        std::fs::write("../mos-generators-2-to-50.txt", &output).unwrap();
        assert_eq!(output, include_str!("../../mos-generators-2-to-50.txt"));
    }
}
