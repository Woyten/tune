//! Explore equal temperaments and vals.

use std::fmt::Display;

use crate::{
    comma::Comma,
    math,
    pergen::{AccidentalsFormat, AccidentalsOrder, NoteFormatter, PerGen},
    pitch::Ratio,
};

#[derive(Clone, Debug)]
pub struct EqualTemperament {
    temperament_type: TemperamentType,
    primary_step: i16,
    secondary_step: i16,
    num_steps_per_fifth: u16,
    size_of_octave: Ratio,
    pergen: PerGen,
    acc_format: AccidentalsFormat,
    formatter: NoteFormatter,
}

impl EqualTemperament {
    pub fn meantone(num_steps_per_octave: u16, num_steps_per_fifth: u16) -> Self {
        let primary_step = (2 * i32::from(num_steps_per_fifth) - i32::from(num_steps_per_octave))
            .try_into()
            .expect("primary step out of range");
        let secondary_step = (3 * i32::from(num_steps_per_octave)
            - 5 * i32::from(num_steps_per_fifth))
        .try_into()
        .expect("secondary step out of range");
        let sharpness = primary_step - secondary_step;

        Self {
            temperament_type: TemperamentType::Meantone,
            primary_step,
            secondary_step,
            num_steps_per_fifth,
            size_of_octave: Ratio::octave(),
            pergen: PerGen::new(num_steps_per_octave, num_steps_per_fifth),
            acc_format: AccidentalsFormat {
                num_symbols: 7,
                genchain_origin: 3,
            },
            formatter: NoteFormatter {
                note_names: ['F', 'C', 'G', 'D', 'A', 'E', 'B'][..].into(),
                sharp_sign: sharp_sign_from_sharpness(sharpness),
                flat_sign: flat_sign_from_sharpness(sharpness),
                order: AccidentalsOrder::from_sharpness(sharpness),
            },
        }
    }

    pub fn porcupine(num_steps_per_octave: u16, primary_step: u16) -> EqualTemperament {
        let primary_step = primary_step.try_into().expect("primary step out of range");
        let secondary_step = (i32::from(num_steps_per_octave) - 6 * i32::from(primary_step))
            .try_into()
            .expect("secondary step out of range");
        let sharpness = primary_step - secondary_step;

        EqualTemperament {
            temperament_type: TemperamentType::Porcupine,
            primary_step,
            secondary_step,
            num_steps_per_fifth: (i32::from(num_steps_per_octave) - 3 * i32::from(primary_step))
                .try_into()
                .expect("fifth out of range"),
            size_of_octave: Ratio::octave(),
            pergen: PerGen::new(
                num_steps_per_octave,
                primary_step.try_into().expect("primary step out of range"),
            ),
            acc_format: AccidentalsFormat {
                num_symbols: 7,
                genchain_origin: 3,
            },
            formatter: NoteFormatter {
                note_names: ['A', 'B', 'C', 'D', 'E', 'F', 'G'][..].into(),
                sharp_sign: sharp_sign_from_sharpness(sharpness),
                flat_sign: flat_sign_from_sharpness(sharpness),
                order: AccidentalsOrder::from_sharpness(sharpness),
            },
        }
    }

    pub fn with_size_of_octave(mut self, size_of_octave: Ratio) -> Self {
        self.size_of_octave = size_of_octave;
        self
    }

    pub fn find() -> TemperamentFinder {
        TemperamentFinder {
            second_best_fifth_allowed: true,
            preference: TemperamentPreference::PorcupineWhenMeantoneIsBad,
        }
    }

    pub fn as_porcupine(&self) -> Option<EqualTemperament> {
        let num_steps_of_fourth = self.num_steps_per_octave() - self.num_steps_per_fifth();
        if num_steps_of_fourth % 3 == 0 {
            Some(
                Self::porcupine(self.num_steps_per_octave(), num_steps_of_fourth / 3)
                    .with_size_of_octave(self.size_of_octave()),
            )
        } else {
            None
        }
    }

    pub fn temperament_type(&self) -> TemperamentType {
        self.temperament_type
    }

    pub fn primary_step(&self) -> i16 {
        self.primary_step
    }

    pub fn secondary_step(&self) -> i16 {
        self.secondary_step
    }

    pub fn sharpness(&self) -> i16 {
        self.primary_step - self.secondary_step
    }

    pub fn num_steps_per_octave(&self) -> u16 {
        self.pergen.period()
    }

    pub fn size_of_octave(&self) -> Ratio {
        self.size_of_octave
    }

    pub fn num_steps_per_fifth(&self) -> u16 {
        self.num_steps_per_fifth
    }

    pub fn size_of_fifth(&self) -> Ratio {
        self.size_of_octave
            .divided_into_equal_steps(self.num_steps_per_octave())
            .repeated(self.num_steps_per_fifth())
    }

    pub fn num_cycles(&self) -> u16 {
        self.pergen.num_cycles()
    }

    pub fn get_heptatonic_name(&self, index: u16) -> String {
        self.formatter
            .format(&self.pergen.get_accidentals(&self.acc_format, index))
    }
}

fn sharp_sign_from_sharpness(sharpness: i16) -> char {
    if sharpness >= 0 {
        '#'
    } else {
        '-'
    }
}

fn flat_sign_from_sharpness(sharpness: i16) -> char {
    if sharpness >= 0 {
        'b'
    } else {
        '+'
    }
}

pub struct TemperamentFinder {
    second_best_fifth_allowed: bool,
    preference: TemperamentPreference,
}

impl TemperamentFinder {
    pub fn with_second_best_fifth_allowed(mut self, flat_fifth_allowed: bool) -> Self {
        self.second_best_fifth_allowed = flat_fifth_allowed;
        self
    }

    pub fn with_preference(mut self, preference: TemperamentPreference) -> Self {
        self.preference = preference;
        self
    }

    pub fn by_edo(&self, num_steps_per_octave: impl Into<f64>) -> EqualTemperament {
        self.by_step_size(Ratio::octave().divided_into_equal_steps(num_steps_per_octave))
    }

    pub fn by_step_size(&self, step_size: Ratio) -> EqualTemperament {
        let num_steps_per_octave =
            Ratio::octave().num_equal_steps_of_size(step_size).round() as u16;
        let best_fifth = Ratio::from_float(1.5)
            .num_equal_steps_of_size(step_size)
            .round() as u16;

        self.from_starting_point(num_steps_per_octave, best_fifth)
            .with_size_of_octave(step_size.repeated(num_steps_per_octave))
    }

    fn from_starting_point(&self, num_steps_per_octave: u16, best_fifth: u16) -> EqualTemperament {
        let (best_fifth_temperament, has_acceptable_qualities) =
            self.create_and_rate_temperament(num_steps_per_octave, best_fifth);
        if has_acceptable_qualities {
            return best_fifth_temperament;
        }

        if self.second_best_fifth_allowed && best_fifth > 0 {
            let (flat_fifth_temperament, has_acceptable_qualities) =
                self.create_and_rate_temperament(num_steps_per_octave, best_fifth - 1);
            if has_acceptable_qualities {
                return flat_fifth_temperament;
            }
        }

        best_fifth_temperament
    }

    fn create_and_rate_temperament(
        &self,
        num_steps_per_octave: u16,
        num_steps_per_fifth: u16,
    ) -> (EqualTemperament, bool) {
        let temperament = EqualTemperament::meantone(num_steps_per_octave, num_steps_per_fifth);
        let has_acceptable_qualities =
            temperament.primary_step() > 0 && temperament.secondary_step() >= 0;

        let try_porcupine = match self.preference {
            TemperamentPreference::Meantone => false,
            TemperamentPreference::PorcupineWhenMeantoneIsBad => {
                !has_acceptable_qualities || temperament.sharpness() < 0
            }
            TemperamentPreference::Porcupine => true,
        };

        if try_porcupine {
            if let Some(porcupine) = temperament.as_porcupine() {
                if porcupine.secondary_step() >= 0 {
                    return (porcupine, true);
                }
            }
        }

        (temperament, has_acceptable_qualities)
    }
}

pub enum TemperamentPreference {
    /// Always choose meantone even if it will have bad qualities e.g. `secondary_step < 0`.
    Meantone,
    /// Try to fall back to porcupine when meantone would have bad qualities.
    PorcupineWhenMeantoneIsBad,
    /// Use porcupine whenever possible.
    Porcupine,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TemperamentType {
    /// Octave-reduced temperament treating 4 fifths to be equal to one major third.
    ///
    /// The major third can be divided into two equal parts which form the natural or *primary steps* of the scale.
    ///
    /// The note names are derived from the genchain of fifths [ &hellip; Bb F C G D A E B F# &hellip; ].
    /// This results in standard music notation with G at one fifth above C and D at two fifths == 1/2 major third == 1 primary step above C.
    Meantone,

    /// Octave-reduced temperament treating 3 major thirds to be equal to 5 fifths.
    ///
    /// This temperament is best described in terms of *primary steps* three of which form a fourth.
    /// A primary step can formally be considered a minor second but in terms of just ratios may be closer to a major second.
    ///
    /// The note names are derived from the genchain of primary steps [ &hellip; G# A B C D E F G Ab &hellip; ].
    /// In contrast to meantone, the intervals E-F and F-G have the same size of one primary step while G-A is different, usually larger.
    Porcupine,
}

impl Display for TemperamentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_name = match self {
            TemperamentType::Meantone => "Meantone",
            TemperamentType::Porcupine => "Porcupine",
        };
        write!(f, "{}", display_name)
    }
}

/// A [`Val`] is a step size and a sequence of step numbers that, multiplied component-wise, are to be considered equivalent to the prime number sequence [2, 3, 5, 7, ...].
///
/// Treating a number of steps to be equivalent to a specific total ratio is the core idea of tempering.
/// That said, a val is an irreducible representation of the arithmetic properties of a temperament's generator.
pub struct Val {
    step_size: Ratio,
    values: Vec<u16>,
}

impl Val {
    /// Creates a [`Val`] from the given values.
    ///
    /// [`None`] is returned if the provided list is too long.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let still_okay = vec![1; 54];
    /// assert!(Val::create(Ratio::from_semitones(1), still_okay).is_some());
    ///
    /// let too_long = vec![1; 55];
    /// assert!(Val::create(Ratio::from_semitones(1), too_long).is_none());
    /// ```
    pub fn create(step_size: Ratio, values: impl Into<Vec<u16>>) -> Option<Self> {
        let values = values.into();
        if values.len() > math::U8_PRIMES.len() {
            None
        } else {
            Some(Self { step_size, values })
        }
    }

    /// Calculates the patent [`Val`] for the given `step_size`.
    ///
    /// The patent val is the sequence of steps which, multiplied by `step_size`, provide the *best approximation* for the prime number ratios [2, 3, 5, 7, ..., `prime_limit`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_12_edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 13);
    /// assert_eq!(val_of_12_edo.values(), &[12, 19, 28, 34, 42, 44]);
    ///
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// assert_eq!(val_of_17_edo.values(), &[17, 27, 39, 48, 59]);
    ///
    /// let val_of_13_edt = Val::patent(Ratio::from_float(3.0).divided_into_equal_steps(13), 7);
    /// assert_eq!(val_of_13_edt.values(), &[8, 13, 19, 23]);
    /// ```
    pub fn patent(step_size: Ratio, prime_limit: u8) -> Self {
        Self {
            step_size,
            values: math::U8_PRIMES
                .iter()
                .filter(|&&prime_number| prime_number <= prime_limit)
                .map(|&prime_number| {
                    Ratio::from_float(prime_number.into())
                        .num_equal_steps_of_size(step_size)
                        .round() as u16
                })
                .collect(),
        }
    }

    /// Returns the step size stored in this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let some_step_size = Ratio::octave().divided_into_equal_steps(17);
    /// let val = Val::create(some_step_size, []).unwrap();
    /// assert_eq!(val.step_size(), some_step_size);
    /// ```
    pub fn step_size(&self) -> Ratio {
        self.step_size
    }

    /// Returns the values stored in this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let some_numbers = [5, 6, 7];
    /// let val = Val::create(Ratio::from_semitones(1), some_numbers).unwrap();
    /// assert_eq!(val.values(), some_numbers);
    /// ```
    pub fn values(&self) -> &[u16] {
        &self.values
    }

    /// Returns the prime limit of this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let custom_val = Val::create(Ratio::from_semitones(1), [12, 19, 28, 34, 42]).unwrap();
    /// assert_eq!(custom_val.prime_limit(), 11);
    /// ```
    pub fn prime_limit(&self) -> u8 {
        if self.values.is_empty() {
            1
        } else {
            math::U8_PRIMES[self.values.len() - 1]
        }
    }

    /// Returns the current [`Val`]s absolute errors i.e. the deviation from the prime number ratios.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let errors = Vec::from_iter(val_of_17_edo.errors().map(Ratio::as_cents));
    ///
    /// assert_eq!(errors.len(), 5);
    /// assert_approx_eq!(errors[0], 0.0);
    /// assert_approx_eq!(errors[1], 3.927352);
    /// assert_approx_eq!(errors[2], -33.372537);
    /// assert_approx_eq!(errors[3], 19.409388);
    /// assert_approx_eq!(errors[4], 13.387940);
    /// ```

    pub fn errors(&self) -> impl Iterator<Item = Ratio> + '_ {
        self.values
            .iter()
            .zip(math::U8_PRIMES)
            .map(move |(&value, &prime)| {
                self.step_size
                    .repeated(value)
                    .deviation_from(Ratio::from_float(f64::from(prime)))
            })
    }

    /// Returns the current [`Val`]'s errors where the unit of measurement is one `step_size`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let errors_in_steps = Vec::from_iter(val_of_17_edo.errors_in_steps());
    ///
    /// assert_eq!(errors_in_steps.len(), 5);
    /// assert_approx_eq!(errors_in_steps[0] * 100.0, 0.0);
    /// assert_approx_eq!(errors_in_steps[1] * 100.0, 5.563749);
    /// assert_approx_eq!(errors_in_steps[2] * 100.0, -47.277761);
    /// assert_approx_eq!(errors_in_steps[3] * 100.0, 27.496633);
    /// assert_approx_eq!(errors_in_steps[4] * 100.0, 18.966248);
    /// ```

    pub fn errors_in_steps(&self) -> impl Iterator<Item = f64> + '_ {
        self.errors()
            .map(move |error_abs| error_abs.num_equal_steps_of_size(self.step_size))
    }

    /// Calculates the Tenney-Euclidean simple badness.
    ///
    /// # Example
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::temperament::Val;
    /// # use tune::pitch::Ratio;
    /// let step_size_12_edo = Ratio::octave().divided_into_equal_steps(12);
    /// assert_approx_eq!(Val::patent(step_size_12_edo, 11).te_simple_badness() * 1000.0, 35.760225);
    ///
    /// let step_size_19_edo = Ratio::octave().divided_into_equal_steps(19);
    /// assert_approx_eq!(Val::patent(step_size_19_edo, 11).te_simple_badness() * 1000.0, 28.495822);
    /// ```
    pub fn te_simple_badness(&self) -> f64 {
        self.errors_in_steps()
            .zip(math::U8_PRIMES)
            .map(|(error_in_steps, &prime)| {
                let error_in_primes =
                    error_in_steps / Ratio::from_float(f64::from(prime)).as_octaves();
                error_in_primes * error_in_primes
            })
            .sum::<f64>()
    }

    /// Returns the current [`Val`]s subgroup with the absolute errors below the given `threshold`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let subgroup = Vec::from_iter(val_of_17_edo.subgroup(Ratio::from_cents(25.0)));
    ///
    /// assert_eq!(subgroup, [2, 3, 7, 11]);
    /// ```
    pub fn subgroup(&self, threshold: Ratio) -> impl IntoIterator<Item = u8> + '_ {
        self.errors()
            .zip(math::U8_PRIMES)
            .filter(move |&(error, _)| error.as_cents().abs() < threshold.as_cents().abs())
            .map(|(_, &prime)| prime)
    }

    /// Applies the temperament's mapping function to the given [`Comma`].
    ///
    /// Specifically, it calculates the scalar product of the values of `self` and the values of the `comma` if the prime limit of `self` is at least the prime of `comma`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::comma::Comma;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let fifth = Comma::new("fifth", &[-1, 1][..]);
    /// assert_eq!(fifth.as_fraction(), Some((3, 2)));
    ///
    /// // The 12-edo fifth is at 7 steps
    /// let val_of_12edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 5);
    /// assert_eq!(val_of_12edo.map(&fifth), Some(7));
    ///
    /// // The 31-edo fifth is at 18 steps
    /// let val_of_31edo = Val::patent(Ratio::octave().divided_into_equal_steps(31), 5);
    /// assert_eq!(val_of_31edo.map(&fifth), Some(18));
    ///
    /// // 7-limit intervals cannot be represented by a 5-limit val
    /// let seventh = Comma::new("seventh", &[-2, 0, 0, 1][..]);
    /// assert_eq!(seventh.as_fraction(), Some((7, 4)));
    /// assert_eq!(val_of_12edo.map(&seventh), None);
    /// ```
    pub fn map(&self, comma: &Comma) -> Option<i32> {
        (self.prime_limit() >= comma.prime_limit()).then(|| {
            self.values
                .iter()
                .zip(comma.prime_factors())
                .map(|(&v, &c)| i32::from(v) * i32::from(c))
                .sum()
        })
    }

    /// Checks whether the current [`Val`] defines a rank-1 temperament which tempers out the given [`Comma`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::comma::Comma;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let diesis = Comma::new("diesis", &[7, 0, -3][..]);
    /// assert_eq!(diesis.as_fraction(), Some((128, 125)));
    ///
    /// // 12-edo tempers out the diesis
    /// let val_of_12edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 5);
    /// assert!(val_of_12edo.tempers_out(&diesis));
    ///
    /// // 31-edo does not temper out the diesis
    /// let val_of_31edo = Val::patent(Ratio::octave().divided_into_equal_steps(31), 5);
    /// assert!(!val_of_31edo.tempers_out(&diesis));
    /// ```
    pub fn tempers_out(&self, comma: &Comma) -> bool {
        self.map(comma) == Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn note_names() {
        let mut output = String::new();
        for num_steps_per_octave in 1u16..100 {
            let temperament = EqualTemperament::find().by_edo(num_steps_per_octave);
            writeln!(
                output,
                "---- {}-EDO ({}) ----",
                num_steps_per_octave,
                temperament.temperament_type()
            )
            .unwrap();
            writeln!(
                output,
                "primary_step={}, secondary_step={}, sharpness={}, num_cycles={}",
                temperament.primary_step(),
                temperament.secondary_step(),
                temperament.sharpness(),
                temperament.num_cycles(),
            )
            .unwrap();
            for index in 0..num_steps_per_octave {
                writeln!(
                    output,
                    "{} - {}",
                    index,
                    temperament.get_heptatonic_name(index)
                )
                .unwrap();
            }
        }
        std::fs::write("edo-notes-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-notes-1-to-99.txt"));
    }
}
