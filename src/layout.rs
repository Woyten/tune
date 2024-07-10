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

/// Find note names and step sizes for a given division of the octave using different genchains.
#[derive(Clone, Debug)]
pub struct IsomorphicLayout {
    genchain: Genchain,
    b_val: bool,
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
            Genchain::Mavila9,
            Genchain::Meantone7,
            Genchain::Meantone5,
            Genchain::Porcupine8,
            Genchain::Tetracot7,
            Genchain::Hanson7,
        ]
        .into_iter()
        .flat_map(|genchain| {
            genchain.create_layout(&patent_val, false).or_else(|| {
                b_val
                    .as_ref()
                    .and_then(|b_val| genchain.create_layout(b_val, true))
            })
        })
        .collect()
    }

    pub fn genchain(&self) -> Genchain {
        self.genchain
    }

    pub fn b_val(&self) -> bool {
        self.b_val
    }

    pub fn wart(&self) -> &'static str {
        if self.b_val {
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
        match (self.genchain, self.mos.sharpness().cmp(&0)) {
            (_, Ordering::Equal) => "equalized",
            (Genchain::Mavila9, Ordering::Greater) => "armotonic",
            (Genchain::Mavila9, Ordering::Less) => "balzano",
            (Genchain::Meantone7, Ordering::Greater) => "diatonic",
            (Genchain::Meantone7, Ordering::Less) => "antidiatonic",
            (Genchain::Meantone5, Ordering::Greater) => "antipentic",
            (Genchain::Meantone5, Ordering::Less) => "pentic",
            (Genchain::Porcupine8, Ordering::Greater) => "pine",
            (Genchain::Porcupine8, Ordering::Less) => "antipine",
            (Genchain::Tetracot7, Ordering::Greater) => "archeotonic",
            (Genchain::Tetracot7, Ordering::Less) => "onyx",
            (Genchain::Hanson7, Ordering::Greater) => "smitonic",
            (Genchain::Hanson7, Ordering::Less) => "mosh",
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

    /// Generates an automatic color schema for the given layout.
    ///
    /// This function is a wrapper around [`Mos::get_layers`] with the following quality-of-life enhancements:
    ///
    /// - The returned color layer is typed as a [`Layer`].
    /// - The values are returned in stepwise instead of genchain order.
    /// - Multi-cyclic MOSes are considered.
    /// - The origin of the layout's genchain is considered.
    ///
    /// # Return Value
    ///
    /// The color schema is returned as a [`Vec`] of [`Layer`]s in stepwise order.
    /// It is the caller's responsibility to map the returned values to their target colors.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::layout::IsomorphicLayout;
    /// # use tune::layout::Layer;
    /// let (n, s, f, e) = (Layer::Natural, Layer::Sharp, Layer::Flat, Layer::Enharmonic);
    ///
    /// // Color layers of 17-EDO (s(0)/f(0) = sharp/flat = ±2 EDO steps)
    /// assert_eq!(
    ///     IsomorphicLayout::find_by_edo(17)[0].get_layers(),
    ///     &[
    ///         n, f(0), s(0), // D, Eb, D#
    ///         n,             // E
    ///         n, f(0), s(0), // F, Gb, F#
    ///         n, f(0), s(0), // G, Ab, G#
    ///         n, f(0), s(0), // A, Bb, A#
    ///         n,             // B
    ///         n, f(0), s(0), // C, Db, C#
    ///     ]
    /// );
    ///
    /// // Color layers of 24-EDO (s(0)/f(0) = half-sharp/flat = ±1 EDO step)
    /// assert_eq!(
    ///     IsomorphicLayout::find_by_edo(24)[0].get_layers(),
    ///     &[
    ///         n, s(0), e(1), f(0), // D, D^, D#/Eb, Ev
    ///         n, s(0),             // E, E^
    ///         n, s(0), e(1), f(0), // F, F^, F#/Gb, Gv
    ///         n, s(0), e(1), f(0), // G, G^, G#/Ab, Av
    ///         n, s(0), e(1), f(0), // A, A^, A#/Bb, Bv
    ///         n, s(0),             // B, B^
    ///         n, s(0), e(1), f(0), // C, C^, C#/Db, Dv
    ///     ]
    /// );
    /// ```
    pub fn get_layers(&self) -> Vec<Layer> {
        let mut layers = self.mos.get_layers();
        let num_layers =
            layers.last().map(|&index| index + 1).unwrap_or_default() * self.pergen.num_cycles();
        let has_enharmonic_layer = num_layers % 2 == 0;

        let offset = usize::from(self.acc_format.genchain_origin) % layers.len();
        layers.rotate_left(offset);

        (0..self.pergen.period())
            .map(|index| {
                let generation = self.pergen().get_generation(index);
                let layer = layers[usize::from(generation.degree)] * self.pergen.num_cycles()
                    + generation.cycle.unwrap_or_default();
                if layer == 0 {
                    Layer::Natural
                } else if layer < (num_layers + 1) / 2 {
                    Layer::Sharp(layer - 1)
                } else if layer == (num_layers + 1) / 2 && has_enharmonic_layer {
                    Layer::Enharmonic(layer - 1)
                } else {
                    Layer::Flat(num_layers - layer - 1)
                }
            })
            .collect()
    }
}

/// A descriptor for a consecutive genchain segment after decomposing a MOS into its color layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Layer containing the natural notes.
    Natural,
    /// Layer at the given level containing sharp notes.
    Sharp(u16),
    /// Layer at the given level containing flat notes.
    Flat(u16),
    /// Layer at the given level containing notes with ambiguous accidental.
    Enharmonic(u16),
}

/// Genchain used to derive note names, colors and step sizes for a given tuning.
///
/// The name is to be understood as a representative for an entire family of temperaments that share the same genchain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Genchain {
    /// Similar to [`Genchain::Meantone7`] but with 9 natural notes instead of 7.
    ///
    /// This genchain can be used when rather flat versions of 3/2 are involved and [`Genchain::Meantone7`] would result in a MOS with negative sharpness.
    ///
    /// The generated notes are [ &hellip; Fb, B, G, C, H, D, Z, E, A, F, B#, &hellip; ].
    /// Due to the additional notes, the conventional relationships between interval names and just ratios no longer apply.
    /// For instance, a Mavila\[9\] major third will sound similar to a Meantone\[7\] minor third and a Mavila\[9\] minor fourth will sound similar to a Meantone\[7\] major third.
    Mavila9,

    /// Octave-reduced genchain treating four fifths (3/2) to be equal to one major third.
    ///
    /// The major third can be divided into two equal parts which form the *primary steps* of the resulting MOS.
    ///
    /// The notes are generated via the genchain of fifths [ &hellip; Bb F C G D A E B F# &hellip; ].
    /// This results in standard music notation with G at one fifth above C and D at two fifths == 1/2 major third == 1 primary step above C.
    ///
    /// This genchain is compatible with other chain-of-fifth-based temperaments like Mavila and Superpyth.
    Meantone7,

    /// Similar to [`Genchain::Meantone7`] but with 5 natural notes instead of 7.
    ///
    /// This genchain can be used when rather sharp versions of 3/2 are involved and [`Genchain::Meantone7`] would not result in a MOS.
    ///
    /// The generated notes are [ &hellip; Eb C G D A E C# &hellip; ].
    Meantone5,

    /// Octave-reduced genchain treating three seconds to be equal to one major fourth (4/3).
    ///
    /// The second acts as a *primary step* and as the generator with generated notes being [ &hellip; Hb A B C D E F G H A# &hellip; ].
    ///
    /// Unlike in meantone, the intervals E-F and F-G have the same size of one primary step while G-A is different which has some important consequences.
    /// For instance, a Porcupine\[8\] major third will sound similar to a Meantone\[7\] minor third and a Porcupine\[8\] minor fourth will sound similar to a Meantone\[7\] major third.
    Porcupine8,

    /// Similar to [`Genchain::Porcupine8`] but with 7 natural notes instead of 8 and with four seconds treated as being equal to one major fifth (3/2).
    ///
    /// This genchain can be used when rather sharp versions of 4/3 are involved and [`Genchain::Porcupine8`] would not result in a MOS.
    ///
    /// The generated notes are [ &hellip; Gb A B C D E F G A# &hellip; ].
    Tetracot7,

    /// Octave-reduced genchain treating six minor thirds to be equal to one major twelfth (3/1).
    ///
    /// The third is split into a major and minor second, corresponding to the *primary step* and *secondary step* sizes.
    ///
    /// The sixth is used to generate the notes [ &hellip; Eb C A F D B G E C# &hellip; ].
    Hanson7,
}

impl Genchain {
    fn create_layout(self, val: &Val, b_val: bool) -> Option<IsomorphicLayout> {
        let pergen = self.get_pergen(val)?;
        let spec = self.get_parameters();

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
            genchain: self,
            b_val,
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
            Genchain::Mavila9 | Genchain::Meantone7 | Genchain::Meantone5 => {
                let fifth = tritave.checked_sub(octave)?;
                PerGen::new(octave, fifth)
            }
            Genchain::Porcupine8 => {
                let third_fourth = exact_div(octave.checked_mul(2)?.checked_sub(tritave)?, 3)?;
                PerGen::new(octave, third_fourth)
            }
            Genchain::Tetracot7 => {
                let quarter_fifth = exact_div(tritave.checked_sub(octave)?, 4)?;
                PerGen::new(octave, quarter_fifth)
            }
            Genchain::Hanson7 => {
                let sixth_twelfth = exact_div(tritave, 6)?;
                let bright_generator = octave.checked_sub(sixth_twelfth)?;
                PerGen::new(octave, bright_generator)
            }
        })
    }

    fn get_parameters(self) -> GenchainParameters {
        match self {
            Genchain::Mavila9 => GenchainParameters {
                genchain: &['B', 'G', 'C', 'H', 'D', 'Z', 'E', 'A', 'F'],
                genchain_origin: 4,
            },
            Genchain::Meantone7 => GenchainParameters {
                genchain: &['F', 'C', 'G', 'D', 'A', 'E', 'B'],
                genchain_origin: 3,
            },
            Genchain::Meantone5 => GenchainParameters {
                genchain: &['C', 'G', 'D', 'A', 'E'],
                genchain_origin: 2,
            },
            Genchain::Porcupine8 => GenchainParameters {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'],
                genchain_origin: 3,
            },
            Genchain::Tetracot7 => GenchainParameters {
                genchain: &['A', 'B', 'C', 'D', 'E', 'F', 'G'],
                genchain_origin: 3,
            },
            Genchain::Hanson7 => GenchainParameters {
                genchain: &['C', 'A', 'F', 'D', 'B', 'G', 'E'],
                genchain_origin: 3,
            },
        }
    }
}

fn exact_div(numer: u16, denom: u16) -> Option<u16> {
    (numer % denom == 0).then_some(numer / denom)
}

impl Display for Genchain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_name = match self {
            Genchain::Mavila9 => "Mavila[9]",
            Genchain::Meantone7 => "Meantone[7]",
            Genchain::Meantone5 => "Meantone[5]",
            Genchain::Porcupine8 => "Porcupine[8]",
            Genchain::Tetracot7 => "Tetracot[7]",
            Genchain::Hanson7 => "Hanson[7]",
        };
        write!(f, "{display_name}")
    }
}

struct GenchainParameters {
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

            let layers = layout.get_layers();
            let mos = layout.mos().coprime();

            for y in -5i16..=5 {
                for x in 0..10 {
                    write!(
                        output,
                        "{:>4}",
                        format_layer(
                            &layers[usize::from(math::i32_rem_u(
                                mos.get_key(x, y),
                                num_steps_per_octave
                            ))]
                        ),
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
            layout.genchain()
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

    fn format_layer(layer: &Layer) -> String {
        match layer {
            Layer::Natural => "nat".to_owned(),
            Layer::Sharp(index) => format!("sh{index}"),
            Layer::Flat(index) => format!("fl{index}"),
            Layer::Enharmonic(index) => format!("en{index}"),
        }
    }
}
