//! Find generalized notes and names for rank-2 temperaments.

use crate::math;
use std::{
    borrow::Cow,
    cmp::Ordering,
    fmt::Write,
    iter,
    ops::{Add, Sub},
};

#[derive(Clone, Debug)]
pub struct PerGen {
    period: u16,
    generator: u16,
    num_cycles: u16,
    generator_inverse: u16,
}

impl PerGen {
    pub fn new(period: u16, generator: u16) -> Self {
        let (num_cycles, _, generator_inverse) =
            extended_gcd(i32::from(period), i32::from(generator));

        let num_cycles = u16::try_from(num_cycles).unwrap();
        let generator_inverse = math::i32_rem_u(generator_inverse, period / num_cycles);

        Self {
            period,
            generator,
            num_cycles,
            generator_inverse,
        }
    }

    pub fn period(&self) -> u16 {
        self.period
    }

    pub fn generator(&self) -> u16 {
        self.generator
    }

    pub fn num_cycles(&self) -> u16 {
        self.num_cycles
    }

    pub fn reduced_period(&self) -> u16 {
        self.period / self.num_cycles
    }

    pub fn get_generation(&self, index: u16) -> Generation {
        let reduced_index = index / self.num_cycles;

        let degree = math::i32_rem_u(
            i32::from(self.generator_inverse) * i32::from(reduced_index),
            self.reduced_period(),
        );

        Generation {
            cycle: (self.num_cycles > 1).then_some(index % self.num_cycles),
            degree,
        }
    }

    pub fn get_accidentals(&self, format: &AccidentalsFormat, index: u16) -> Accidentals {
        let generation = self.get_generation(index);
        let num_steps = self.reduced_period();

        if num_steps >= format.num_symbols {
            let degree = i32::from(format.genchain_origin) + i32::from(generation.degree);
            let end_of_genchain = format.num_symbols - 1;

            let sharp_coord = math::i32_rem_u(degree, num_steps);
            let flat_coord = math::i32_rem_u(i32::from(end_of_genchain) - degree, num_steps);

            // genchain:    F-->C-->G-->D-->A-->E-->B->F#->C#->G#->D#->A#-->F
            // sharp_coord: 9  10  11   0   1   2   3   4   5   6   7   8   9
            // flat_coord:  3   2   1   0  11  10   9   8   7   6   5   4   3

            Accidentals {
                cycle: generation.cycle,
                sharp_index: sharp_coord % format.num_symbols,
                sharp_count: sharp_coord / format.num_symbols,
                flat_index: end_of_genchain - flat_coord % format.num_symbols,
                flat_count: flat_coord / format.num_symbols,
            }
        } else {
            let shift = i32::from(generation.degree > 0) * i32::from(num_steps);

            let mut sharp_degree = i32::from(format.genchain_origin) + i32::from(generation.degree);
            let mut flat_degree = sharp_degree - shift;

            // genchain:        F->C->G->D->A->E->B
            // sharp_degree:             0  1  2  3  4
            // flat_degree:  1  2  3  4  0

            if sharp_degree >= i32::from(format.num_symbols) {
                sharp_degree -= shift;
            }
            if flat_degree < 0 {
                flat_degree += shift;
            }

            // genchain:     F->C->G->D->A->E->B
            // sharp_degree:       4  0  1  2  3
            // flat_degree:  2  3  4  0  1

            Accidentals {
                cycle: generation.cycle,
                sharp_index: u16::try_from(sharp_degree).unwrap(),
                sharp_count: 0,
                flat_index: u16::try_from(flat_degree).unwrap(),
                flat_count: 0,
            }
        }
    }

    pub fn get_moses(&self) -> impl Iterator<Item = Mos> {
        Mos::<u16>::new_genesis(self.period, self.generator).children()
    }
}

#[allow(clippy::many_single_char_names)]
fn extended_gcd(a: i32, b: i32) -> (i32, i32, i32) {
    let mut gcd = (a, b);
    let mut a_inv = (1, 0);
    let mut b_inv = (0, 1);

    while gcd.1 != 0 {
        let q = gcd.0 / gcd.1;
        gcd = (gcd.1, gcd.0 - q * gcd.1);
        a_inv = (a_inv.1, a_inv.0 - q * a_inv.1);
        b_inv = (b_inv.1, b_inv.0 - q * b_inv.1);
    }

    (gcd.0, a_inv.0, b_inv.0)
}

#[derive(Copy, Clone, Debug)]
pub struct Generation {
    pub cycle: Option<u16>,
    pub degree: u16,
}

#[derive(Clone, Debug)]
pub struct AccidentalsFormat {
    pub num_symbols: u16,
    pub genchain_origin: u16,
}

#[derive(Clone, Debug)]
pub struct Accidentals {
    pub cycle: Option<u16>,
    pub sharp_index: u16,
    pub sharp_count: u16,
    pub flat_index: u16,
    pub flat_count: u16,
}

#[derive(Clone, Debug)]
pub struct NoteFormatter {
    pub note_names: Cow<'static, [char]>,
    pub sharp_sign: char,
    pub flat_sign: char,
    pub order: AccidentalsOrder,
}

impl NoteFormatter {
    pub fn format(&self, accidentals: &Accidentals) -> String {
        if accidentals.sharp_count == 0
            && accidentals.flat_count == 0
            && accidentals.sharp_index == accidentals.flat_index
        {
            return self.render_note_with_cycle(
                accidentals.cycle,
                accidentals.sharp_index,
                0,
                '\0',
            );
        }

        match accidentals.sharp_count.cmp(&accidentals.flat_count) {
            Ordering::Greater => self.render_note_with_cycle(
                accidentals.cycle,
                accidentals.flat_index,
                accidentals.flat_count,
                self.flat_sign,
            ),
            Ordering::Less => self.render_note_with_cycle(
                accidentals.cycle,
                accidentals.sharp_index,
                accidentals.sharp_count,
                self.sharp_sign,
            ),
            Ordering::Equal => self.render_enharmonic_note_with_cycle(
                accidentals.cycle,
                accidentals.sharp_index,
                accidentals.flat_index,
                accidentals.sharp_count,
            ),
        }
    }

    fn render_note_with_cycle(
        &self,
        cycle: Option<u16>,
        index: u16,
        num_accidentals: u16,
        accidental: char,
    ) -> String {
        let mut formatted = String::new();

        self.write_note(&mut formatted, index, num_accidentals, accidental);
        self.write_cycle(&mut formatted, cycle);

        formatted
    }

    fn render_enharmonic_note_with_cycle(
        &self,
        cycle: Option<u16>,
        sharp_index: u16,
        flat_index: u16,
        num_accidentals: u16,
    ) -> String {
        let mut formatted = String::new();

        if cycle.is_some() {
            formatted.push('(');
        }
        match self.order {
            AccidentalsOrder::SharpFlat => {
                self.write_note(
                    &mut formatted,
                    sharp_index,
                    num_accidentals,
                    self.sharp_sign,
                );
                formatted.push('/');
                self.write_note(&mut formatted, flat_index, num_accidentals, self.flat_sign);
            }
            AccidentalsOrder::FlatSharp => {
                self.write_note(&mut formatted, flat_index, num_accidentals, self.flat_sign);
                formatted.push('/');
                self.write_note(
                    &mut formatted,
                    sharp_index,
                    num_accidentals,
                    self.sharp_sign,
                );
            }
        }
        if cycle.is_some() {
            formatted.push(')');
        }
        self.write_cycle(&mut formatted, cycle);

        formatted
    }

    fn write_note(&self, target: &mut String, index: u16, num_accidentals: u16, accidental: char) {
        target.push(*self.note_names.get(usize::from(index)).unwrap_or(&'?'));
        for _ in 0..num_accidentals {
            target.push(accidental);
        }
    }

    fn write_cycle(&self, target: &mut String, cycle: Option<u16>) {
        if let Some(cycle) = cycle {
            write!(target, "[{cycle}]").unwrap();
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AccidentalsOrder {
    SharpFlat,
    FlatSharp,
}

impl AccidentalsOrder {
    pub fn from_sharpness(sharpness: i16) -> Self {
        if sharpness >= 0 {
            AccidentalsOrder::SharpFlat
        } else {
            AccidentalsOrder::FlatSharp
        }
    }
}

#[allow(private_bounds)]
pub trait MosParam: Num {
    type Sharpness: NumCast<Self>;
    type TotalSteps: NumCast<Self>;
}

impl MosParam for u16 {
    type Sharpness = i32;
    type TotalSteps = u32;
}

impl MosParam for f64 {
    type Sharpness = f64;
    type TotalSteps = f64;
}

#[derive(Clone, Copy, Debug)]
pub struct Mos<StepSize = u16, StepCount = u16> {
    num_primary_steps: StepCount,
    num_secondary_steps: StepCount,
    primary_step: StepSize,
    secondary_step: StepSize,
    size: u16,
}

impl<StepCount: MosParam> Mos<u16, StepCount> {
    pub fn new_genesis(period: u16, generator: u16) -> Self {
        let primary_step = generator % period;
        Self {
            num_primary_steps: StepCount::one(),
            num_secondary_steps: StepCount::one(),
            primary_step,
            secondary_step: period - generator,
            size: period,
        }
    }
}

impl<StepCount: MosParam> Mos<f64, StepCount> {
    pub fn new_genesis(generator: f64) -> Self {
        let primary_step = generator.rem_euclid(1.0);
        Self {
            num_primary_steps: StepCount::one(),
            num_secondary_steps: StepCount::one(),
            primary_step,
            secondary_step: 1.0 - primary_step,
            size: 1,
        }
    }
}

impl<StepSize: MosParam> Mos<StepSize, u16> {
    pub fn new_primary_step_heavy(num_primary_steps: u16, num_secondary_steps: u16) -> Self {
        Self {
            num_primary_steps,
            num_secondary_steps,
            primary_step: StepSize::one(),
            secondary_step: StepSize::default(),
            size: num_primary_steps,
        }
    }

    pub fn new_secondary_step_heavy(num_primary_steps: u16, num_secondary_steps: u16) -> Self {
        Self {
            num_primary_steps,
            num_secondary_steps,
            primary_step: StepSize::default(),
            secondary_step: StepSize::one(),
            size: num_secondary_steps,
        }
    }
}

impl<StepSize: MosParam, StepCount: MosParam> Mos<StepSize, StepCount> {
    pub fn child(mut self) -> Option<Self> {
        if self.primary_step == StepSize::default() || self.secondary_step == StepSize::default() {
            return None;
        }

        let num_steps = self
            .num_secondary_steps
            .checked_add(self.num_primary_steps)?;
        let sharpness = self.primary_step.abs_diff(self.secondary_step);

        match self.primary_step.partial_cmp(&self.secondary_step) {
            Some(Ordering::Greater) => {
                self.num_secondary_steps = num_steps;
                self.primary_step = sharpness;
            }
            Some(Ordering::Less) => {
                self.num_primary_steps = num_steps;
                self.secondary_step = sharpness;
            }
            Some(Ordering::Equal) | None => return None,
        }

        Some(self)
    }

    pub fn children(self) -> impl Iterator<Item = Self> {
        iter::successors(Some(self), |mos| mos.child())
    }

    pub fn parent(self) -> Option<Self> {
        Some(self.dual().child()?.dual())
    }

    pub fn parents(self) -> impl Iterator<Item = Self> {
        iter::successors(Some(self), |mos| mos.parent())
    }

    pub fn genesis(self) -> Self {
        self.parents().last().unwrap()
    }

    pub fn dual(self) -> Mos<StepCount, StepSize> {
        Mos {
            num_primary_steps: self.primary_step,
            num_secondary_steps: self.secondary_step,
            primary_step: self.num_primary_steps,
            secondary_step: self.num_secondary_steps,
            size: self.size,
        }
    }

    pub fn num_steps(self) -> StepCount::TotalSteps {
        StepCount::TotalSteps::from(self.num_primary_steps)
            + StepCount::TotalSteps::from(self.num_secondary_steps)
    }

    pub fn num_primary_steps(self) -> StepCount {
        self.num_primary_steps
    }

    pub fn num_secondary_steps(self) -> StepCount {
        self.num_secondary_steps
    }

    pub fn primary_step(self) -> StepSize {
        self.primary_step
    }

    pub fn secondary_step(self) -> StepSize {
        self.secondary_step
    }

    pub fn size(self) -> u16 {
        self.size
    }

    pub fn sharpness(self) -> StepSize::Sharpness {
        StepSize::Sharpness::from(self.primary_step)
            - StepSize::Sharpness::from(self.secondary_step)
    }

    pub fn is_convergent(&self) -> bool {
        let abs_diff = self.primary_step.abs_diff(self.secondary_step);
        abs_diff <= self.primary_step && abs_diff <= self.secondary_step
    }
}

impl<StepCount> Mos<u16, StepCount> {
    pub fn num_cycles(self) -> u16 {
        math::gcd_u16(self.primary_step, self.secondary_step)
    }

    pub fn reduced_size(self) -> u16 {
        self.size / self.num_cycles()
    }
}

impl Mos<u16, u16> {
    pub fn get_colors(self, acc_format: &AccidentalsFormat) -> Vec<usize> {
        dbg!(self.num_cycles());

        let num_natural_primary_layers = u16::from(self.primary_step > 0);
        let num_natural_secondary_layers = u16::from(self.secondary_step > 0);

        let num_non_natural_primary_layers =
            self.primary_step / self.num_cycles() - num_natural_primary_layers;
        let num_non_natural_secondary_layers =
            self.secondary_step / self.num_cycles() - num_natural_secondary_layers;

        let num_intermediate_primary_layers = num_non_natural_primary_layers / 2;
        let num_intermediate_secondary_layers = num_non_natural_secondary_layers / 2;

        let num_enharmonic_primary_layers = num_non_natural_primary_layers % 2;
        let num_enharmonic_secondary_layers = num_non_natural_secondary_layers % 2;

        let size_of_neutral_layer = num_natural_primary_layers * self.num_primary_steps
            + num_natural_secondary_layers * self.num_secondary_steps;

        let size_of_enharmonic_layer = num_enharmonic_primary_layers * self.num_primary_steps
            + num_enharmonic_secondary_layers * self.num_secondary_steps;

        let mut sizes_of_intermediate_layers = Vec::new();
        sizes_of_intermediate_layers.extend(repeat(
            num_intermediate_primary_layers.min(num_intermediate_secondary_layers),
            self.num_primary_steps() + self.num_secondary_steps(),
        ));
        sizes_of_intermediate_layers.extend(repeat(
            num_intermediate_primary_layers.saturating_sub(num_intermediate_secondary_layers),
            self.num_primary_steps(),
        ));
        sizes_of_intermediate_layers.extend(repeat(
            num_intermediate_secondary_layers.saturating_sub(num_intermediate_primary_layers),
            self.num_secondary_steps(),
        ));

        let mut colors = Vec::new();
        colors.extend(repeat(size_of_neutral_layer, 0));
        for (layer_index, &layer_size) in sizes_of_intermediate_layers.iter().enumerate() {
            colors.extend(repeat(layer_size, 2 * layer_index + 1));
        }
        colors.extend(repeat(
            size_of_enharmonic_layer,
            sizes_of_intermediate_layers.len() * 2 + 1,
        ));
        for (layer_index, &layer_size) in sizes_of_intermediate_layers.iter().enumerate().rev() {
            colors.extend(repeat(layer_size, 2 * layer_index + 2))
        }

        let offset = usize::from(acc_format.genchain_origin) % colors.len();
        colors.rotate_left(offset);

        colors
    }
}

fn repeat<T: Clone>(count: u16, item: T) -> impl Iterator<Item = T> {
    iter::repeat(item).take(usize::from(count))
}

trait NumBase: Copy + Default + PartialOrd + Add<Output = Self> + Sub<Output = Self> {}

impl<T: Copy + Default + PartialOrd + Add<Output = Self> + Sub<Output = Self>> NumBase for T {}

// This trait is visible in the docs.
trait Num: NumBase {
    fn one() -> Self;

    fn abs_diff(self, other: Self) -> Self;

    fn checked_add(self, other: Self) -> Option<Self>;
}

impl Num for u16 {
    fn one() -> Self {
        1
    }

    fn abs_diff(self, other: Self) -> Self {
        self.abs_diff(other)
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        self.checked_add(other)
    }
}

impl Num for f64 {
    fn one() -> Self {
        1.0
    }

    fn abs_diff(self, other: Self) -> Self {
        (self - other).abs()
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(self + other)
    }
}

// This trait is visible in the docs.
trait NumCast<T>: NumBase + From<T> {}

impl<T: NumBase + From<U>, U> NumCast<U> for T {}

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

        assert_eq!(hexatonic_names(2, 1, 2), "G, D/C");
        assert_eq!(heptatonic_names(2, 1, 2), "G, D/C");
        assert_eq!(hexatonic_names(2, 1, 3), "D, A/G");
        assert_eq!(heptatonic_names(2, 1, 3), "D, A/G");
        assert_eq!(hexatonic_names(2, 1, 4), "A, E/D");
        assert_eq!(heptatonic_names(2, 1, 4), "A, E/D");

        assert_eq!(hexatonic_names(3, 2, 2), "G, A/C, D/F");
        assert_eq!(heptatonic_names(3, 2, 2), "G, A/C, D/F");
        assert_eq!(hexatonic_names(3, 2, 3), "D, E/G, A/C");
        assert_eq!(heptatonic_names(3, 2, 3), "D, E/G, A/C");
        assert_eq!(hexatonic_names(3, 2, 4), "A, D, E/G");
        assert_eq!(heptatonic_names(3, 2, 4), "A, B/D, E/G");

        assert_eq!(hexatonic_names(4, 3, 2), "G, E/C, A/F, D");
        assert_eq!(heptatonic_names(4, 3, 2), "G, E/C, A/F, D");
        assert_eq!(hexatonic_names(4, 3, 3), "D, G, E/C, A/F");
        assert_eq!(heptatonic_names(4, 3, 3), "D, B/G, E/C, A/F");
        assert_eq!(hexatonic_names(4, 3, 4), "A, D, G, E/C");
        assert_eq!(heptatonic_names(4, 3, 4), "A, D, B/G, E/C");

        assert_eq!(hexatonic_names(5, 3, 2), "G, A, C, D, E/F");
        assert_eq!(heptatonic_names(5, 3, 2), "G, A, B/C, D, E/F");
        assert_eq!(hexatonic_names(5, 3, 3), "D, E/F, G, A, C");
        assert_eq!(heptatonic_names(5, 3, 3), "D, E/F, G, A, B/C");
        assert_eq!(hexatonic_names(5, 3, 4), "A, C, D, E/F, G");
        assert_eq!(heptatonic_names(5, 3, 4), "A, B/C, D, E/F, G");
    }

    #[test]
    fn heptatonic_12edo_notation() {
        // Degree 0 == C (common choice)
        assert_eq!(
            heptatonic_names(12, 7, 1),
            "C, C#/Db, D, D#/Eb, E, F, F#/Gb, G, G#/Ab, A, A#/Bb, B"
        );
        // Degree 0 == D
        assert_eq!(
            heptatonic_names(12, 7, 3),
            "D, D#/Eb, E, F, F#/Gb, G, G#/Ab, A, A#/Bb, B, C, C#/Db"
        );
    }

    #[test]
    fn octatonic_13edo_notation() {
        // Degree 0 == A (common choice, see https://en.xen.wiki/w/13edo)
        assert_eq!(
            octatonic_names(13, 8, 4),
            "A, Ab/B#, B, C, Cb/D#, D, Db/E#, E, F, Fb/G#, G, H, Hb/A#"
        );
        // Degree 0 == D
        assert_eq!(
            octatonic_names(13, 8, 3),
            "D, Db/E#, E, F, Fb/G#, G, H, Hb/A#, A, Ab/B#, B, C, Cb/D#"
        );
    }

    fn hexatonic_names(period: u16, generator: u16, genchain_origin: u16) -> String {
        note_name(
            period,
            generator,
            &['F', 'C', 'G', 'D', 'A', 'E'],
            genchain_origin,
            AccidentalsOrder::SharpFlat,
        )
    }

    fn heptatonic_names(period: u16, generator: u16, genchain_origin: u16) -> String {
        note_name(
            period,
            generator,
            &['F', 'C', 'G', 'D', 'A', 'E', 'B'],
            genchain_origin,
            AccidentalsOrder::SharpFlat,
        )
    }

    fn octatonic_names(period: u16, generator: u16, offset: u16) -> String {
        note_name(
            period,
            generator,
            &['E', 'B', 'G', 'D', 'A', 'F', 'C', 'H'],
            offset,
            AccidentalsOrder::FlatSharp,
        )
    }

    fn note_name(
        period: u16,
        generator: u16,
        note_names: &'static [char],
        genchain_origin: u16,
        order: AccidentalsOrder,
    ) -> String {
        let pergen = PerGen::new(period, generator);
        let acc_format = AccidentalsFormat {
            num_symbols: u16::try_from(note_names.len()).unwrap(),
            genchain_origin,
        };
        let formatter = NoteFormatter {
            note_names: note_names.into(),
            sharp_sign: '#',
            flat_sign: 'b',
            order,
        };

        (0..period)
            .map(|index| formatter.format(&pergen.get_accidentals(&acc_format, index)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
