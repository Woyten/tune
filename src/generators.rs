use crate::math;
use std::{cmp::Ordering, convert::TryInto, iter};

#[derive(Clone, Debug)]
pub struct PerGen {
    period: u16,
    generator: u16,
    num_cycles: u16,
}

impl PerGen {
    pub fn new(period: u16, generator: u16) -> Self {
        Self {
            period,
            generator,
            num_cycles: math::gcd_u16(period, generator),
        }
    }

    pub fn period(&self) -> u16 {
        self.period
    }

    pub fn num_cycles(&self) -> u16 {
        self.num_cycles
    }

    pub fn num_steps_per_cycle(&self) -> u16 {
        self.period / self.num_cycles
    }

    pub fn get_generation(&self, index: i32) -> u16 {
        let reduced_index = index.div_euclid(i32::from(self.num_cycles));
        let reduced_period = i32::from(self.period / self.num_cycles);
        let reduced_generator = i32::from(self.generator / self.num_cycles);
        let inverse_of_generator = extended_gcd(reduced_generator, reduced_period).0;
        (inverse_of_generator * reduced_index)
            .rem_euclid(reduced_period)
            .try_into()
            .unwrap()
    }

    pub fn get_cycle(&self, index: i32) -> u16 {
        index
            .rem_euclid(i32::from(self.num_cycles))
            .try_into()
            .unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct NoteFormatter {
    pub note_names: &'static [&'static str],
    pub genchain_origin: i16,
    pub next_cycle_sign: char,
    pub prev_cycle_sign: char,
    pub sharpness: i16,
}

impl NoteFormatter {
    pub fn get_name_by_step(&self, per_gen: &PerGen, index: i32) -> String {
        let num_notes = self
            .note_names
            .len()
            .try_into()
            .expect("Too many note names");

        let up_generation =
            i32::from(per_gen.get_generation(index)) + i32::from(self.genchain_origin);
        let down_generation = up_generation - i32::from(per_gen.num_steps_per_cycle());
        let cycle = per_gen.get_cycle(index);

        if up_generation == i32::from(self.genchain_origin) {
            return self.format_note(up_generation, cycle, 0, '\0');
        }

        let num_sharps: u16 = up_generation.div_euclid(num_notes).try_into().unwrap();
        let num_flats: u16 = (-down_generation.div_euclid(num_notes)).try_into().unwrap();

        match num_flats.cmp(&num_sharps) {
            Ordering::Less => {
                self.format_note(down_generation, cycle, num_flats, self.prev_cycle_sign)
            }
            Ordering::Greater => {
                self.format_note(up_generation, cycle, num_sharps, self.next_cycle_sign)
            }
            Ordering::Equal => match self.sharpness.is_positive() {
                true => format!(
                    "{} / {}",
                    self.format_note(up_generation, cycle, num_sharps, self.next_cycle_sign),
                    self.format_note(down_generation, cycle, num_flats, self.prev_cycle_sign),
                ),
                false => format!(
                    "{} / {}",
                    self.format_note(down_generation, cycle, num_flats, self.prev_cycle_sign),
                    self.format_note(up_generation, cycle, num_sharps, self.next_cycle_sign),
                ),
            },
        }
    }

    fn format_note(
        &self,
        generation: i32,
        cycle: u16,
        num_accidentals: u16,
        accidental: char,
    ) -> String {
        format!(
            "{}{}{}",
            repeated_char('▲', cycle),
            self.note_name(generation),
            repeated_char(accidental, num_accidentals),
        )
    }

    fn note_name(&self, generation: i32) -> &'static str {
        let index: usize = generation
            .rem_euclid(self.note_names.len().try_into().unwrap())
            .try_into()
            .unwrap();
        self.note_names[index]
    }
}

fn repeated_char(char_to_repeat: char, num_repetitions: u16) -> String {
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
