use crate::math;
use crate::ratio::Ratio;
use std::convert::TryInto;

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

    pub fn size_of_fifth(&self) -> Ratio {
        Ratio::from_octaves(
            f64::from(self.num_steps_per_fifth) / f64::from(self.num_steps_per_octave),
        )
    }

    pub fn num_cycles(&self) -> u16 {
        self.num_cycles
    }

    pub fn num_steps_per_cycle(&self) -> u16 {
        self.num_steps_per_octave / self.num_cycles
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
}
