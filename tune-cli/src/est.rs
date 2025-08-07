use std::fmt;
use std::fmt::Display;
use std::io;

use clap::Parser;
use tune::layout::IsomorphicLayout;
use tune::math;
use tune::pitch::Ratio;
use tune::temperament;
use tune::temperament::CommaCatalog;
use tune::temperament::Val;

use crate::App;
use crate::CliResult;

#[derive(Parser)]
pub(crate) struct EstOptions {
    /// Size of the interval to analyze
    step_size: Ratio,

    /// Odd limit for val output
    #[arg(long = "limit", default_value = "13")]
    odd_limit: u8,

    /// Error threshold for subgroup determination
    #[arg(long = "error", default_value = "25c")]
    error_threshold: Ratio,
}

impl EstOptions {
    pub fn run(&self, app: &mut App) -> CliResult {
        let layouts = IsomorphicLayout::find_by_step_size(self.step_size);

        for print_b_val in [false, true] {
            let mut val_printed = false;

            for layout in &layouts {
                if layout.b_val() != print_b_val {
                    continue;
                }

                let mut printer = EstPrinter {
                    app,
                    val: Val::patent(self.step_size, self.odd_limit),
                    catalog: CommaCatalog::new(temperament::huygens_fokker_intervals()),
                };

                if layout.b_val() {
                    printer.val.pick_alternative(1);
                }

                let stretch = printer.val.errors().next().unwrap();

                if !val_printed {
                    printer.print_headline(printer.val.values()[0], layout.wart(), stretch)?;
                    printer.print_newline()?;

                    printer.print_basic_information(self.step_size)?;
                    printer.print_newline()?;

                    printer.print_val(self.odd_limit, self.error_threshold)?;
                    printer.print_newline()?;

                    printer.print_tempered_out_commas()?;
                    printer.print_newline()?;

                    printer.print_interval_location("septimal minor third")?;
                    printer.print_interval_location("minor third")?;
                    printer.print_interval_location("major third")?;
                    printer.print_interval_location("perfect fourth")?;
                    printer.print_interval_location("perfect fifth")?;
                    printer.print_interval_location("harmonic seventh")?;
                    printer.print_interval_location("octave")?;

                    printer.print_newline()?;

                    val_printed = true;
                }

                printer.print_generalized_notes(layout)?;
                printer.print_newline()?;
            }
        }

        Ok(())
    }
}

struct EstPrinter<'a, 'b> {
    app: &'a mut App<'b>,
    val: Val,
    catalog: CommaCatalog,
}

impl EstPrinter<'_, '_> {
    fn print_newline(&mut self) -> io::Result<()> {
        self.app.writeln("")
    }

    fn print_headline(
        &mut self,
        num_steps_per_octave: u16,
        wart: &str,
        stretch: Ratio,
    ) -> io::Result<()> {
        self.app.writeln(format_args!(
            "==== Properties of {}{}-EDO{} ====",
            num_steps_per_octave,
            wart,
            if stretch.is_negligible() {
                String::new()
            } else {
                format!(" stretched by {stretch:#}")
            },
        ))
    }

    fn print_basic_information(&mut self, step_size: Ratio) -> io::Result<()> {
        let fret_constant = step_size.as_float() / (step_size.as_float() - 1.0);
        self.app.writeln(format_args!(
            "- step size: {step_size:#}\n\
             - fret constant: {fret_constant:.3}",
        ))
    }

    fn print_val(&mut self, odd_limit: u8, threshold: Ratio) -> io::Result<()> {
        self.app
            .writeln(format_args!("---- Val ({odd_limit}-limit) ----"))?;
        self.print_newline()?;

        self.app.writeln(format_args!(
            "- notation: <{}|",
            WithSeparator(", ", || self.val.values())
        ))?;

        self.app.writeln(format_args!(
            "- errors (absolute): [{}]",
            WithSeparator(", ", || self.val.errors().map(|e| format!("{e:#}")))
        ))?;
        self.app.writeln(format_args!(
            "- errors (relative): [{}]",
            WithSeparator(", ", || self
                .val
                .errors_in_steps()
                .map(|e| format!("{:+.1}%", e * 100.0)))
        ))?;
        self.app.writeln(format_args!(
            "- TE simple badness: {:.3}‰",
            self.val.te_simple_badness() * 1000.0
        ))?;
        self.app.writeln(format_args!(
            "- subgroup: {}",
            WithSeparator(".", || self.val.subgroup(threshold))
        ))?;

        Ok(())
    }

    fn print_tempered_out_commas(&mut self) -> io::Result<()> {
        let val = &self.val;

        for &limit in math::U8_PRIMES
            .iter()
            .take_while(|&&limit| limit <= val.prime_limit())
        {
            for comma in self.catalog.commas_for_limit(limit) {
                if self.val.tempers_out(comma) {
                    if let Some((numer, denom)) = comma.as_fraction() {
                        self.app.writeln(format_args!(
                            "- tempers out {}-limit {}/{} ({})",
                            comma.prime_limit(),
                            numer,
                            denom,
                            comma.description()
                        ))?;
                    }
                }
            }
        }

        Ok(())
    }

    fn print_interval_location(&mut self, interval_name: &str) -> io::Result<()> {
        let interval = self.catalog.comma_for_name(interval_name).unwrap();
        let fraction = interval.as_fraction().unwrap();
        let tempered_location = self.val.map(interval).unwrap_or_default();
        let patent_location = interval
            .as_ratio()
            .num_equal_steps_of_size(self.val.step_size())
            .round();

        self.app.writeln(format_args!(
            "- tempered vs. patent location of {}/{}: {} vs. {}",
            fraction.0, fraction.1, tempered_location, patent_location
        ))
    }

    fn print_generalized_notes(&mut self, layout: &IsomorphicLayout) -> io::Result<()> {
        self.app
            .writeln(format_args!("==== {} notation ====", layout.genchain()))?;
        self.print_newline()?;

        self.app.writeln(format_args!(
            "- number of cycles: {}",
            layout.pergen().num_cycles()
        ))?;
        self.app.writeln(format_args!(
            "- 1 primary step = {} EDO steps",
            layout.mos().primary_step()
        ))?;
        self.app.writeln(format_args!(
            "- 1 secondary step = {} EDO steps",
            layout.mos().secondary_step()
        ))?;
        self.app.writeln(format_args!(
            "- 1 sharp (# or -) = {} EDO steps ({})",
            layout.mos().sharpness(),
            layout.get_scale_name()
        ))?;
        self.print_newline()?;

        self.app.writeln("---- Note names ----")?;
        self.print_newline()?;

        for index in 0..layout.pergen().period() {
            self.app.writeln(format_args!(
                "{:>4}. {}",
                index,
                layout.get_note_name(index)
            ))?;
        }
        self.print_newline()?;

        self.app.writeln("---- Keyboard layout ----")?;
        self.print_newline()?;

        let mos = layout.mos().coprime();
        for y in -5i16..=5 {
            for x in 0..10 {
                self.app.write(format_args!(
                    "{:>4}",
                    mos.get_key(x, y)
                        .rem_euclid(i32::from(layout.pergen().period())),
                ))?;
            }
            self.print_newline()?;
        }

        Ok(())
    }
}

struct WithSeparator<S, F>(S, F);

impl<S: Display, F: Fn() -> I, I: IntoIterator> Display for WithSeparator<S, F>
where
    I::Item: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iterator = (self.1)().into_iter();
        if let Some(first) = iterator.next() {
            write!(f, "{first}")?;
        }
        for tail in iterator {
            write!(f, "{}{}", self.0, tail)?;
        }

        Ok(())
    }
}
