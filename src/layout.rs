//! Find generator chains and keyboard layouts.

use std::{
    cmp::Ordering,
    fmt::{self, Display},
};

use crate::{
    pergen::{Accidentals, AccidentalsFormat, AccidentalsOrder, Mos, NoteFormatter, PerGen},
    pitch::Ratio,
    temperament::Val,
};

/// Find note names and step sizes for a given division of the octave using different notation schemas.
#[derive(Clone, Debug)]
pub struct IsomorphicLayout {
    notation: NotationSchema,
    alt_tritave: bool,
    pergen: PerGen,
    mos: Mos,
    acc_format: AccidentalsFormat,
    formatter: NoteFormatter,
}

impl IsomorphicLayout {
    pub fn find_by_edo(num_steps_per_octave: impl Into<f64>) -> Vec<IsomorphicLayout> {
        Self::find_by_step_size(Ratio::octave().divided_into_equal_steps(num_steps_per_octave))
    }

    pub fn find_by_step_size(step_size: Ratio) -> Vec<IsomorphicLayout> {
        let patent_val = Val::patent(step_size, 5);

        let patent_val_errors: Vec<_> = patent_val
            .errors_in_steps()
            .map(|error| error.abs())
            .collect();
        let evaluate_b_val = patent_val_errors[1] > 1.0 / 3.0; // Ensures b_val error is at most twice as large as patent_val error

        let b_val = evaluate_b_val.then(|| {
            let mut b_val = patent_val.clone();
            b_val.pick_alternative(1);
            b_val
        });

        [
            // Sorted from highest to lowest sharpness within a group
            NotationSchema::Mavila9,
            NotationSchema::Meantone7,
            NotationSchema::Meantone5,
            NotationSchema::Porcupine8,
            NotationSchema::Tetracot7,
            NotationSchema::Hanson7,
        ]
        .into_iter()
        .flat_map(|notation| {
            notation.create_layout(&patent_val, false).or_else(|| {
                b_val
                    .as_ref()
                    .and_then(|b_val| notation.create_layout(b_val, true))
            })
        })
        .collect()
    }

    pub fn notation(&self) -> NotationSchema {
        self.notation
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

    pub fn get_scale_name(&self) -> &'static str {
        match (self.mos().sharpness().cmp(&0), self.notation()) {
            (Ordering::Equal, _) => "equalized",
            (Ordering::Greater, NotationSchema::Mavila9) => "armotonic",
            (Ordering::Less, NotationSchema::Mavila9) => "balzano",
            (Ordering::Greater, NotationSchema::Meantone7) => "diatonic",
            (Ordering::Less, NotationSchema::Meantone7) => "antidiatonic",
            (Ordering::Greater, NotationSchema::Meantone5) => "antipentic",
            (Ordering::Less, NotationSchema::Meantone5) => "pentic",
            (Ordering::Greater, NotationSchema::Porcupine8) => "pine",
            (Ordering::Less, NotationSchema::Porcupine8) => "antipine",
            (Ordering::Greater, NotationSchema::Tetracot7) => "archeotonic",
            (Ordering::Less, NotationSchema::Tetracot7) => "onyx",
            (Ordering::Greater, NotationSchema::Hanson7) => "smitonic",
            (Ordering::Less, NotationSchema::Hanson7) => "mosh",
        }
    }

    /// Obtains the note name for the given degree of the current layout.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::IsomorphicLayout;
    /// let positive_sharpness = IsomorphicLayout::find_by_edo(31).into_iter().next().unwrap();
    ///
    /// assert_eq!(positive_sharpness.get_note_name(0), "D");
    /// assert_eq!(positive_sharpness.get_note_name(1), "Ebb");
    /// assert_eq!(positive_sharpness.get_note_name(18), "A");
    /// assert_eq!(positive_sharpness.get_note_name(25), "B#");
    ///
    /// let negative_sharpness = IsomorphicLayout::find_by_edo(16).into_iter().skip(1).next().unwrap();
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

    /// Generate an automatic color schema for the given layout.
    ///
    /// The resulting color schema is arranged in layers, with the innermost layer representing the natural notes and the outermost layer representing the most enharmonic notes, if any.
    ///
    /// The intermediate layers as well as the enharmonic layer contain the notes between the natural ones and use the same shape as the primary and secondary sub-scale or the full natural scale.
    ///
    /// The total number of layers depends on the larger of the primary and secondary step sizes of the given layout.
    ///
    /// # Return Value
    ///
    /// The color schema is returned as a `Vec` of abstract indexes that the method caller can use to look up the final color.
    /// The color indexes are returned in genchain order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::IsomorphicLayout;
    /// // Color layers of 31-EDO: 7 (n) + 7 (#) + 7 (b) + 5 (##) + 5 (bb)
    /// assert_eq!(
    ///     IsomorphicLayout::find_by_edo(31).into_iter().next().unwrap().get_colors(),
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
    ///     IsomorphicLayout::find_by_edo(19).into_iter().next().unwrap().get_colors(),
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
    ///     IsomorphicLayout::find_by_edo(24).into_iter().next().unwrap().get_colors(),
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
    ///     IsomorphicLayout::find_by_edo(7).into_iter().next().unwrap().get_colors(),
    ///     &[
    ///         0, 0, 0, // Neutral layer (D, A, E)
    ///         1, 1,    // Enharmonic layer
    ///         0, 0,    // Render parent MOS (C, G)
    ///     ]
    /// );
    /// ```
    pub fn get_colors(&self) -> Vec<usize> {
        if u32::from(self.mos.reduced_size()) <= self.mos.num_steps() {
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

/// Schema used to derive note names, colors and step sizes for a given tuning.
///
/// The name is to be understood as a representative for an entire family of temperaments that share the same notation schema.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotationSchema {
    /// Similar to [`NotationSchema::Meantone7`] but with 9 natural notes instead of 7.
    ///
    /// The schema can be used when rather flat versions of 3/2 are involved and [`NotationSchema::Meantone7`] would result in a MOS scale with negative sharpness.
    ///
    /// The genchain order is [ &hellip; Fb, B, G, C, H, D, Z, E, A, F, B#, &hellip; ].
    ///
    /// Due to the additional notes, the conventional relationships between interval names and just ratios no longer apply.
    /// For instance, a Mavila\[9\] major third will sound similar to a Meantone\[7\] minor third and a Mavila\[9\] minor fourth will sound similar to a Meantone\[7\] major third.
    Mavila9,

    /// Octave-reduced notation schema treating four fifths (3/2) to be equal to one major third.
    ///
    /// The major third can be divided into two equal parts which form the *primary steps* of the scale.
    ///
    /// The note names are derived from the genchain of fifths [ &hellip; Bb F C G D A E B F# &hellip; ].
    /// This results in standard music notation with G at one fifth above C and D at two fifths == 1/2 major third == 1 primary step above C.
    ///
    /// This schema is compatible with non-meantone chain-of-fifth-based temperaments like Mavila and Superpyth.
    Meantone7,

    /// Similar to [`NotationSchema::Meantone7`] but with 5 natural notes instead of 7.
    ///
    /// The schema can be used when rather sharp versions of 3/2 are involved and [`NotationSchema::Meantone7`] would not result in a MOS scale.
    ///
    /// The genchain order is [ &hellip; Eb C G D A E C# &hellip; ].
    Meantone5,

    /// Octave-reduced notation schema treating three seconds to be equal to one major fourth (4/3).
    ///
    /// The second acts as a *primary step* and as the generator for the genchain [ &hellip; Hb A B C D E F G H A# &hellip; ].
    ///
    /// Unlike in meantone, the intervals E-F and F-G have the same size of one primary step while G-A is different which has some important consequences.
    /// For instance, a Porcupine\[8\] major third will sound similar to a Meantone\[7\] minor third and a Porcupine\[8\] minor fourth will sound similar to a Meantone\[7\] major third.
    Porcupine8,

    /// Similar to [`NotationSchema::Porcupine8`] but with 7 natural notes instead of 8 and with four seconds treated as being equal to one major fifth (3/2).
    ///
    /// The schema can be used when rather sharp versions of 4/3 are involved and [`NotationSchema::Porcupine8`] would not result in a MOS scale.
    ///
    /// The genchain order is [ &hellip; Gb A B C D E F G A# &hellip; ].
    Tetracot7,

    /// Octave-reduced notation schema treating six thirds to be equal to one major twelfth (3/1).
    ///
    /// The third is split into a major and minor second, corresponding to the *primary step* and *secondary step* sizes.
    ///
    /// The sixth is used as the generator for the genchain [ &hellip; Eb C A F D B G E C# &hellip; ].
    Hanson7,
}

impl NotationSchema {
    fn create_layout(self, val: &Val, alt_tritave: bool) -> Option<IsomorphicLayout> {
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

        Some(IsomorphicLayout {
            notation: self,
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
            NotationSchema::Mavila9 | NotationSchema::Meantone7 | NotationSchema::Meantone5 => {
                let fifth = tritave.checked_sub(octave)?;
                PerGen::new(octave, fifth)
            }
            NotationSchema::Porcupine8 => {
                let third_fourth = exact_div(octave.checked_mul(2)?.checked_sub(tritave)?, 3)?;
                PerGen::new(octave, third_fourth)
            }
            NotationSchema::Tetracot7 => {
                let quarter_fifth = exact_div(tritave.checked_sub(octave)?, 4)?;
                PerGen::new(octave, quarter_fifth)
            }
            NotationSchema::Hanson7 => {
                let sixth_twelfth = exact_div(tritave, 6)?;
                let bright_generator = octave.checked_sub(sixth_twelfth)?;
                PerGen::new(octave, bright_generator)
            }
        })
    }

    fn get_spec(self) -> NotationSpec {
        match self {
            NotationSchema::Mavila9 => NotationSpec {
                genchain: &['B', 'G', 'C', 'H', 'D', 'Z', 'E', 'A', 'F'],
                genchain_origin: 4,
            },
            NotationSchema::Meantone7 => NotationSpec {
                genchain: &['F', 'C', 'G', 'D', 'A', 'E', 'B'],
                genchain_origin: 3,
            },
            NotationSchema::Meantone5 => NotationSpec {
                genchain: &['C', 'G', 'D', 'A', 'E'],
                genchain_origin: 2,
            },
            NotationSchema::Porcupine8 => NotationSpec {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'],
                genchain_origin: 3,
            },
            NotationSchema::Tetracot7 => NotationSpec {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G'],
                genchain_origin: 3,
            },
            NotationSchema::Hanson7 => NotationSpec {
                genchain: &['C', 'A', 'F', 'D', 'B', 'G', 'E'],
                genchain_origin: 3,
            },
        }
    }
}

fn exact_div(numer: u16, denom: u16) -> Option<u16> {
    (numer % denom == 0).then_some(numer / denom)
}

impl Display for NotationSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_name = match self {
            NotationSchema::Mavila9 => "Mavila[9]",
            NotationSchema::Meantone7 => "Meantone[7]",
            NotationSchema::Meantone5 => "Meantone[5]",
            NotationSchema::Porcupine8 => "Porcupine[8]",
            NotationSchema::Tetracot7 => "Tetracot[7]",
            NotationSchema::Hanson7 => "Hanson[7]",
        };
        write!(f, "{display_name}")
    }
}

struct NotationSpec {
    genchain: &'static [char],
    genchain_origin: u16,
}

#[cfg(test)]
mod tests {
    use crate::math;

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
        for layout in IsomorphicLayout::find_by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &layout)?;

            for index in 0..num_steps_per_octave {
                writeln!(output, "{} - {}", index, layout.get_note_name(index))?;
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
        for layout in IsomorphicLayout::find_by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &layout)?;

            let mos = layout.mos().coprime();

            for y in -5i16..=5 {
                for x in 0..10 {
                    write!(
                        output,
                        "{:>4}",
                        mos.get_key(x, y)
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
        for layout in IsomorphicLayout::find_by_edo(num_steps_per_octave) {
            print_edo_headline(output, num_steps_per_octave, &layout)?;

            let colors = layout.get_colors();
            let mos = layout.mos().coprime();

            for y in -5i16..=5 {
                for x in 0..10 {
                    write!(
                        output,
                        "{:>4}",
                        colors[usize::from(
                            layout
                                .pergen()
                                .get_generation(math::i32_rem_u(
                                    mos.get_key(x, y),
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
        layout: &IsomorphicLayout,
    ) -> Result<(), fmt::Error> {
        writeln!(
            output,
            "---- {}{}-EDO ({}) ----",
            num_steps_per_octave,
            layout.wart(),
            layout.notation()
        )?;

        writeln!(
            output,
            "primary_step={}, secondary_step={}, sharpness={}, num_cycles={}",
            layout.mos().primary_step(),
            layout.mos().secondary_step(),
            layout.mos().sharpness(),
            layout.pergen().num_cycles(),
        )
    }
}
