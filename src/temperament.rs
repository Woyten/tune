//! Explore equal temperaments.

use crate::{
    generators::{NoteFormatter, NoteOrder, PerGen},
    ratio::Ratio,
};
use std::{convert::TryInto, fmt::Display};

#[derive(Clone, Debug)]
pub struct EqualTemperament {
    temperament_type: TemperamentType,
    primary_step: i16,
    secondary_step: i16,
    num_steps_per_fifth: u16,
    size_of_octave: Ratio,
    per_gen: PerGen,
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
        Self {
            temperament_type: TemperamentType::Meantone,
            primary_step,
            secondary_step,
            num_steps_per_fifth,
            size_of_octave: Ratio::octave(),
            per_gen: PerGen::new(num_steps_per_octave, num_steps_per_fifth),
            formatter: NoteFormatter {
                note_names: &["F", "C", "G", "D", "A", "E", "B"],
                genchain_origin: 3,
                next_cycle_sign: '#',
                prev_cycle_sign: 'b',
                sharpness: primary_step - secondary_step,
                note_order: NoteOrder::Normal,
            },
        }
    }

    pub fn porcupine(num_steps_per_octave: u16, primary_step: u16) -> EqualTemperament {
        let primary_step = primary_step.try_into().expect("primary step out of range");
        let secondary_step = (i32::from(num_steps_per_octave) - 6 * i32::from(primary_step))
            .try_into()
            .expect("secondary step out of range");
        EqualTemperament {
            temperament_type: TemperamentType::Porcupine,
            primary_step,
            secondary_step,
            num_steps_per_fifth: (i32::from(num_steps_per_octave) - 3 * i32::from(primary_step))
                .try_into()
                .expect("fifth out of range"),
            size_of_octave: Ratio::octave(),
            per_gen: PerGen::new(
                num_steps_per_octave,
                primary_step.try_into().expect("primary step out of range"),
            ),
            formatter: NoteFormatter {
                note_names: &["A", "B", "C", "D", "E", "F", "G"],
                genchain_origin: 3,
                next_cycle_sign: '-',
                prev_cycle_sign: '+',
                sharpness: secondary_step - primary_step,
                note_order: NoteOrder::Reversed,
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
        self.formatter.sharpness
    }

    pub fn num_steps_per_octave(&self) -> u16 {
        self.per_gen.period()
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
        self.per_gen.num_cycles()
    }

    pub fn get_heptatonic_name(&self, index: i32) -> String {
        self.formatter.get_name_by_step(&self.per_gen, index)
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

        let try_pocupine = match self.preference {
            TemperamentPreference::Meantone => false,
            TemperamentPreference::PorcupineWhenMeantoneIsBad => {
                !has_acceptable_qualities || temperament.sharpness() < 0
            }
            TemperamentPreference::Porcupine => true,
        };

        if try_pocupine {
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

static U8_PRIMES: &[u8] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251,
];

/// A [`Val`] is a sequence of step numbers that, multiplied by a fixed ratio, are to be considered equivalent to the prime number sequence [2, 3, 5, 7, ...].
///
/// Treating a number of steps to be equivalent to a specific total ratio is the core idea of tempering.
/// That said, a [`Val`] is an irreducible representation of the arithmetic properties of a temperament.
pub struct Val {
    values: Vec<u16>,
}

impl Val {
    /// Calculates the patent val for the given `ratio`.
    ///
    /// The patent val is the sequence of steps which, multiplied by `ratio`, provide the *best approxiation* for the prime number ratios [2, 3, 5, 7, ..., `prime_limit`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::ratio::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_12_edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 13);
    /// assert_eq!(val_of_12_edo.values(), &[12, 19, 28, 34, 42, 44]);
    ///
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// assert_eq!(val_of_17_edo.values(), &[17, 27, 39, 48, 59]);
    ///
    /// let val_of_boh_pier = Val::patent(Ratio::from_float(3.0).divided_into_equal_steps(13), 7);
    /// assert_eq!(val_of_boh_pier.values(), &[8, 13, 19, 23]);
    /// ```
    pub fn patent(ratio: Ratio, prime_limit: u8) -> Self {
        Self {
            values: U8_PRIMES
                .iter()
                .filter(|&&prime_number| prime_number <= prime_limit)
                .map(|&prime_number| {
                    Ratio::from_float(prime_number.into())
                        .num_equal_steps_of_size(ratio)
                        .round() as u16
                })
                .collect(),
        }
    }

    /// Creates a [`Val`] from the given values.
    ///
    /// [`None`] is returned if the provided list is too long.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Val;
    /// let still_okay = vec![1; 54];
    /// assert!(Val::from_values(still_okay).is_some());
    ///
    /// let too_long = vec![1; 55];
    /// assert!(Val::from_values(too_long).is_none());
    /// ```
    pub fn from_values(values: impl Into<Vec<u16>>) -> Option<Self> {
        let values = values.into();
        if values.len() > U8_PRIMES.len() {
            None
        } else {
            Some(Self { values })
        }
    }

    /// Returns the values stored in this val.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Val;
    /// let arbitrary_numbers = [5, 6, 7];
    /// let val = Val::from_values(arbitrary_numbers).unwrap();
    /// assert_eq!(val.values(), arbitrary_numbers);
    /// ```
    pub fn values(&self) -> &[u16] {
        &self.values
    }

    /// Returns the prime limit of this val.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Val;
    /// let custom_val = Val::from_values([12, 19, 28, 34, 42]).unwrap();
    /// assert_eq!(custom_val.prime_limit(), 11);
    /// ```
    pub fn prime_limit(&self) -> u8 {
        if self.values.len() == 0 {
            0
        } else {
            U8_PRIMES[self.values.len() - 1]
        }
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
                    temperament.get_heptatonic_name(index.into())
                )
                .unwrap();
            }
        }
        std::fs::write("edo-notes-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-notes-1-to-99.txt"));
    }
}
