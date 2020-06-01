use crate::math;
use crate::ratio::Ratio;
use std::{cmp::Ordering, convert::TryInto, iter};

#[derive(Clone, Debug)]
pub struct Meantone {
    num_steps_per_octave: u16,
    num_steps_per_fifth: u16,
    num_cycles: u16,
    primary_step: i16,
    secondary_step: i16,
}

impl Meantone {
    pub fn new(num_steps_per_octave: u16, num_steps_per_fifth: u16) -> Self {
        Self {
            num_steps_per_octave,
            num_steps_per_fifth,
            num_cycles: math::gcd_u16(num_steps_per_octave, num_steps_per_fifth),
            primary_step: (2 * i32::from(num_steps_per_fifth) - i32::from(num_steps_per_octave))
                .try_into()
                .expect("large step out of range"),
            secondary_step: (3 * i32::from(num_steps_per_octave)
                - 5 * i32::from(num_steps_per_fifth))
            .try_into()
            .expect("small step out of range"),
        }
    }

    pub fn for_edo(num_steps_per_octave: u16) -> Self {
        Self::new(
            num_steps_per_octave,
            (Ratio::from_float(1.5).as_octaves() * f64::from(num_steps_per_octave)).round() as u16,
        )
    }

    pub fn num_steps_per_octave(&self) -> u16 {
        self.num_steps_per_octave
    }

    pub fn num_steps_per_fifth(&self) -> u16 {
        self.num_steps_per_fifth
    }

    pub fn num_steps_per_cycle(&self) -> u16 {
        self.num_steps_per_octave / self.num_cycles
    }

    pub fn size_of_fifth(&self) -> Ratio {
        Ratio::from_octaves(
            f64::from(self.num_steps_per_fifth) / f64::from(self.num_steps_per_octave),
        )
    }

    pub fn num_cycles(&self) -> u16 {
        self.num_cycles
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

    pub fn get_generation(&self, index: i32) -> u16 {
        let reduced_index = index.div_euclid(i32::from(self.num_cycles));
        let reduced_octave = i32::from(self.num_steps_per_octave / self.num_cycles);
        let reduced_fifth = i32::from(self.num_steps_per_fifth / self.num_cycles);
        let inverse_of_fifth = extended_gcd(reduced_fifth, reduced_octave).0;
        (inverse_of_fifth * reduced_index)
            .rem_euclid(reduced_octave)
            .try_into()
            .unwrap()
    }

    pub fn get_cycle(&self, index: i32) -> u16 {
        index
            .rem_euclid(i32::from(self.num_cycles()))
            .try_into()
            .unwrap()
    }

    pub fn get_pitch_class(&self, index: i32) -> PitchClass {
        let up_generation = self.get_generation(index);
        let down_generation =
            (self.num_steps_per_cycle() - up_generation).rem_euclid(self.num_steps_per_cycle());
        PitchClass {
            up_generation,
            down_generation,
            cycle: self.get_cycle(index),
        }
    }
}

pub struct PitchClass {
    up_generation: u16,
    down_generation: u16,
    cycle: u16,
}

impl PitchClass {
    pub fn format_heptatonic(&self) -> String {
        if self.up_generation == 0 && self.down_generation == 0 {
            return format_note(0, self.cycle, 0, '\0');
        }

        let num_sharps = (self.up_generation + 3) / 7;
        let num_flats = (self.down_generation + 3) / 7;

        match num_flats.cmp(&num_sharps) {
            Ordering::Less => format_note(7 - self.down_generation % 7, self.cycle, num_flats, 'b'),
            Ordering::Greater => format_note(self.up_generation, self.cycle, num_sharps, '#'),
            Ordering::Equal => format!(
                "{} / {}",
                format_note(self.up_generation, self.cycle, num_sharps, '#',),
                format_note(7 - self.down_generation % 7, self.cycle, num_flats, 'b',),
            ),
        }
    }
}

fn format_note(generation: u16, cycle: u16, num_accidentals: u16, accidental: char) -> String {
    format!(
        "{}{}{}",
        note_name(generation),
        repeated_char(cycle, '^'),
        repeated_char(num_accidentals, accidental),
    )
}

fn note_name(note_index: u16) -> &'static str {
    match note_index.rem_euclid(7) {
        0 => "D",
        1 => "A",
        2 => "E",
        3 => "B",
        4 => "F",
        5 => "C",
        6 => "G",
        _ => unreachable!(),
    }
}

fn repeated_char(num_repetitions: u16, char_to_repeat: char) -> String {
    iter::repeat(char_to_repeat)
        .take(num_repetitions.into())
        .collect()
}

#[allow(clippy::many_single_char_names)]
fn extended_gcd(a: i32, b: i32) -> (i32, i32) {
    let mut r = (a, b);
    let mut s = (1, 0);
    let mut t = (0, 1);

    while r.1 != 0 {
        let q = r.0 / r.1;
        r = (r.1, r.0 - q * r.1);
        s = (s.1, s.0 - q * s.1);
        t = (t.1, t.0 - q * t.1);
    }

    (s.0, t.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn note_names() {
        let mut output = String::new();
        for num_divisions in 1u16..100 {
            let meantone = Meantone::for_edo(num_divisions);
            writeln!(output, "---- {}-EDO ----", num_divisions).unwrap();
            writeln!(
                output,
                "primary_step={}, secondary_step={}, sharpness={}, num_cycles={}",
                meantone.primary_step(),
                meantone.secondary_step(),
                meantone.sharpness(),
                meantone.num_cycles(),
            )
            .unwrap();
            for index in 0..num_divisions {
                writeln!(
                    output,
                    "{} - {}",
                    index,
                    meantone
                        .get_pitch_class(i32::from(index))
                        .format_heptatonic()
                )
                .unwrap();
            }
        }
        assert_eq!(output, include_str!("../edo-notes-1-to-99.txt"));
    }
}
