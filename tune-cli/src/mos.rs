use std::{cmp::Ordering, convert::TryFrom, io, iter, mem};

use structopt::StructOpt;
use tune::{math, pitch::Ratio};

use crate::App;

#[derive(StructOpt)]
pub(crate) enum MosCommand {
    /// Find MOSes for a given generator
    #[structopt(name = "find")]
    FindMoses(FindMosesOptions),

    /// Find generators for a given MOS
    #[structopt(name = "gen")]
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

#[derive(StructOpt)]
pub(crate) struct FindMosesOptions {
    /// Period of the MOS
    #[structopt(long = "per", default_value = "2.0")]
    period: Ratio,

    /// Generator of the MOS
    generator: Ratio,

    /// Chroma size below which the scale is considered an equal-step scale
    #[structopt(long = "chroma", default_value = "0.5c")]
    threshold: Ratio,
}

impl FindMosesOptions {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        for mos in Mos::new(self.generator.num_equal_steps_of_size(self.period)).children() {
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

        app.writeln("(*) means convergent i.e. the best EDO configuration so far")?;

        Ok(())
    }
}

#[derive(StructOpt)]
pub(crate) struct FindGeneratorsOptions {
    /// Period of the MOS
    #[structopt(long = "per", default_value = "2.0")]
    period: Ratio,

    /// Number of large steps
    num_large_steps: u16,

    /// Number of small steps
    num_small_steps: u16,
}

impl FindGeneratorsOptions {
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        let (equalized, paucitonic) = get_gen_range(self.num_large_steps, self.num_small_steps);

        app.writeln(format_args!(
            "{}L{}s ({}): \
            period={:#.0}, \
            equalized_gen = {}\\{} ({:#.0}), \
            proper_gen = {}\\{} ({:#.0}), \
            paucitonic_gen = {}\\{} ({:#.0})",
            self.num_large_steps,
            self.num_small_steps,
            ls_pattern(
                usize::try_from(equalized.0).unwrap(),
                self.num_large_steps,
                self.num_small_steps
            ),
            self.period,
            equalized.0,
            equalized.1,
            self.period
                .repeated(equalized.0)
                .divided_into_equal_steps(equalized.1),
            equalized.0 + paucitonic.0,
            equalized.1 + paucitonic.1,
            self.period
                .repeated(equalized.0 + paucitonic.0)
                .divided_into_equal_steps(equalized.1 + paucitonic.1),
            paucitonic.0,
            paucitonic.1,
            self.period
                .repeated(paucitonic.0)
                .divided_into_equal_steps(paucitonic.1),
        ))
    }
}

#[derive(Clone, Debug)]
struct Mos {
    num_large_steps: u16,
    num_small_steps: u16,
    large_step_size: f64,
    small_step_size: f64,
}

impl Mos {
    fn new(generator: f64) -> Self {
        Mos {
            num_large_steps: 1,
            num_small_steps: 0,
            large_step_size: 1.0,
            small_step_size: generator.rem_euclid(1.0),
        }
    }

    fn equalized(num_large_steps: u16, num_small_steps: u16) -> Self {
        let num_steps = f64::from(num_large_steps) + f64::from(num_small_steps);

        Self {
            num_large_steps,
            num_small_steps,
            large_step_size: 1.0 / num_steps,
            small_step_size: 1.0 / num_steps,
        }
    }

    fn paucitonic(num_large_steps: u16, num_small_steps: u16) -> Self {
        Self {
            num_large_steps,
            num_small_steps,
            large_step_size: 1.0 / f64::from(num_large_steps),
            small_step_size: 0.0,
        }
    }

    fn children(&self) -> impl Iterator<Item = Mos> {
        let mut mos = self.clone();

        iter::from_fn(move || {
            let child = mos.child();
            if let Some(child) = &child {
                mos = child.clone();
            }
            child
        })
    }

    fn child(&self) -> Option<Mos> {
        let mut result = self.clone();

        result.num_small_steps = result.num_small_steps.checked_add(result.num_large_steps)?;
        result.large_step_size -= result.small_step_size;

        if result.small_step_size > result.large_step_size {
            mem::swap(&mut result.large_step_size, &mut result.small_step_size);
            mem::swap(&mut result.num_large_steps, &mut result.num_small_steps);
        }

        Some(result)
    }

    fn genesis_mos(&self) -> Mos {
        let mut mos = self.clone();

        loop {
            let parent = mos.parent();
            if let Some(parent) = parent {
                mos = parent;
            } else {
                return mos;
            }
        }
    }

    fn parent(&self) -> Option<Mos> {
        if self.num_large_steps == 0 || self.num_small_steps == 0 {
            return None;
        }

        match self.num_large_steps.cmp(&self.num_small_steps) {
            Ordering::Greater => Some(Self {
                num_large_steps: self.num_small_steps,
                num_small_steps: self.num_large_steps - self.num_small_steps,
                large_step_size: self.large_step_size + self.small_step_size,
                small_step_size: self.large_step_size,
            }),
            Ordering::Less => Some(Self {
                num_large_steps: self.num_large_steps,
                num_small_steps: self.num_small_steps - self.num_large_steps,
                large_step_size: self.large_step_size + self.small_step_size,
                small_step_size: self.small_step_size,
            }),
            Ordering::Equal => None,
        }
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

fn ls_pattern(l_generator: usize, num_large_steps: u16, num_small_steps: u16) -> String {
    let num_steps = usize::from(num_large_steps) + usize::from(num_small_steps);
    let num_periods = usize::from(math::gcd_u16(num_large_steps, num_small_steps));

    let num_generations = usize::from(num_large_steps) / num_periods;
    let reduced_num_steps = num_steps / num_periods;

    let mut pattern = vec![b'.'; num_steps];

    for (generation, symbol) in iter::repeat(b'L')
        .take(num_generations)
        .chain(iter::repeat(b's'))
        .take(reduced_num_steps)
        .enumerate()
    {
        pattern[(generation * l_generator % reduced_num_steps)] = symbol;
    }

    pattern.insert(l_generator, b'|');

    String::from_utf8(pattern).unwrap()
}

fn get_gen_range(num_large_steps: u16, num_small_steps: u16) -> ((u32, u32), (u32, u32)) {
    let equalized_gen = Mos::equalized(num_large_steps, num_small_steps).genesis_mos();
    let paucitonic_gen = Mos::paucitonic(num_large_steps, num_small_steps).genesis_mos();

    let num_steps = u32::from(num_large_steps) + u32::from(num_small_steps);

    let (equalized_step, paucitonic_step) =
        if equalized_gen.large_step_size < paucitonic_gen.large_step_size {
            (
                equalized_gen.large_step_size,
                paucitonic_gen.large_step_size,
            )
        } else {
            (
                equalized_gen.small_step_size,
                paucitonic_gen.small_step_size,
            )
        };

    let equalized = (
        (equalized_step * f64::from(num_steps)).round() as u32,
        num_steps,
    );
    let paucitonic = (
        (paucitonic_step * f64::from(num_large_steps)).round() as u32,
        u32::from(num_large_steps),
    );

    (equalized, paucitonic)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn mos_generator_ranges() {
        let mut output = String::new();

        for num_notes in 2..=50 {
            writeln!(&mut output, "---- MOSes with {} notes ----", num_notes).unwrap();
            for num_large_steps in 1..num_notes {
                let num_small_steps = num_notes - num_large_steps;

                let (equalized, paucitonic) = get_gen_range(num_large_steps, num_small_steps);

                writeln!(
                    &mut output,
                    "{}L{}s ({}): equalized_gen = {}\\{}, proper_gen = {}\\{}, paucitonic_gen = {}\\{}",
                    num_large_steps,
                    num_small_steps,
                    ls_pattern(usize::try_from(equalized.0).unwrap(), num_large_steps, num_small_steps),
                    equalized.0,
                    equalized.1,
                    equalized.0 + paucitonic.0,
                    equalized.1 + paucitonic.1,
                    paucitonic.0,
                    paucitonic.1,
                )
                .unwrap();
            }
        }

        std::fs::write("../mos-generators-2-to-50.txt", &output).unwrap();
        assert_eq!(output, include_str!("../../mos-generators-2-to-50.txt"));
    }
}
