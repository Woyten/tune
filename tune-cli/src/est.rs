use std::{
    cmp::Ordering,
    fmt::{self, Display},
    io,
};

use clap::Parser;
use tune::{
    layout::{EqualTemperament, PrototypeTemperament},
    math,
    pitch::Ratio,
    temperament::{self, CommaCatalog, Val},
};

use crate::App;

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
    pub fn run(&self, app: &mut App) -> io::Result<()> {
        let mut patent_val_printed = false;
        let mut non_patent_val_printed = false;

        for temperament in EqualTemperament::find().by_step_size(self.step_size) {
            let mut printer = EstPrinter {
                app,
                val: Val::patent(self.step_size, self.odd_limit),
                catalog: CommaCatalog::new(temperament::huygens_fokker_intervals()),
            };

            if temperament.alt_tritave() {
                printer.val.pick_alternative(1);
            }

            let stretch = printer.val.errors().next().unwrap();

            if !patent_val_printed && !temperament.alt_tritave()
                || !non_patent_val_printed && temperament.alt_tritave()
            {
                printer.print_headline(printer.val.values()[0], temperament.wart(), stretch)?;
                printer.print_newline()?;

                printer.print_basic_information(self.step_size)?;
                printer.print_newline()?;

                printer.print_val(self.odd_limit, self.error_threshold)?;
                printer.print_newline()?;

                printer.print_matching_temperament("syntonic comma", "meantone")?;
                printer.print_matching_temperament("major chroma", "mavila")?;
                printer.print_matching_temperament("porcupine comma", "porcupine")?;
                printer.print_tempered_out_commas()?;
                printer.print_newline()?;

                printer.print_interval_location("septimal minor third")?;
                printer.print_interval_location("minor third")?;
                printer.print_interval_location("major third")?;
                printer.print_interval_location("perfect fourth")?;
                printer.print_interval_location("perfect fifth")?;
                printer.print_interval_location("harmonic seventh")?;
                printer.print_interval_location("octave")?;

                patent_val_printed |= !temperament.alt_tritave();
                non_patent_val_printed |= temperament.alt_tritave();
                printer.print_newline()?;
            }

            printer.print_generalized_notes(&temperament)?;
            printer.print_newline()?;
        }

        Ok(())
    }
}

struct EstPrinter<'a, 'b> {
    app: &'a mut App<'b>,
    val: Val,
    catalog: CommaCatalog,
}

impl<'a, 'b> EstPrinter<'a, 'b> {
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

    fn print_matching_temperament(
        &mut self,
        comma_name: &str,
        temperament_name: &str,
    ) -> io::Result<()> {
        if self
            .val
            .tempers_out(self.catalog.comma_for_name(comma_name).unwrap())
        {
            self.app
                .writeln(format_args!("- supports {temperament_name} temperament"))?;
        }

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

    fn print_generalized_notes(&mut self, temperament: &EqualTemperament) -> io::Result<()> {
        let mos_type = match (temperament.sharpness().cmp(&0), temperament.prototype()) {
            (Ordering::Equal, _) => "equalized",
            (Ordering::Greater, PrototypeTemperament::Meantone7) => "diatonic",
            (Ordering::Less, PrototypeTemperament::Meantone7) => "antidiatonic",
            (Ordering::Greater, PrototypeTemperament::Mavila9) => "armotonic",
            (Ordering::Less, PrototypeTemperament::Mavila9) => "balzano",
            (Ordering::Greater, PrototypeTemperament::Porcupine7) => "archeotonic",
            (Ordering::Less, PrototypeTemperament::Porcupine7) => "onyx",
            (Ordering::Greater, PrototypeTemperament::Porcupine8) => "pine",
            (Ordering::Less, PrototypeTemperament::Porcupine8) => "antipine",
        };

        self.app.writeln(format_args!(
            "==== {} notation ====",
            temperament.prototype()
        ))?;
        self.print_newline()?;

        self.app.writeln(format_args!(
            "- number of cycles: {}",
            temperament.pergen().num_cycles()
        ))?;
        self.app.writeln(format_args!(
            "- 1 primary step = {} EDO steps",
            temperament.primary_step()
        ))?;
        self.app.writeln(format_args!(
            "- 1 secondary step = {} EDO steps",
            temperament.secondary_step()
        ))?;
        self.app.writeln(format_args!(
            "- 1 sharp (# or -) = {} EDO steps ({})",
            temperament.sharpness(),
            mos_type
        ))?;
        self.print_newline()?;

        let keyboard = temperament.get_keyboard();

        self.app.writeln("---- Note names ----")?;
        self.print_newline()?;

        for index in 0..temperament.pergen().period() {
            self.app.writeln(format_args!(
                "{:>4}. {}",
                index,
                temperament.get_note_name(index)
            ))?;
        }
        self.print_newline()?;

        self.app.writeln("---- Keyboard layout ----")?;
        self.print_newline()?;

        for y in -5i16..=5 {
            for x in 0..10 {
                self.app.write(format_args!(
                    "{:>4}",
                    keyboard
                        .get_key(x, y)
                        .rem_euclid(i32::from(temperament.pergen().period())),
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
