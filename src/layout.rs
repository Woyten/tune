//! Find generator chains and keyboard layouts.

use std::fmt::{self, Display};

use crate::{
    math,
    pergen::{Accidentals, AccidentalsFormat, AccidentalsOrder, Mos, NoteFormatter, PerGen},
    pitch::Ratio,
    temperament::Val,
};

/// A type that encapsulates the rules for generating and laying out a scale with a specified step size.
#[derive(Clone, Debug)]
pub struct EqualTemperament {
    prototype: PrototypeTemperament,
    alt_tritave: bool,
    pergen: PerGen,
    mos: Mos,
    acc_format: AccidentalsFormat,
    formatter: NoteFormatter,
}

impl EqualTemperament {
    pub fn find() -> TemperamentFinder {
        TemperamentFinder {
            preference: vec![
                vec![
                    // Prefer Meantone7 since it is ubiquitous in musical notation
                    PrototypeTemperament::Meantone7,
                    // Afterwards, prefer notation systems with more notes
                    PrototypeTemperament::Mavila9,
                    PrototypeTemperament::Meantone5,
                ],
                vec![
                    PrototypeTemperament::Porcupine8,
                    PrototypeTemperament::Porcupine7,
                ],
            ],
        }
    }

    pub fn prototype(&self) -> PrototypeTemperament {
        self.prototype
    }

    pub fn alt_tritave(&self) -> bool {
        self.alt_tritave
    }

    pub fn wart(&self) -> &'static str {
        if self.alt_tritave {
            "b"
        } else {
            ""
        }
    }

    pub fn pergen(&self) -> &PerGen {
        &self.pergen
    }

    pub fn mos(&self) -> Mos {
        self.mos
    }

    /// Obtains the note name for the given degree of the current temperament.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::EqualTemperament;
    /// let positive_sharpness = EqualTemperament::find().by_edo(31).into_iter().next().unwrap();
    ///
    /// assert_eq!(positive_sharpness.get_note_name(0), "D");
    /// assert_eq!(positive_sharpness.get_note_name(1), "Ebb");
    /// assert_eq!(positive_sharpness.get_note_name(18), "A");
    /// assert_eq!(positive_sharpness.get_note_name(25), "B#");
    ///
    /// let negative_sharpness = EqualTemperament::find().by_edo(16).into_iter().skip(1).next().unwrap();
    ///
    /// assert_eq!(negative_sharpness.get_note_name(0), "D");
    /// assert_eq!(negative_sharpness.get_note_name(1), "D+/E-");
    /// assert_eq!(negative_sharpness.get_note_name(9), "A");
    /// assert_eq!(negative_sharpness.get_note_name(12), "B+");
    /// ```
    pub fn get_note_name(&self, index: u16) -> String {
        self.formatter.format(&self.get_accidentals(index))
    }

    pub fn get_accidentals(&self, index: u16) -> Accidentals {
        self.pergen.get_accidentals(&self.acc_format, index)
    }

    pub fn get_keyboard(&self) -> IsomorphicKeyboard {
        IsomorphicKeyboard {
            primary_step: self.mos.primary_step(),
            secondary_step: self.mos.secondary_step(),
        }
        .coprime()
    }

    /// Generate an automatic color schema for the given temperament.
    ///
    /// The resulting color schema is arranged in layers, with the innermost layer representing the natural notes and the outermost layer representing the most enharmonic notes, if any.
    ///
    /// The intermediate layers as well as the enharmonic layer contain the notes between the natural ones and use the same shape as the primary and secondary sub-scale or the full natural scale.
    ///
    /// The total number of layers depends on the larger of the primary and secondary step sizes of the given temperament.
    ///
    /// # Return Value
    ///
    /// The color schema is returned as a `Vec` of abstract indexes that the method caller can use to look up the final color.
    /// The color indexes are returned in genchain order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::EqualTemperament;
    /// // Color layers of 31-EDO: 7 (n) + 7 (#) + 7 (b) + 5 (##) + 5 (bb)
    /// assert_eq!(
    ///     EqualTemperament::find().by_edo(31).into_iter().next().unwrap().get_colors(),
    ///     &[
    ///         0, 0, 0, 0,          // Neutral layer (D, A, E, B)
    ///         1, 1, 1, 1, 1, 1, 1, // Sharp layer
    ///         3, 3, 3, 3, 3,       // Double-sharp layer
    ///         4, 4, 4, 4, 4,       // Double-flat layer
    ///         2, 2, 2, 2, 2, 2, 2, // Flat layer
    ///         0, 0, 0,             // Neutral layer (F, C, G)
    ///     ]
    /// );
    ///
    /// // Color layers of 19-EDO: 7 (n) + 5 (#) + 5 (b) + 2 (enharmonic)
    /// assert_eq!(
    ///     EqualTemperament::find().by_edo(19).into_iter().next().unwrap().get_colors(),
    ///     &[
    ///         0, 0, 0, 0,    // Neutral layer (D, A, E, B)
    ///         1, 1, 1, 1, 1, // Sharp layer
    ///         3, 3,          // Enharmonic layer
    ///         2, 2, 2, 2, 2, // Flat layer
    ///         0, 0, 0,       // Neutral layer (F, C, G)
    ///     ]
    /// );
    ///
    /// // Color layers of 24-EDO: 7 (n) + 5 (enharmonic), cycles removed
    /// assert_eq!(
    ///     EqualTemperament::find().by_edo(24).into_iter().next().unwrap().get_colors(),
    ///     &[
    ///         0, 0, 0, 0,    // Neutral layer (D, A, E, B)
    ///         1, 1, 1, 1, 1, // Enharmonic layer
    ///         0, 0, 0,       // Neutral layer (F, C, G)
    ///     ]
    /// );
    ///
    /// // Color layers of 7-EDO: 5 (n) + 2 (enharmonic)
    /// // Render parent MOS (2L3s) to avoid using only a single color
    /// assert_eq!(
    ///     EqualTemperament::find().by_edo(7).into_iter().next().unwrap().get_colors(),
    ///     &[
    ///         0, 0, 0, // Neutral layer (D, A, E)
    ///         1, 1,    // Enharmonic layer
    ///         0, 0,    // Render parent MOS (C, G)
    ///     ]
    /// );
    /// ```
    pub fn get_colors(&self) -> Vec<usize> {
        if self.mos.reduced_size() <= self.mos.num_steps() {
            if let Some(parent_mos) = self.mos.parent() {
                let parent_acc_format = AccidentalsFormat {
                    num_symbols: u16::try_from(parent_mos.num_steps()).unwrap(),
                    genchain_origin: (f64::from(self.acc_format.genchain_origin)
                        * f64::from(parent_mos.num_steps())
                        / f64::from(self.mos.num_steps()))
                    .round() as u16,
                };

                return parent_mos.get_colors(&parent_acc_format);
            }
        }

        self.mos.get_colors(&self.acc_format)
    }
}

/// Finds an appropriate [`EqualTemperament`] based on the list of [`PrototypeTemperament`]s provided.
pub struct TemperamentFinder {
    preference: Vec<Vec<PrototypeTemperament>>,
}

impl TemperamentFinder {
    pub fn with_preference(mut self, preference: Vec<Vec<PrototypeTemperament>>) -> Self {
        self.preference = preference;
        self
    }

    pub fn by_edo(&self, num_steps_per_octave: impl Into<f64>) -> Vec<EqualTemperament> {
        self.by_step_size(Ratio::octave().divided_into_equal_steps(num_steps_per_octave))
    }

    pub fn by_step_size(&self, step_size: Ratio) -> Vec<EqualTemperament> {
        let patent_val = Val::patent(step_size, 5);
        let mut b_val = patent_val.clone();
        b_val.pick_alternative(1);

        let mut temperaments = Vec::new();

        'prototypes: for prototypes in &self.preference {
            let mut prototype_found = false;

            // Accept the first matching prototype within a preference group.
            // If its sharpness is < 0, accept another one with sharpness >= 0.

            for prototype in prototypes {
                if let Some(temperament) = prototype.create_temperament(&patent_val, false) {
                    if temperament.mos.sharpness() >= 0 {
                        temperaments.push(temperament);
                        continue 'prototypes;
                    } else if !prototype_found {
                        temperaments.push(temperament);
                    }
                    prototype_found = true;
                }
            }

            // Accept a b-val as a last resort. As a trade-off, the sharpness should be > 0.

            if !prototype_found {
                for prototype in prototypes {
                    if let Some(temperament) = prototype.create_temperament(&b_val, true) {
                        if temperament.mos.sharpness() > 0 {
                            temperaments.push(temperament);
                            continue 'prototypes;
                        }
                    }
                }
            }
        }

        temperaments.sort_by(|t1, t2| {
            t1.alt_tritave()
                .cmp(&t2.alt_tritave())
                .then_with(|| {
                    t1.mos
                        .sharpness()
                        .signum()
                        .cmp(&t2.mos.sharpness().signum())
                        .reverse()
                })
                .then_with(|| t1.pergen.num_cycles().cmp(&t2.pergen.num_cycles()))
        });

        temperaments
    }
}

/// The temperament providing the generation schema and layout rules for a given scale as a prototype.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PrototypeTemperament {
    /// Octave-reduced temperament treating 4 fifths to be equal to one major third.
    ///
    /// The major third can be divided into two equal parts which form the *primary steps* of the scale.
    ///
    /// The note names are derived from the genchain of fifths (3/2) [ &hellip; Bb F C G D A E B F# &hellip; ].
    /// This results in standard music notation with G at one fifth above C and D at two fifths == 1/2 major third == 1 primary step above C.
    ///
    /// This prototype template also applies to other chain-of-fifth-based temperaments like Mavila and Superpyth.
    Meantone7,

    /// Similar to [`PrototypeTemperament::Meantone7`] but with 5 natural notes instead of 7.
    ///
    /// Preferred for rather sharp versions of 3/2 where using 7 natural would not result in a MOS scale.
    ///
    /// The genchain order is [ &hellip; Eb C G D A E C# &hellip; ].
    Meantone5,

    /// Similar to [`PrototypeTemperament::Meantone7`] but with 9 natural notes instead of 7.
    ///
    /// Preferred for rather flat versions of 3/2 where using 7 natural notes would result in a MOS scale with negative sharpness.
    ///
    /// The genchain order is [ &hellip; Fb, B, G, C, H, D, Z, E, A, F, B# ].
    ///
    /// Due to the added notes, the usual relationships between interval names and just ratios no longer apply.
    /// For instance, a Mavila[9] major third will sound similar to a Meantone[7] minor third and a Mavila[9] minor fourth will sound similar to a Meantone[7] major third.
    Mavila9,

    /// Octave-reduced temperament treating 3 major thirds to be equal to two major fourths.
    ///
    /// The temperament is best described in terms of *primary steps*, three of which form a major fourth.
    ///
    /// The note names are derived from the genchain of primary steps [ &hellip; Hb A B C D E F G H A# &hellip; ].
    ///
    /// Unlike in meantone, the intervals E-F and F-G have the same size of one primary step while G-A is different which has some important consequences.
    /// For instance, a Porcupine[8] major third will sound similar to a Meantone[7] minor third and a Porcupine[8] minor fourth will sound similar to a Meantone[7] major third.
    Porcupine8,

    /// Similar to [`PrototypeTemperament::Porcupine8`] but with 7 natural notes instead of 8.
    ///
    /// Preferred for rather sharp versions of 4/3 where using 8 natural notes would not result in a MOS scale.
    ///
    /// The genchain order is [ &hellip; Gb A B C D E F G A# &hellip; ].
    Porcupine7,
}

impl PrototypeTemperament {
    fn create_temperament(self, val: &Val, alt_tritave: bool) -> Option<EqualTemperament> {
        let pergen = self.get_pergen(val)?;
        let spec = self.get_spec();

        let mos = pergen.get_moses().find(|mos| {
            usize::from(mos.num_primary_steps()) + usize::from(mos.num_secondary_steps())
                == spec.genchain.len()
        })?;

        let (sharp_sign, flat_sign, order) = if mos.primary_step() >= mos.secondary_step() {
            ('#', 'b', AccidentalsOrder::SharpFlat)
        } else {
            ('-', '+', AccidentalsOrder::FlatSharp)
        };

        Some(EqualTemperament {
            prototype: self,
            alt_tritave,
            pergen,
            mos,
            acc_format: AccidentalsFormat {
                num_symbols: u16::try_from(mos.num_steps()).ok()?,
                genchain_origin: spec.genchain_origin,
            },
            formatter: NoteFormatter {
                note_names: spec.genchain.into(),
                sharp_sign,
                flat_sign,
                order,
            },
        })
    }

    fn get_pergen(&self, val: &Val) -> Option<PerGen> {
        let values = val.values();
        let octave = values[0];
        let tritave = values[1];

        Some(match self {
            PrototypeTemperament::Meantone7
            | PrototypeTemperament::Meantone5
            | PrototypeTemperament::Mavila9 => {
                let fifth = tritave.checked_sub(octave)?;
                PerGen::new(octave, fifth)
            }
            PrototypeTemperament::Porcupine7 | PrototypeTemperament::Porcupine8 => {
                let third_fourth = exact_div(octave.checked_mul(2)?.checked_sub(tritave)?, 3)?;
                PerGen::new(octave, third_fourth)
            }
        })
    }

    fn get_spec(self) -> TemperamentSpec {
        match self {
            PrototypeTemperament::Meantone7 => TemperamentSpec {
                genchain: &['F', 'C', 'G', 'D', 'A', 'E', 'B'],
                genchain_origin: 3,
            },
            PrototypeTemperament::Meantone5 => TemperamentSpec {
                genchain: &['C', 'G', 'D', 'A', 'E'],
                genchain_origin: 2,
            },
            PrototypeTemperament::Mavila9 => TemperamentSpec {
                genchain: &['B', 'G', 'C', 'H', 'D', 'Z', 'E', 'A', 'F'],
                genchain_origin: 4,
            },
            PrototypeTemperament::Porcupine8 => TemperamentSpec {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'],
                genchain_origin: 3,
            },
            PrototypeTemperament::Porcupine7 => TemperamentSpec {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G'],
                genchain_origin: 3,
            },
        }
    }
}

fn exact_div(numer: u16, denom: u16) -> Option<u16> {
    (numer % denom == 0).then_some(numer / denom)
}

impl Display for PrototypeTemperament {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_name = match self {
            PrototypeTemperament::Meantone7 => "Meantone[7]",
            PrototypeTemperament::Meantone5 => "Meantone[5]",
            PrototypeTemperament::Mavila9 => "Mavila[9]",
            PrototypeTemperament::Porcupine8 => "Porcupine[8]",
            PrototypeTemperament::Porcupine7 => "Porcupine[7]",
        };
        write!(f, "{display_name}")
    }
}

struct TemperamentSpec {
    genchain: &'static [char],
    genchain_origin: u16,
}

/// A straightforward data structure for retrieving scale degrees on an isomorphic keyboard.
#[derive(Debug, Clone)]
pub struct IsomorphicKeyboard {
    /// The primary step width of the isometric keyboard.
    pub primary_step: u16,

    /// The secondary step width of the isometric keyboard.
    pub secondary_step: u16,
}

impl IsomorphicKeyboard {
    /// Make the keyboard coprime s.t. all scale degrees are reachable.
    ///
    /// This addresses the scenario where not all key degrees can be reached when the step sizes are not coprime.
    /// For instance, when `primary_step == 4` and `secondary_step == 2`, degrees with odd numbers cannot be obtained.
    ///
    /// This function solves the issue by adjusting `secondary_step` to divide the step width along the sharp axis into smaller segments.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::IsomorphicKeyboard;
    /// let already_coprime = IsomorphicKeyboard {
    ///     primary_step: 3,
    ///     secondary_step: 2,
    /// }.coprime();
    ///
    /// // Already coprime => Do nothing
    /// assert_eq!(already_coprime.primary_step, 3);
    /// assert_eq!(already_coprime.secondary_step, 2);
    ///
    /// let positive_sharp_value = IsomorphicKeyboard {
    ///     primary_step: 4,
    ///     secondary_step: 2,
    /// }.coprime();
    ///
    /// // Sharp value is 4-2=2 before and 4-3=1 afterwards
    /// assert_eq!(positive_sharp_value.primary_step, 4);
    /// assert_eq!(positive_sharp_value.secondary_step, 3);
    ///
    /// let negative_sharp_value = IsomorphicKeyboard {
    ///     primary_step: 2,
    ///     secondary_step: 4,
    /// }.coprime();
    ///
    /// // Sharp value is 2-4=-2 before and 2-3=-1 afterwards
    /// assert_eq!(negative_sharp_value.primary_step, 2);
    /// assert_eq!(negative_sharp_value.secondary_step, 3);
    ///
    /// let zero_sharp_value = IsomorphicKeyboard {
    ///     primary_step: 2,
    ///     secondary_step: 2,
    /// }.coprime();
    ///
    /// // Special case: Sharp value is 2-2=0 before and 2-1=1 afterwards
    /// assert_eq!(zero_sharp_value.primary_step, 2);
    /// assert_eq!(zero_sharp_value.secondary_step, 1);
    ///
    /// let large_sharp_value = IsomorphicKeyboard {
    ///     primary_step: 6,
    ///     secondary_step: 2,
    /// }.coprime();
    ///
    /// // Special case: Sharp value is 6-2=4 before and 6-5=1 afterwards
    /// assert_eq!(large_sharp_value.primary_step, 6);
    /// assert_eq!(large_sharp_value.secondary_step, 5);
    /// ```
    pub fn coprime(mut self) -> IsomorphicKeyboard {
        // Special case: Set sharp value to 1 if it is currently 0
        if self.primary_step == self.secondary_step {
            self.secondary_step = self.primary_step - 1;
            return self;
        }

        loop {
            let gcd = math::gcd_u16(self.secondary_step, self.primary_step);

            if gcd == 1 {
                return self;
            }

            let current_sharp_value = self.primary_step.abs_diff(self.secondary_step);
            let wanted_sharp_value = current_sharp_value / gcd;
            let sharp_delta = current_sharp_value - wanted_sharp_value;

            if self.primary_step > self.secondary_step {
                self.secondary_step += sharp_delta;
            } else {
                self.secondary_step -= sharp_delta;
            }
        }
    }

    /// Get the scale degree of the key at location `(x, y)`.
    ///
    /// ```
    /// # use tune::layout::IsomorphicKeyboard;
    /// let keyboard = IsomorphicKeyboard {
    ///     primary_step: 5,
    ///     secondary_step: 3,
    /// };
    ///
    /// assert_eq!(keyboard.get_key(-2, -2), -16);
    /// assert_eq!(keyboard.get_key(-2, -1), -13);
    /// assert_eq!(keyboard.get_key(-2, 0), -10);
    /// assert_eq!(keyboard.get_key(-1, 0), -5);
    /// assert_eq!(keyboard.get_key(0, 0), 0);
    /// assert_eq!(keyboard.get_key(0, 1), 3);
    /// assert_eq!(keyboard.get_key(0, 2), 6);
    /// assert_eq!(keyboard.get_key(1, 2), 11);
    /// assert_eq!(keyboard.get_key(2, 2), 16);
    /// ```
    pub fn get_key(&self, num_primary_steps: i16, num_secondary_steps: i16) -> i32 {
        i32::from(num_primary_steps) * i32::from(self.primary_step)
            + i32::from(num_secondary_steps) * i32::from(self.secondary_step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn edo_notes_1_to_99() {
        let mut output = String::new();

        for num_steps_per_octave in 1u16..100 {
            print_notes(&mut output, num_steps_per_octave).unwrap();
        }

        std::fs::write("edo-notes-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-notes-1-to-99.txt"));
    }

    fn print_notes(output: &mut String, num_steps_per_octave: u16) -> Result<(), fmt::Error> {
        for temperament in EqualTemperament::find().by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &temperament)?;

            for index in 0..num_steps_per_octave {
                writeln!(output, "{} - {}", index, temperament.get_note_name(index))?;
            }
        }

        Ok(())
    }

    #[test]
    fn edo_keyboards_1_to_99() {
        let mut output = String::new();

        for num_steps_per_octave in 1..100 {
            print_keyboards(&mut output, num_steps_per_octave).unwrap();
        }

        std::fs::write("edo-keyboards-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-keyboards-1-to-99.txt"));
    }

    fn print_keyboards(output: &mut String, num_steps_per_octave: u16) -> Result<(), fmt::Error> {
        for temperament in EqualTemperament::find().by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &temperament)?;

            let keyboard = temperament.get_keyboard();

            for y in -5i16..=5 {
                for x in 0..10 {
                    write!(
                        output,
                        "{:>4}",
                        keyboard
                            .get_key(x, y)
                            .rem_euclid(i32::from(num_steps_per_octave)),
                    )?;
                }
                writeln!(output)?;
            }
        }

        Ok(())
    }

    #[test]
    fn edo_colors_1_to_99() {
        let mut output = String::new();

        for num_steps_per_octave in 1..100 {
            print_colors(&mut output, num_steps_per_octave).unwrap();
        }

        std::fs::write("edo-colors-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-colors-1-to-99.txt"));
    }

    fn print_colors(output: &mut String, num_steps_per_octave: u16) -> Result<(), fmt::Error> {
        for temperament in EqualTemperament::find().by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &temperament)?;

            let colors = temperament.get_colors();
            let keyboard = temperament.get_keyboard();

            for y in -5i16..=5 {
                for x in 0..10 {
                    write!(
                        output,
                        "{:>4}",
                        colors[usize::from(
                            temperament
                                .pergen()
                                .get_generation(math::i32_rem_u(
                                    keyboard.get_key(x, y),
                                    num_steps_per_octave
                                ))
                                .degree
                        )],
                    )?;
                }
                writeln!(output)?;
            }
        }

        Ok(())
    }

    fn print_edo_headline(
        output: &mut String,
        num_steps_per_octave: u16,
        temperament: &EqualTemperament,
    ) -> Result<(), fmt::Error> {
        writeln!(
            output,
            "---- {}{}-EDO ({}) ----",
            num_steps_per_octave,
            temperament.wart(),
            temperament.prototype()
        )?;

        writeln!(
            output,
            "primary_step={}, secondary_step={}, sharpness={}, num_cycles={}",
            temperament.mos().primary_step(),
            temperament.mos().secondary_step(),
            temperament.mos().sharpness(),
            temperament.pergen().num_cycles(),
        )
    }
}
