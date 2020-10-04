use crate::math;
use crate::temperament::EqualTemperament;
use std::convert::TryInto;

/// A physical or logical key on a real or virtual instrument without any notion of a pitch.
///
/// It does *not* represent a musical key, like in "F&nbsp;minor", which is why this struct is called [`PianoKey`].
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PianoKey {
    midi_number: i32,
}

impl PianoKey {
    pub fn from_midi_number(midi_number: impl Into<i32>) -> Self {
        Self {
            midi_number: midi_number.into(),
        }
    }

    pub fn midi_number(self) -> i32 {
        self.midi_number
    }

    /// Counts the number of keys [left inclusive, right exclusive) between `self` and `other`.
    pub fn num_keys_before(self, other: PianoKey) -> i32 {
        other.midi_number - self.midi_number
    }

    pub fn plus_steps(self, num_steps: i32) -> PianoKey {
        PianoKey::from_midi_number(self.midi_number() + num_steps)
    }
}

#[derive(Debug, Clone)]
pub struct Keyboard {
    root_key: PianoKey,
    primary_step: i16,
    secondary_step: i16,
}

impl Keyboard {
    pub fn root_at(root_key: PianoKey) -> Self {
        Self {
            root_key,
            primary_step: 2,
            secondary_step: 1,
        }
    }

    pub fn with_steps(mut self, primary_step: i16, secondary_step: i16) -> Self {
        self.primary_step = primary_step;
        self.secondary_step = secondary_step;
        self
    }

    pub fn with_steps_of(self, temperament: &EqualTemperament) -> Self {
        self.with_steps(temperament.primary_step(), temperament.secondary_step())
    }

    pub fn coprime(mut self) -> Keyboard {
        if self.secondary_step == 0 {
            self.secondary_step = self.primary_step;
        }

        let mut gcd;
        while {
            gcd = self.gcd();
            gcd > 1
        } {
            self.secondary_step /= gcd;
        }

        self
    }

    fn gcd(&self) -> i16 {
        gcd_i16(self.secondary_step, self.primary_step)
    }

    pub fn primary_step(&self) -> i16 {
        self.primary_step
    }

    pub fn secondary_step(&self) -> i16 {
        self.secondary_step
    }

    pub fn get_key(&self, x: i16, y: i16) -> PianoKey {
        let num_steps = i32::from(self.primary_step) * i32::from(x)
            - i32::from(self.secondary_step) * i32::from(y);
        self.root_key.plus_steps(num_steps)
    }
}

fn gcd_i16(numer: i16, denom: i16) -> i16 {
    math::gcd_u16(
        numer.abs().try_into().unwrap(),
        denom.abs().try_into().unwrap(),
    )
    .try_into()
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;

    #[test]
    fn keyboard_layout() {
        let mut output = String::new();
        for num_steps_per_octave in 1..100 {
            print_keyboard(&mut output, num_steps_per_octave);
        }
        std::fs::write("edo-keyboards-1-to-99.txt", &output).unwrap();
        assert_eq!(output, include_str!("../edo-keyboards-1-to-99.txt"));
    }

    pub fn print_keyboard(string: &mut String, num_steps_per_octave: u16) {
        let temperament = EqualTemperament::find().by_edo(num_steps_per_octave);
        let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
            .with_steps_of(&temperament)
            .coprime();

        writeln!(string, "---- {}-EDO ----", num_steps_per_octave).unwrap();
        writeln!(
            string,
            "primary_step={}, secondary_step={}, num_cycles={}",
            temperament.primary_step(),
            temperament.secondary_step(),
            temperament.num_cycles(),
        )
        .unwrap();

        for y in (-5i16..5).rev() {
            for x in 0..10 {
                write!(
                    string,
                    "{:^4}",
                    keyboard
                        .get_key(x, y)
                        .midi_number()
                        .rem_euclid(i32::from(num_steps_per_octave)),
                )
                .unwrap();
            }
            writeln!(string).unwrap();
        }
    }
}
