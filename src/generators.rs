use crate::math;
use std::{cmp::Ordering, convert::TryFrom, fmt::Write};

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

    pub fn get_generation(&self, index: u16) -> Generation {
        let reduced_index = index / self.num_cycles;
        let reduced_period = self.period / self.num_cycles;
        let reduced_generator = self.generator / self.num_cycles;

        let inverse_of_generator =
            extended_gcd(i32::from(reduced_generator), i32::from(reduced_period)).0;

        let degree = math::i32_rem_u(
            inverse_of_generator * i32::from(reduced_index),
            reduced_period,
        );

        Generation {
            cycle: (self.num_cycles > 1).then(|| index % self.num_cycles),
            degree,
        }
    }

    fn get_accidentals(&self, index: u16, offset: i16, num_symbols: u16) -> Accidentals {
        let generation = self.get_generation(index);
        let num_steps = self.num_steps_per_cycle();

        if num_steps >= num_symbols {
            let degree = i32::from(offset) + i32::from(generation.degree);

            let sharp_coord = math::i32_rem_u(degree, num_steps);
            let flat_coord = math::i32_rem_u(i32::from(num_symbols - 1) - degree, num_steps);

            Accidentals {
                cycle: generation.cycle,
                sharp_index: sharp_coord % num_symbols,
                sharp_count: sharp_coord / num_symbols,
                flat_index: num_symbols - 1 - flat_coord % num_symbols,
                flat_count: flat_coord / num_symbols,
            }
        } else {
            let num_steps = i32::from(generation.degree > 0) * i32::from(num_steps);

            let mut sharp_degree = i32::from(generation.degree);
            let mut flat_degree = sharp_degree - num_steps;

            let threshold = i32::from(num_symbols - 1) / 2;
            if sharp_degree > threshold {
                sharp_degree -= num_steps;
            }
            if flat_degree < -threshold {
                flat_degree += num_steps;
            }

            Accidentals {
                cycle: generation.cycle,
                sharp_index: math::i32_rem_u(sharp_degree + i32::from(offset), num_symbols),
                sharp_count: 0,
                flat_index: math::i32_rem_u(flat_degree + i32::from(offset), num_symbols),
                flat_count: 0,
            }
        }
    }
}

pub struct Generation {
    cycle: Option<u16>,
    degree: u16,
}

pub struct Accidentals {
    cycle: Option<u16>,
    sharp_index: u16,
    sharp_count: u16,
    flat_index: u16,
    flat_count: u16,
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
    pub fn get_name_by_step(&self, per_gen: &PerGen, index: u16) -> String {
        let num_symbols = u16::try_from(self.note_names.len()).expect("Too many note names");

        let accidentals = per_gen.get_accidentals(index, self.genchain_origin, num_symbols);

        if accidentals.sharp_count == 0
            && accidentals.flat_count == 0
            && accidentals.sharp_index == accidentals.flat_index
        {
            return self.format_note(accidentals.cycle, accidentals.sharp_index, 0, '\0');
        }

        match accidentals.sharp_count.cmp(&accidentals.flat_count) {
            Ordering::Greater => self.format_note(
                accidentals.cycle,
                accidentals.flat_index,
                accidentals.flat_count,
                self.prev_cycle_sign,
            ),
            Ordering::Less => self.format_note(
                accidentals.cycle,
                accidentals.sharp_index,
                accidentals.sharp_count,
                self.next_cycle_sign,
            ),
            Ordering::Equal => match self.sharpness.is_positive() {
                true => format!(
                    "{} / {}",
                    self.format_note(
                        accidentals.cycle,
                        accidentals.sharp_index,
                        accidentals.sharp_count,
                        self.next_cycle_sign,
                    ),
                    self.format_note(
                        accidentals.cycle,
                        accidentals.flat_index,
                        accidentals.flat_count,
                        self.prev_cycle_sign,
                    ),
                ),
                false => format!(
                    "{} / {}",
                    self.format_note(
                        accidentals.cycle,
                        accidentals.flat_index,
                        accidentals.flat_count,
                        self.prev_cycle_sign,
                    ),
                    self.format_note(
                        accidentals.cycle,
                        accidentals.sharp_index,
                        accidentals.sharp_count,
                        self.next_cycle_sign,
                    ),
                ),
            },
        }
    }

    fn format_note(
        &self,
        cycle: Option<u16>,
        index: u16,
        num_accidentals: u16,
        accidental: char,
    ) -> String {
        let mut formatted = String::new();
        write!(formatted, "{}", self.note_names[usize::from(index)]).unwrap();
        if let Some(cycle) = cycle {
            write!(formatted, "[{}]", cycle).unwrap();
        }
        for _ in 0..num_accidentals {
            write!(formatted, "{}", accidental).unwrap();
        }
        formatted
    }
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

    #[test]
    fn small_edo_notation_with_different_genchain_origins() {
        assert_eq!(hexatonic_names(1, 1, 2), "G");
        assert_eq!(heptatonic_names(1, 1, 2), "G");
        assert_eq!(hexatonic_names(1, 1, 3), "D");
        assert_eq!(heptatonic_names(1, 1, 3), "D");
        assert_eq!(hexatonic_names(1, 1, 4), "A");
        assert_eq!(heptatonic_names(1, 1, 4), "A");

        assert_eq!(hexatonic_names(2, 1, 2), "G, D / C");
        assert_eq!(heptatonic_names(2, 1, 2), "G, D / C");
        assert_eq!(hexatonic_names(2, 1, 3), "D, A / G");
        assert_eq!(heptatonic_names(2, 1, 3), "D, A / G");
        assert_eq!(hexatonic_names(2, 1, 4), "A, E / D");
        assert_eq!(heptatonic_names(2, 1, 4), "A, E / D");

        assert_eq!(hexatonic_names(3, 2, 2), "G, A / C, D / F");
        assert_eq!(heptatonic_names(3, 2, 2), "G, A / C, D / F");
        assert_eq!(hexatonic_names(3, 2, 3), "D, E / G, A / C");
        assert_eq!(heptatonic_names(3, 2, 3), "D, E / G, A / C");
        assert_eq!(hexatonic_names(3, 2, 4), "A, F / D, E / G");
        assert_eq!(heptatonic_names(3, 2, 4), "A, B / D, E / G");

        assert_eq!(hexatonic_names(4, 3, 2), "G, C, A / F, D");
        assert_eq!(heptatonic_names(4, 3, 2), "G, E / C, A / F, D / B");
        assert_eq!(hexatonic_names(4, 3, 3), "D, G, E / C, A");
        assert_eq!(heptatonic_names(4, 3, 3), "D, B / G, E / C, A / F");
        assert_eq!(hexatonic_names(4, 3, 4), "A, D, F / G, E");
        assert_eq!(heptatonic_names(4, 3, 4), "A, F / D, B / G, E / C");

        assert_eq!(hexatonic_names(5, 3, 2), "G, A, C, D, F");
        assert_eq!(heptatonic_names(5, 3, 2), "G, A / B, C, D, E / F");
        assert_eq!(hexatonic_names(5, 3, 3), "D, E, G, A, C");
        assert_eq!(heptatonic_names(5, 3, 3), "D, E / F, G, A, B / C");
        assert_eq!(hexatonic_names(5, 3, 4), "A, F, D, E, G");
        assert_eq!(heptatonic_names(5, 3, 4), "A, B / C, D, E, F / G");
    }

    #[test]
    fn heptatonic_12edo_notation() {
        // Degree 0 == C (common choice)
        assert_eq!(
            heptatonic_names(12, 7, 1),
            "C, C# / Db, D, D# / Eb, E, F, F# / Gb, G, G# / Ab, A, A# / Bb, B"
        );
        // Degree 0 == D
        assert_eq!(
            heptatonic_names(12, 7, 3),
            "D, D# / Eb, E, F, F# / Gb, G, G# / Ab, A, A# / Bb, B, C, C# / Db"
        );
    }

    #[test]
    fn octatonic_13edo_notation() {
        // Degree 0 == A (common choice, see https://en.xen.wiki/w/13edo)
        assert_eq!(
            octatonic_names(13, 8, 4),
            "A, Ab / B#, B, C, Cb / D#, D, Db / E#, E, F, Fb / G#, G, H, Hb / A#"
        );
        // Degree 0 == D
        assert_eq!(
            octatonic_names(13, 8, 3),
            "D, Db / E#, E, F, Fb / G#, G, H, Hb / A#, A, Ab / B#, B, C, Cb / D#"
        );
    }

    fn hexatonic_names(period: u16, generator: u16, genchain_origin: i16) -> String {
        note_name(
            period,
            generator,
            &["F", "C", "G", "D", "A", "E"],
            genchain_origin,
            1,
        )
    }

    fn heptatonic_names(period: u16, generator: u16, genchain_origin: i16) -> String {
        note_name(
            period,
            generator,
            &["F", "C", "G", "D", "A", "E", "B"],
            genchain_origin,
            1,
        )
    }

    fn octatonic_names(period: u16, generator: u16, offset: i16) -> String {
        note_name(
            period,
            generator,
            &["E", "B", "G", "D", "A", "F", "C", "H"],
            offset,
            -1,
        )
    }

    fn note_name(
        period: u16,
        generator: u16,
        note_names: &'static [&'static str],
        genchain_origin: i16,
        sharpness: i16,
    ) -> String {
        let pergen = PerGen::new(period, generator);
        let formatter = NoteFormatter {
            note_names,
            genchain_origin,
            next_cycle_sign: '#',
            prev_cycle_sign: 'b',
            sharpness,
        };
        let result = (0..period)
            .map(|index| formatter.get_name_by_step(&pergen, index))
            .collect::<Vec<_>>()
            .join(", ");
        result
    }
}
