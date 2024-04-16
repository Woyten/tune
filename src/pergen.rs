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

/// Representation of *Moment-Of-Symmetry* structures with shape *x*p*y*s.
///
/// Note that an *x*p*y*s (number of primary and secondary steps) representation is used instead of *x*L*y*s (number of large and small steps).
/// In addition, the primary and secondary step sizes are maintained as well, s.t. it is possible to determine the large and small step count.
///
/// A major advantage of the secondary/primary representation is that certain information is conserved.
/// For example, if we have a MOS *m* generated by a genesis MOS *g*, we can use the sign of `m.sharpness()` to determine whether `g.primary_step()` refers to the bright or to the dark generator of *m*.
#[derive(Clone, Copy, Debug)]
pub struct Mos<StepSize = u16, StepCount = u16> {
    num_primary_steps: StepCount,
    num_secondary_steps: StepCount,
    primary_step: StepSize,
    secondary_step: StepSize,
    size: u16,
}

impl<StepCount: MosParam> Mos<u16, StepCount> {
    /// Creates a new 1p1s [`Mos<u16>`] with a total size of `period` and a step ratio of `generator` &div; `period - generator`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// let edo_12_fifth = 7;
    /// let edo_12_fourth = 5;
    ///
    /// let mos = Mos::<u16>::new_genesis(12, edo_12_fifth);
    /// assert_eq!(mos.size(), 12);
    /// assert_eq!(mos.num_steps(), 2);
    /// assert_eq!(mos.num_primary_steps(), 1);
    /// assert_eq!(mos.num_secondary_steps(), 1);
    /// assert_eq!(mos.primary_step(), edo_12_fifth);
    /// assert_eq!(mos.secondary_step(), edo_12_fourth);
    ///
    /// let edo_12_tritave = 19;
    ///
    /// let mos = Mos::<u16>::new_genesis(12, edo_12_tritave);
    /// assert_eq!(mos.size(), 12);
    /// assert_eq!(mos.num_steps(), 2);
    /// assert_eq!(mos.num_primary_steps(), 1);
    /// assert_eq!(mos.num_secondary_steps(), 1);
    /// assert_eq!(mos.primary_step(), edo_12_fifth); // MOS is reduced to the period.
    /// assert_eq!(mos.secondary_step(), edo_12_fourth);
    /// ```
    pub fn new_genesis(period: u16, generator: u16) -> Self {
        let primary_step = generator % period;
        Self {
            num_primary_steps: StepCount::one(),
            num_secondary_steps: StepCount::one(),
            primary_step,
            secondary_step: period - primary_step,
            size: period,
        }
    }
}

impl<StepCount: MosParam> Mos<f64, StepCount> {
    /// Creates a new 1p1s [`Mos<f64>`] with a total size of 1 and a step ratio of `generator` &div; `1.0 - generator`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pergen::Mos;
    /// # use tune::pitch::Ratio;
    /// let just_fifth = Ratio::from_float(3.0 / 2.0).as_octaves();
    /// let just_fourth = Ratio::from_float(4.0 / 3.0).as_octaves();
    ///
    /// let mos = Mos::<f64>::new_genesis(just_fifth);
    /// assert_eq!(mos.size(), 1);
    /// assert_eq!(mos.num_steps(), 2);
    /// assert_eq!(mos.num_primary_steps(), 1);
    /// assert_eq!(mos.num_secondary_steps(), 1);
    /// assert_approx_eq!(mos.primary_step(), just_fifth);
    /// assert_approx_eq!(mos.secondary_step(), just_fourth);
    ///
    /// let just_tritave = Ratio::from_float(3.0).as_octaves();
    ///
    /// let mos = Mos::<f64>::new_genesis(just_tritave);
    /// assert_eq!(mos.size(), 1);
    /// assert_eq!(mos.num_steps(), 2);
    /// assert_eq!(mos.num_primary_steps(), 1);
    /// assert_eq!(mos.num_secondary_steps(), 1);
    /// assert_approx_eq!(mos.primary_step(), just_fifth); // MOS is reduced to the period.
    /// assert_approx_eq!(mos.secondary_step(), just_fourth);
    /// ```
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
    /// Creates a collapsed *x*L*y*s [`Mos`] with a step size ratio of 1 &div; 0 and a sharpness of 1.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// let num_diatonic_large_steps = 5;
    /// let num_diatonic_small_steps = 2;
    ///
    /// let mos = Mos::<u16>::new_collapsed(
    ///     num_diatonic_large_steps,
    ///     num_diatonic_small_steps
    /// );
    /// assert_eq!(mos.size(), num_diatonic_large_steps);
    /// assert_eq!(mos.num_steps(), 7);
    /// assert_eq!(mos.num_primary_steps(), num_diatonic_large_steps);
    /// assert_eq!(mos.num_secondary_steps(), num_diatonic_small_steps);
    /// assert_eq!(mos.primary_step(), 1);
    /// assert_eq!(mos.secondary_step(), 0);
    /// ```
    pub fn new_collapsed(num_large_steps: u16, num_small_steps: u16) -> Self {
        Self {
            num_primary_steps: num_large_steps,
            num_secondary_steps: num_small_steps,
            primary_step: StepSize::one(),
            secondary_step: StepSize::default(),
            size: num_large_steps,
        }
    }
}

impl<StepSize: MosParam, StepCount: MosParam> Mos<StepSize, StepCount> {
    /// Returns the current MOS' child MOS if possible.
    ///
    /// Returns [`None`] if the child MOS would be collapsed or if the step sizes would exceed numeric bounds.
    ///
    /// Note that, since the [`Mos`] type includes explicit step sizes, there is only one specific child MOS.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// let edo_12_fifth = 7;
    /// let mos = Mos::<u16>::new_genesis(12, edo_12_fifth);
    ///
    /// let child_mos = mos.child().unwrap();
    /// assert_eq!(child_mos.size(), 12);
    /// assert_eq!(child_mos.num_steps(), 3);
    /// assert_eq!(child_mos.num_primary_steps(), 1);
    /// assert_eq!(child_mos.num_secondary_steps(), 2);
    /// assert_eq!(child_mos.primary_step(), 2);
    /// assert_eq!(child_mos.secondary_step(), 5);
    ///
    /// let critical_mos = mos.children().last().unwrap();
    /// assert_eq!(critical_mos.primary_step(), 1);
    /// assert_eq!(critical_mos.secondary_step(), 1);
    ///
    /// // Child MOS would be collapsed since primary_step() == secondary_step().
    /// assert!(critical_mos.child().is_none());
    /// ```
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

    /// Retrieves a sequence of child MOSes i.e. the MOSes for a given generator.
    ///
    /// The sequence includes the current MOS and will stop once a MOS is no longer properly representable.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// # use tune::pitch::Ratio;
    /// let just_fifth = Ratio::from_float(3.0 / 2.0).as_octaves();
    /// let mos = Mos::<f64>::new_genesis(just_fifth);
    /// let children = mos.children().collect::<Vec<_>>();
    ///
    /// let genesis_mos = &children[0];
    /// assert_eq!(genesis_mos.size(), 1);
    /// assert_eq!(genesis_mos.num_steps(), 2);
    /// assert_eq!(genesis_mos.num_primary_steps(), 1);
    /// assert_eq!(genesis_mos.num_secondary_steps(), 1);
    ///
    /// let diatonic_mos = &children[3];
    /// assert_eq!(diatonic_mos.size(), 1);
    /// assert_eq!(diatonic_mos.num_steps(), 7);
    /// assert_eq!(diatonic_mos.num_primary_steps(), 5);
    /// assert_eq!(diatonic_mos.num_secondary_steps(), 2);
    ///
    /// let critical_mos = &children[42];
    /// assert_eq!(critical_mos.size(), 1);
    /// assert_eq!(critical_mos.num_steps(), 79335);
    /// assert_eq!(critical_mos.num_primary_steps(), 47468);
    /// assert_eq!(critical_mos.num_secondary_steps(), 31867);
    ///
    /// // Child MOS cannot be represented since num_steps() is not a valid u16.
    /// assert!(critical_mos.child().is_none());
    /// ```
    pub fn children(self) -> impl Iterator<Item = Self> {
        iter::successors(Some(self), |mos| mos.child())
    }

    /// The inverse operation of [`Mos::child`].
    pub fn parent(self) -> Option<Self> {
        Some(self.dual().child()?.dual())
    }

    /// The inverse operation of [`Mos::children`].
    pub fn parents(self) -> impl Iterator<Item = Self> {
        iter::successors(Some(self), |mos| mos.parent())
    }

    /// Calculates the generating parent MOS with shape 1p1s to obtain the generator bounds for a MOS with shape *x*p*y*s
    ///
    /// First, we need to calculate the generators of the collapsed *x*L*y*s MOS and the collapsed mirrored *y*L*x*s MOS.
    /// This is achieved by calling [`Mos::new_collapsed(x, y).genesis()`](Mos::new_collapsed) and [`Mos::new_collapsed(y, x).genesis()`](Mos::new_collapsed).
    ///
    /// Since [`Mos::new_collapsed`] yields a MOS with a positive sharpness of 1, the corresponding genesis MOSes will reveal the *bright* generator via [`Mos::primary_step()`] and the *dark* generator via [`Mos::secondary_step()`].
    ///
    /// The full generator ranges then become
    ///
    /// (a) `mirror_mos.secondary_step() \ mirror_mos.size() .. mos.primary_step() \ mos.size()`
    ///
    /// or
    ///
    /// (b) `mos.secondary_step() \ mos.size() .. mirror_mos.primary_step() \ mirror_mos.size()`.
    ///
    /// Both generator ranges seamlessly interpolate between the mirrored and the unmirrored MOS and are equally valid solutions.
    /// Note, however, that both ranges have a "bad" end that is affected by the presence of dark generators.
    /// Thus, if we want to focus on the unmirrored *x*L*y*s MOS, range (a) is preferable.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// // Find generator bounds of the diatonic (5L2s) scale.
    ///
    /// // Create a collapsed 5L2s MOS.
    /// let diatonic_mos = Mos::<u16>::new_collapsed(5, 2);
    ///
    /// let genesis_mos = diatonic_mos.genesis();
    /// assert_eq!(genesis_mos.num_primary_steps(), 1);
    /// assert_eq!(genesis_mos.num_secondary_steps(), 1);
    /// assert_eq!(genesis_mos.primary_step(), 3); // Bright generator.
    /// assert_eq!(genesis_mos.secondary_step(), 2); // Dark generator.
    /// assert_eq!(genesis_mos.size(), 5);
    ///
    /// // => The bright generator of 5L2s is 3\5. -> Upper bound!
    /// // => The dark generator of 5L2s is 2\5.
    ///
    /// // Create a collapsed 2L5s mirror MOS.
    /// let diatonic_mirror_mos = Mos::<u16>::new_collapsed(2, 5);
    ///
    /// let genesis_mirror_mos = diatonic_mirror_mos.genesis();
    /// assert_eq!(genesis_mirror_mos.num_primary_steps(), 1);
    /// assert_eq!(genesis_mirror_mos.num_secondary_steps(), 1);
    /// assert_eq!(genesis_mirror_mos.primary_step(), 1); // Bright generator.
    /// assert_eq!(genesis_mirror_mos.secondary_step(), 1); // Dark generator.
    /// assert_eq!(genesis_mirror_mos.size(), 2);
    ///
    /// // => The bright generator of 2L5s is 1\2.
    /// // => The dark generator of 2L5s is 1\2. -> Lower bound!
    ///
    /// // Result:
    /// // The total generator range is from 1\2 (2L5s, dark end) to 3\5 (5L2s, bright end).
    /// // The equal-step generator is (1+3)\(2+5) = 4\7.
    /// // The proper generator is (1+2*3)/(2+2*5) = 7\12.
    /// ```
    pub fn genesis(self) -> Self {
        self.parents().last().unwrap()
    }

    /// Creates a MOS with step sizes and step counts swapped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// let mos = Mos::<u16>::new_genesis(31, 18);
    ///
    /// let diatonic_mos = mos.children().skip(3).next().unwrap();
    /// assert_eq!(diatonic_mos.num_primary_steps(), 5);
    /// assert_eq!(diatonic_mos.num_secondary_steps(), 2);
    /// assert_eq!(diatonic_mos.primary_step(), 5);
    /// assert_eq!(diatonic_mos.secondary_step(), 3);
    ///
    /// let dual_mos = diatonic_mos.dual();
    /// assert_eq!(dual_mos.num_primary_steps(), 5);
    /// assert_eq!(dual_mos.num_secondary_steps(), 3);
    /// assert_eq!(dual_mos.primary_step(), 5);
    /// assert_eq!(dual_mos.secondary_step(), 2);
    /// ```
    pub fn dual(self) -> Mos<StepCount, StepSize> {
        Mos {
            num_primary_steps: self.primary_step,
            num_secondary_steps: self.secondary_step,
            primary_step: self.num_primary_steps,
            secondary_step: self.num_secondary_steps,
            size: self.size,
        }
    }

    /// Creates a MOS with primary and secondary semantics swapped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pergen::Mos;
    /// let bright_diatonic_mos = Mos::<u16>::new_genesis(12, 7).children().skip(3).next().unwrap();
    /// assert_eq!(bright_diatonic_mos.num_primary_steps(), 5);
    /// assert_eq!(bright_diatonic_mos.num_secondary_steps(), 2);
    /// assert_eq!(bright_diatonic_mos.primary_step(), 2);
    /// assert_eq!(bright_diatonic_mos.secondary_step(), 1);
    ///
    /// let dark_diatonic_mos = bright_diatonic_mos.mirror();
    /// assert_eq!(dark_diatonic_mos.num_primary_steps(), 2);
    /// assert_eq!(dark_diatonic_mos.num_secondary_steps(), 5);
    /// assert_eq!(dark_diatonic_mos.primary_step(), 1);
    /// assert_eq!(dark_diatonic_mos.secondary_step(), 2);
    /// ```
    pub fn mirror(self) -> Self {
        Self {
            num_primary_steps: self.num_secondary_steps,
            num_secondary_steps: self.num_primary_steps,
            primary_step: self.secondary_step,
            secondary_step: self.primary_step,
            size: self.size,
        }
    }

    /// Returns `num_primary_steps * primary_step + num_secondary_steps * secondary_step`.
    pub fn size(self) -> u16 {
        self.size
    }

    /// Returns `num_primary_steps + num_secondary_steps`.
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

    /// Returns `primary_step - secondary_step`.
    pub fn sharpness(self) -> StepSize::Sharpness {
        StepSize::Sharpness::from(self.primary_step)
            - StepSize::Sharpness::from(self.secondary_step)
    }
}

impl<StepCount> Mos<u16, StepCount> {
    /// Returns `gcd(primary_step, secondary_step)`.
    pub fn num_cycles(self) -> u16 {
        math::gcd_u16(self.primary_step, self.secondary_step)
    }

    /// Returns `size / gcd(primary_step, secondary_step)`.
    pub fn reduced_size(self) -> u16 {
        self.size / self.num_cycles()
    }
}

impl Mos<u16, u16> {
    /// Downstream function for [`EqualTemperament::get_colors`](`crate::layout::EqualTemperament::get_colors`).
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
