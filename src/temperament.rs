//! Prime-number based representation of just intervals.

use std::{borrow::Cow, collections::HashMap};

use crate::{math, pitch::Ratio};

/// A [`Val`] is a step size and a sequence of step numbers that, multiplied component-wise, are to be considered equivalent to the prime number sequence [2, 3, 5, 7, ...].
///
/// Treating a number of steps to be equivalent to a specific total ratio is the core idea of tempering.
/// That said, a val is an irreducible representation of the arithmetic properties of a temperament's generator.
#[derive(Clone, Debug)]
pub struct Val {
    step_size: Ratio,
    values: Vec<u16>,
}

impl Val {
    /// Creates a [`Val`] from the given values.
    ///
    /// [`None`] is returned if the provided list is too long.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let still_okay = vec![1; 54];
    /// assert!(Val::create(Ratio::from_semitones(1), still_okay).is_some());
    ///
    /// let too_long = vec![1; 55];
    /// assert!(Val::create(Ratio::from_semitones(1), too_long).is_none());
    /// ```
    pub fn create(step_size: Ratio, values: impl Into<Vec<u16>>) -> Option<Self> {
        let values = values.into();
        if values.len() > math::U8_PRIMES.len() {
            None
        } else {
            Some(Self { step_size, values })
        }
    }

    /// Calculates the patent [`Val`] for the given `step_size`.
    ///
    /// The patent val is the sequence of steps which, multiplied by `step_size`, provide the *best approximation* for the prime number ratios [2, 3, 5, 7, ..., `prime_limit`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_12_edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 13);
    /// assert_eq!(val_of_12_edo.values(), &[12, 19, 28, 34, 42, 44]);
    ///
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// assert_eq!(val_of_17_edo.values(), &[17, 27, 39, 48, 59]);
    ///
    /// let val_of_13_edt = Val::patent(Ratio::from_float(3.0).divided_into_equal_steps(13), 7);
    /// assert_eq!(val_of_13_edt.values(), &[8, 13, 19, 23]);
    /// ```
    pub fn patent(step_size: Ratio, prime_limit: u8) -> Self {
        Self {
            step_size,
            values: math::U8_PRIMES
                .iter()
                .filter(|&&prime_number| prime_number <= prime_limit)
                .map(|&prime_number| {
                    Ratio::from_float(prime_number)
                        .num_equal_steps_of_size(step_size)
                        .round() as u16
                })
                .collect(),
        }
    }

    /// Returns the step size stored in this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let some_step_size = Ratio::octave().divided_into_equal_steps(17);
    /// let val = Val::create(some_step_size, []).unwrap();
    /// assert_eq!(val.step_size(), some_step_size);
    /// ```
    pub fn step_size(&self) -> Ratio {
        self.step_size
    }

    /// Returns the values stored in this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let some_numbers = [5, 6, 7];
    /// let val = Val::create(Ratio::from_semitones(1), some_numbers).unwrap();
    /// assert_eq!(val.values(), some_numbers);
    /// ```
    pub fn values(&self) -> &[u16] {
        &self.values
    }

    /// Calculates the alternative step size for the given [`Val`] at the given `index`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let patent_val_of_18_edo = &[18, 29, 42, 51, 62, 67];
    /// let patent_val_of_18b_edo = &[18, 28, 42, 51, 62, 67];
    ///
    /// let mut val = Val::patent(Ratio::octave().divided_into_equal_steps(18), 13);
    /// assert_eq!(val.values(), patent_val_of_18_edo);
    ///
    /// let index_of_ratio_2 = 0;
    /// let index_of_ratio_3 = 1;
    ///
    /// // Octave is pure => Do not pick alternative
    /// let alternative_picked = val.pick_alternative(index_of_ratio_2);
    /// assert_eq!(alternative_picked, false);
    /// assert_eq!(val.values(), patent_val_of_18_edo);
    ///
    /// // Tritave is impure => Pick alternative
    /// let alternative_picked = val.pick_alternative(index_of_ratio_3);
    /// assert_eq!(alternative_picked, true);
    /// assert_eq!(val.values(), patent_val_of_18b_edo);
    ///
    /// // Tritave is impure => Pick original value again
    /// let alternative_picked = val.pick_alternative(index_of_ratio_3);
    /// assert_eq!(alternative_picked, true);
    /// assert_eq!(val.values(), patent_val_of_18_edo);
    /// ```
    pub fn pick_alternative(&mut self, index: u8) -> bool {
        let index = usize::from(index);
        if let (Some(value), Some(&prime_number)) =
            (self.values.get_mut(index), math::U8_PRIMES.get(index))
        {
            let deviation = self
                .step_size
                .repeated(*value)
                .deviation_from(Ratio::from_float(prime_number));

            if deviation.is_negligible() {
                return false;
            } else if deviation >= Ratio::default() {
                *value -= 1;
            } else {
                *value += 1;
            }

            true
        } else {
            false
        }
    }

    /// Returns the prime limit of this [`Val`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let custom_val = Val::create(Ratio::from_semitones(1), [12, 19, 28, 34, 42]).unwrap();
    /// assert_eq!(custom_val.prime_limit(), 11);
    /// ```
    pub fn prime_limit(&self) -> u8 {
        if self.values.is_empty() {
            1
        } else {
            math::U8_PRIMES[self.values.len() - 1]
        }
    }

    /// Returns the current [`Val`]s absolute errors i.e. the deviation from the prime number ratios.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let errors = Vec::from_iter(val_of_17_edo.errors().map(Ratio::as_cents));
    ///
    /// assert_eq!(errors.len(), 5);
    /// assert_approx_eq!(errors[0], 0.0);
    /// assert_approx_eq!(errors[1], 3.927352);
    /// assert_approx_eq!(errors[2], -33.372537);
    /// assert_approx_eq!(errors[3], 19.409388);
    /// assert_approx_eq!(errors[4], 13.387940);
    /// ```

    pub fn errors(&self) -> impl Iterator<Item = Ratio> + '_ {
        self.values
            .iter()
            .zip(math::U8_PRIMES)
            .map(move |(&value, &prime)| {
                self.step_size
                    .repeated(value)
                    .deviation_from(Ratio::from_float(prime))
            })
    }

    /// Returns the current [`Val`]'s errors where the unit of measurement is one `step_size`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let errors_in_steps = Vec::from_iter(val_of_17_edo.errors_in_steps());
    ///
    /// assert_eq!(errors_in_steps.len(), 5);
    /// assert_approx_eq!(errors_in_steps[0] * 100.0, 0.0);
    /// assert_approx_eq!(errors_in_steps[1] * 100.0, 5.563749);
    /// assert_approx_eq!(errors_in_steps[2] * 100.0, -47.277761);
    /// assert_approx_eq!(errors_in_steps[3] * 100.0, 27.496633);
    /// assert_approx_eq!(errors_in_steps[4] * 100.0, 18.966248);
    /// ```

    pub fn errors_in_steps(&self) -> impl Iterator<Item = f64> + '_ {
        self.errors()
            .map(move |error_abs| error_abs.num_equal_steps_of_size(self.step_size))
    }

    /// Calculates the Tenney-Euclidean simple badness.
    ///
    /// # Example
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::temperament::Val;
    /// # use tune::pitch::Ratio;
    /// let step_size_12_edo = Ratio::octave().divided_into_equal_steps(12);
    /// assert_approx_eq!(Val::patent(step_size_12_edo, 11).te_simple_badness() * 1000.0, 35.760225);
    ///
    /// let step_size_19_edo = Ratio::octave().divided_into_equal_steps(19);
    /// assert_approx_eq!(Val::patent(step_size_19_edo, 11).te_simple_badness() * 1000.0, 28.495822);
    /// ```
    pub fn te_simple_badness(&self) -> f64 {
        self.errors_in_steps()
            .zip(math::U8_PRIMES)
            .map(|(error_in_steps, &prime)| {
                let error_in_primes = error_in_steps / Ratio::from_float(prime).as_octaves();
                error_in_primes * error_in_primes
            })
            .sum::<f64>()
    }

    /// Returns the current [`Val`]s subgroup with the absolute errors below the given `threshold`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let val_of_17_edo = Val::patent(Ratio::octave().divided_into_equal_steps(17), 11);
    /// let subgroup = Vec::from_iter(val_of_17_edo.subgroup(Ratio::from_cents(25.0)));
    ///
    /// assert_eq!(subgroup, [2, 3, 7, 11]);
    /// ```
    pub fn subgroup(&self, threshold: Ratio) -> impl IntoIterator<Item = u8> + '_ {
        self.errors()
            .zip(math::U8_PRIMES)
            .filter(move |&(error, _)| error.as_cents().abs() < threshold.as_cents().abs())
            .map(|(_, &prime)| prime)
    }

    /// Applies the temperament's mapping function to the given [`Comma`].
    ///
    /// Specifically, it calculates the scalar product of the values of `self` and the values of the `comma` if the prime limit of `self` is at least the prime of `comma`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Comma;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let fifth = Comma::new("fifth", &[-1, 1][..]);
    /// assert_eq!(fifth.as_fraction(), Some((3, 2)));
    ///
    /// // The 12-edo fifth is at 7 steps
    /// let val_of_12edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 5);
    /// assert_eq!(val_of_12edo.map(&fifth), Some(7));
    ///
    /// // The 31-edo fifth is at 18 steps
    /// let val_of_31edo = Val::patent(Ratio::octave().divided_into_equal_steps(31), 5);
    /// assert_eq!(val_of_31edo.map(&fifth), Some(18));
    ///
    /// // 7-limit intervals cannot be represented by a 5-limit val
    /// let seventh = Comma::new("seventh", &[-2, 0, 0, 1][..]);
    /// assert_eq!(seventh.as_fraction(), Some((7, 4)));
    /// assert_eq!(val_of_12edo.map(&seventh), None);
    /// ```
    pub fn map(&self, comma: &Comma) -> Option<i32> {
        (self.prime_limit() >= comma.prime_limit()).then(|| {
            self.values
                .iter()
                .zip(comma.prime_factors())
                .map(|(&v, &c)| i32::from(v) * i32::from(c))
                .sum()
        })
    }

    /// Checks whether the current [`Val`] defines a rank-1 temperament which tempers out the given [`Comma`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Comma;
    /// # use tune::pitch::Ratio;
    /// # use tune::temperament::Val;
    /// let diesis = Comma::new("diesis", &[7, 0, -3][..]);
    /// assert_eq!(diesis.as_fraction(), Some((128, 125)));
    ///
    /// // 12-edo tempers out the diesis
    /// let val_of_12edo = Val::patent(Ratio::octave().divided_into_equal_steps(12), 5);
    /// assert!(val_of_12edo.tempers_out(&diesis));
    ///
    /// // 31-edo does not temper out the diesis
    /// let val_of_31edo = Val::patent(Ratio::octave().divided_into_equal_steps(31), 5);
    /// assert!(!val_of_31edo.tempers_out(&diesis));
    /// ```
    pub fn tempers_out(&self, comma: &Comma) -> bool {
        self.map(comma) == Some(0)
    }
}

/// A named rational interval in its prime factor representation.
#[derive(Clone, Debug)]
pub struct Comma {
    description: Cow<'static, str>,
    prime_factors: Cow<'static, [i8]>,
}

impl Comma {
    /// Creates a comma with the given `description` and prime factor decomposition.
    pub fn new(
        description: impl Into<Cow<'static, str>>,
        prime_factors: impl Into<Cow<'static, [i8]>>,
    ) -> Self {
        Self {
            description: description.into(),
            prime_factors: prime_factors.into(),
        }
    }

    /// Returns the name/description of the [`Comma`].
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the prime factor decomposition of the [`Comma`].
    pub fn prime_factors(&self) -> &[i8] {
        &self.prime_factors
    }

    /// Returns the prime limit of the [`Comma`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament::Comma;
    /// let syntonic_comma = Comma::new("syntonic comma", &[-4, 4, -1][..]);
    /// assert_eq!(syntonic_comma.prime_limit(), 5);
    ///
    /// let trivial_comma = Comma::new("unison", &[][..]);
    /// assert_eq!(trivial_comma.prime_limit(), 1);
    /// ```
    pub fn prime_limit(&self) -> u8 {
        if self.prime_factors.is_empty() {
            1
        } else {
            math::U8_PRIMES[self.prime_factors.len() - 1]
        }
    }

    /// Calculates the [`Ratio`] of the [`Comma`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::temperament::Comma;
    /// let pythagorean_comma = Comma::new("Pythagorean comma", &[-19, 12][..]);
    /// assert_approx_eq!(pythagorean_comma.as_ratio().as_cents(), 23.460010);
    ///
    /// let syntonic_comma = Comma::new("syntonic comma", &[-4, 4, -1][..]);
    /// assert_approx_eq!(syntonic_comma.as_ratio().as_cents(), 21.506290);
    /// ```
    pub fn as_ratio(&self) -> Ratio {
        Ratio::from_float(
            self.prime_factors
                .iter()
                .zip(math::U8_PRIMES)
                .map(|(&power, &prime)| f64::from(prime).powi(i32::from(power)))
                .product::<f64>(),
        )
    }

    /// Returns the numerator and denominator of the [`Comma`] if possible.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::temperament::Comma;
    /// let pythagorean_comma = Comma::new("Pythagorean comma", &[-19, 12][..]);
    /// assert_eq!(pythagorean_comma.as_fraction(), Some((531441, 524288)));
    ///
    /// let syntonic_comma = Comma::new("syntonic comma", &[-4, 4, -1][..]);
    /// assert_eq!(syntonic_comma.as_fraction(), Some((81, 80)));
    ///
    /// // 2^127 * 3^1 > u128::MAX
    /// let out_of_range = Comma::new("Very high numerator comma", &[127, 1][..]);
    /// assert_eq!(out_of_range.as_fraction(), None);
    /// ```
    pub fn as_fraction(&self) -> Option<(u128, u128)> {
        let mut numer: u128 = 1;
        let mut denom: u128 = 1;

        for (&power, &prime) in self.prime_factors.iter().zip(math::U8_PRIMES) {
            if power >= 0 {
                numer = numer
                    .checked_mul(u128::from(prime).checked_pow(u32::try_from(power).unwrap())?)?;
            } else {
                denom = denom
                    .checked_mul(u128::from(prime).checked_pow(u32::try_from(-power).unwrap())?)?;
            }
        }

        Some((numer, denom))
    }
}

/// Utility to access a large set of [`Comma`]s.
#[derive(Clone, Debug)]
pub struct CommaCatalog {
    commas_by_limit: HashMap<u8, Vec<Comma>>,
    comma_ref_by_name: HashMap<String, (u8, usize)>,
}

impl CommaCatalog {
    /// Creates a [`CommaCatalog`] from a given set of [`Comma`]s.
    pub fn new(commas: Vec<Comma>) -> Self {
        let mut commas_by_limit = HashMap::new();
        let mut comma_ref_by_name = HashMap::new();

        for comma in commas {
            let prime_limit = comma.prime_limit();
            let commas_for_limit = commas_by_limit.entry(prime_limit).or_insert_with(Vec::new);

            for name in comma.description().split(',') {
                comma_ref_by_name.insert(normalize(name), (prime_limit, commas_for_limit.len()));
            }

            commas_for_limit.push(comma);
        }

        Self {
            commas_by_limit,
            comma_ref_by_name,
        }
    }
}

impl CommaCatalog {
    /// Returns the [`Comma`]s for the given `prime_limit`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament;
    /// # use tune::temperament::CommaCatalog;
    /// let catalog = CommaCatalog::new(temperament::huygens_fokker_intervals());
    ///
    /// assert_eq!(catalog.commas_for_limit(2).len(), 1);
    /// assert_eq!(catalog.commas_for_limit(3).len(), 42);
    /// assert_eq!(catalog.commas_for_limit(5).len(), 127);
    /// assert_eq!(catalog.commas_for_limit(7).len(), 115);
    /// ```
    pub fn commas_for_limit(&self, prime_limit: u8) -> &[Comma] {
        self.commas_by_limit
            .get(&prime_limit)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns the [`Comma`] for the given `name`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::temperament;
    /// # use tune::temperament::CommaCatalog;
    /// let catalog = CommaCatalog::new(temperament::huygens_fokker_intervals());
    ///
    /// assert_eq!(
    ///     catalog.comma_for_name("pythagorean comma").unwrap().description(),
    ///     "Pythagorean comma, ditonic comma"
    /// );
    /// assert_eq!(
    ///     catalog.comma_for_name("DITONIC COMMA").unwrap().description(),
    ///     "Pythagorean comma, ditonic comma"
    /// );
    /// assert!(catalog.comma_for_name("serial comma").is_none());
    /// ```
    pub fn comma_for_name(&self, name: &str) -> Option<&Comma> {
        let &(prime_limit, index) = self.comma_ref_by_name.get(&normalize(name))?;
        self.commas_by_limit.get(&prime_limit)?.get(index)
    }
}

fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Returns all p-limit commas from <http://www.huygens-fokker.org/docs/intervals.html> where p <= 251.
pub fn huygens_fokker_intervals() -> Vec<Comma> {
    let commas: &[(&str, &[i8])] = &[
        ("unison, perfect prime", &[]),
        ("octave", &[1]),
        ("perfect fifth", &[-1, 1]),
        ("perfect fourth", &[2, -1]),
        ("major sixth, BP sixth", &[0, -1, 1]),
        ("major third", &[-2, 0, 1]),
        ("minor third", &[1, 1, -1]),
        ("minimal tenth, BP tenth", &[0, -1, 0, 1]),
        ("harmonic seventh", &[-2, 0, 0, 1]),
        ("septimal or Huygens' tritone, BP fourth", &[0, 0, -1, 1]),
        ("septimal minor third", &[-1, -1, 0, 1]),
        ("minor sixth", &[3, 0, -1]),
        ("septimal whole tone", &[3, 0, 0, -1]),
        ("major ninth", &[-2, 2]),
        ("just minor seventh, BP seventh", &[0, 2, -1]),
        ("septimal major third, BP third", &[0, 2, 0, -1]),
        ("major whole tone", &[-3, 2]),
        ("Euler's tritone", &[1, 0, 1, -1]),
        ("minor whole tone", &[1, -2, 1]),
        ("neutral ninth", &[0, 0, -1, 0, 1]),
        ("21/4-tone, undecimal neutral seventh", &[-1, -1, 0, 0, 1]),
        ("undecimal augmented fifth", &[0, 0, 0, -1, 1]),
        ("undecimal semi-augmented fourth", &[-3, 0, 0, 0, 1]),
        ("undecimal neutral third", &[0, -2, 0, 0, 1]),
        ("4/5-tone, Ptolemy's second", &[-1, 0, -1, 0, 1]),
        ("septimal major sixth", &[2, 1, 0, -1]),
        ("3/4-tone, undecimal neutral second", &[2, 1, 0, 0, -1]),
        ("16/3-tone", &[0, 0, 0, -1, 0, 1]),
        ("tridecimal neutral sixth", &[-3, 0, 0, 0, 0, 1]),
        ("tridecimal diminished fifth", &[0, -2, 0, 0, 0, 1]),
        ("tridecimal semi-diminished fourth", &[-1, 0, -1, 0, 0, 1]),
        ("tridecimal minor third", &[0, 0, 0, 0, -1, 1]),
        ("tridecimal 2/3-tone", &[-2, -1, 0, 0, 0, 1]),
        ("septimal minor sixth", &[1, -2, 0, 1]),
        (
            "undecimal diminished fourth or major third",
            &[1, 0, 0, 1, -1],
        ),
        ("2/3-tone", &[1, 0, 0, 1, 0, -1]),
        ("septimal minor ninth, BP ninth", &[0, 1, 1, -1]),
        ("classic major seventh", &[-3, 1, 1]),
        ("undecimal augmented fourth", &[0, 1, 1, 0, -1]),
        ("tridecimal 5/4-tone", &[0, 1, 1, 0, 0, -1]),
        ("major diatonic semitone", &[-1, 1, 1, -1]),
        ("septimal major ninth", &[4, 0, 0, -1]),
        ("Pythagorean minor seventh", &[4, -2]),
        ("undecimal semi-diminished fifth", &[4, 0, 0, 0, -1]),
        ("tridecimal neutral third", &[4, 0, 0, 0, 0, -1]),
        ("minor diatonic semitone", &[4, -1, -1]),
        ("septendecimal minor ninth", &[-3, 0, 0, 0, 0, 0, 1]),
        ("septendecimal major seventh", &[0, -2, 0, 0, 0, 0, 1]),
        ("septendecimal diminished seventh", &[-1, 0, -1, 0, 0, 0, 1]),
        ("2nd septendecimal tritone", &[-2, -1, 0, 0, 0, 0, 1]),
        ("supraminor third", &[-1, 0, 0, -1, 0, 0, 1]),
        ("septendecimal whole tone", &[0, -1, -1, 0, 0, 0, 1]),
        ("17th harmonic", &[-4, 0, 0, 0, 0, 0, 1]),
        ("undecimal neutral sixth", &[1, 2, 0, 0, -1]),
        ("tridecimal augmented fourth", &[1, 2, 0, 0, 0, -1]),
        ("Arabic lute index finger", &[1, 2, 0, 0, 0, 0, -1]),
        ("undevicesimal major seventh", &[-1, 0, -1, 0, 0, 0, 0, 1]),
        ("undevicesimal minor sixth", &[-2, -1, 0, 0, 0, 0, 0, 1]),
        ("undevicesimal ditone", &[0, -1, -1, 0, 0, 0, 0, 1]),
        ("19th harmonic", &[-4, 0, 0, 0, 0, 0, 0, 1]),
        ("quasi-meantone", &[0, 0, 0, 0, 0, 0, -1, 1]),
        ("undevicesimal semitone", &[-1, -2, 0, 0, 0, 0, 0, 1]),
        ("small ninth", &[2, -2, 1]),
        ("large minor seventh", &[2, 0, 1, 0, -1]),
        ("tridecimal semi-augmented fifth", &[2, 0, 1, 0, 0, -1]),
        ("septendecimal augmented second", &[2, 0, 1, 0, 0, 0, -1]),
        ("small undevicesimal semitone", &[2, 0, 1, 0, 0, 0, 0, -1]),
        ("undecimal major seventh", &[0, 1, 0, 1, -1]),
        ("narrow fourth", &[-4, 1, 0, 1]),
        ("submajor third", &[0, 1, 0, 1, 0, 0, -1]),
        ("minor semitone", &[-2, 1, -1, 1]),
        ("tridecimal major sixth", &[1, 0, 0, 0, 1, -1]),
        ("undecimal diminished fifth", &[1, -1, -1, 0, 1]),
        ("undecimal minor semitone", &[1, -1, 0, -1, 1]),
        (
            "vicesimotertial major seventh",
            &[-2, -1, 0, 0, 0, 0, 0, 0, 1],
        ),
        ("23rd harmonic", &[-4, 0, 0, 0, 0, 0, 0, 0, 1]),
        (
            "vicesimotertial major third",
            &[-1, -2, 0, 0, 0, 0, 0, 0, 1],
        ),
        ("tridecimal neutral seventh", &[3, 1, 0, 0, 0, -1]),
        ("1st septendecimal tritone", &[3, 1, 0, 0, 0, 0, -1]),
        (
            "smaller undevicesimal major third",
            &[3, 1, 0, 0, 0, 0, 0, -1],
        ),
        (
            "vicesimotertial minor semitone",
            &[3, 1, 0, 0, 0, 0, 0, 0, -1],
        ),
        ("classic augmented eleventh, BP twelfth", &[0, -2, 2]),
        ("classic augmented octave", &[-2, -1, 2]),
        ("middle minor seventh", &[-1, 0, 2, -1]),
        ("classic augmented fifth", &[-4, 0, 2]),
        ("classic augmented fourth", &[-1, -2, 2]),
        ("BP second, quasi-equal minor third", &[0, -1, 2, -1]),
        ("undecimal acute whole tone", &[-1, 0, 2, 0, -1]),
        ("classic chromatic semitone, minor chroma", &[-3, -1, 2]),
        ("tridecimal semi-augmented sixth", &[1, -1, -1, 0, 0, 1]),
        ("tridecimal 1/3-tone", &[1, 0, -2, 0, 0, 1]),
        ("septimal major seventh", &[-1, 3, 0, -1]),
        ("Pythagorean major sixth", &[-4, 3]),
        ("septendecimal minor sixth", &[0, 3, 0, 0, 0, 0, -1]),
        ("acute fourth", &[-2, 3, -1]),
        (
            "neutral third, Zalzal wosta of al-Farabi",
            &[-1, 3, 0, 0, -1],
        ),
        ("vicesimotertial minor third", &[0, 3, 0, 0, 0, 0, 0, 0, -1]),
        (
            "large limma, BP small semitone, Zarlino semitone",
            &[0, 3, -2],
        ),
        ("tridecimal comma", &[-1, 3, 0, 0, 0, -1]),
        ("grave major seventh", &[2, -1, -1, 1]),
        ("submajor sixth", &[2, 0, 0, 1, 0, 0, -1]),
        ("middle second", &[2, 0, -2, 1]),
        ("Archytas' 1/3-tone", &[2, -3, 0, 1]),
        ("29th harmonic", &[-4, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        ("septendecimal minor seventh", &[1, 1, 1, 0, 0, 0, -1]),
        (
            "smaller undevicesimal minor sixth",
            &[1, 1, 1, 0, 0, 0, 0, -1],
        ),
        ("31st harmonic", &[-4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        ("31st-partial chroma", &[-1, -1, -1, 0, 0, 0, 0, 0, 0, 0, 1]),
        ("minor ninth", &[5, -1, -1]),
        ("17th subharmonic", &[5, 0, 0, 0, 0, 0, -1]),
        ("19th subharmonic", &[5, 0, 0, 0, 0, 0, 0, -1]),
        ("wide fifth", &[5, -1, 0, -1]),
        ("23rd subharmonic", &[5, 0, 0, 0, 0, 0, 0, 0, -1]),
        ("classic diminished fourth", &[5, 0, -2]),
        ("Pythagorean minor third", &[5, -3]),
        ("29th subharmonic", &[5, 0, 0, 0, 0, 0, 0, 0, 0, -1]),
        (
            "Greek enharmonic 1/4-tone",
            &[5, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1],
        ),
        ("2 pentatones", &[0, 1, -2, 0, 1]),
        ("tridecimal major third", &[-1, 1, 0, 0, 1, -1]),
        ("undecimal minor third", &[-2, 1, 0, -1, 1]),
        ("undecimal comma, al-Farabi's 1/4-tone", &[-5, 1, 0, 0, 1]),
        ("quasi-mean seventh", &[1, 0, 0, 0, 0, 0, 1, -1]),
        ("supraminor sixth", &[1, -1, 0, -1, 0, 0, 1]),
        ("septendecimal major third", &[1, -3, 0, 0, 0, 0, 1]),
        ("septimal semi-diminished octave", &[-1, -2, 1, 1]),
        ("septimal semi-diminished fifth", &[-3, -1, 1, 1]),
        ("9/4-tone, septimal semi-diminished fourth", &[0, -3, 1, 1]),
        ("septimal neutral second", &[-5, 0, 1, 1]),
        ("septendecimal 1/4-tone", &[-1, 0, 1, 1, 0, 0, -1]),
        (
            "smaller undevicesimal major seventh",
            &[2, 2, 0, 0, 0, 0, 0, -1],
        ),
        ("classic diminished fifth", &[2, 2, -2]),
        ("septimal diesis, 1/4-tone", &[2, 2, -1, -1]),
        ("37th harmonic", &[-5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        (
            "39th harmonic, Zalzal wosta of Ibn Sina",
            &[-5, 1, 0, 0, 0, 1],
        ),
        ("acute major seventh", &[3, -1, 1, -1]),
        ("grave fifth", &[3, -3, 1]),
        ("tridecimal minor diesis", &[3, -1, 1, 0, 0, -1]),
        ("quasi-equal major sixth", &[1, 1, -2, 1]),
        ("undecimal grave minor seventh", &[2, 0, -2, 0, 1]),
        ("neutral sixth", &[2, -3, 0, 0, 1]),
        ("septimale wide minor sixth", &[-2, 2, 1, -1]),
        ("diatonic tritone", &[-5, 2, 1]),
        ("1/5-tone", &[-2, 2, 1, 0, -1]),
        ("23rd-partial chroma", &[1, -2, -1, 0, 0, 0, 0, 0, 1]),
        ("classic diminished octave", &[4, 1, -2]),
        ("septimal semi-augmented fourth", &[4, 1, -1, -1]),
        ("BP eighth", &[0, 0, -2, 2]),
        ("larger approximation to neutral sixth", &[-1, -1, -1, 2]),
        ("Arabic lute acute fourth", &[-2, -2, 0, 2]),
        ("larger approximation to neutral third", &[-3, 0, -1, 2]),
        ("BP minor semitone", &[0, -2, -1, 2]),
        ("undecimal minor whole tone", &[-2, 0, 0, 2, -1]),
        ("slendro diesis, septimal 1/6-tone", &[-4, -1, 0, 2]),
        ("grave major seventh", &[1, -3, 2]),
        ("3 pentatones", &[1, -1, 2, 0, -1]),
        ("Erlich's decatonic comma, tritonic diesis", &[1, 0, 2, -2]),
        ("septendecimal diminished fourth", &[-3, 1, -1, 0, 0, 0, 1]),
        ("17th-partial chroma", &[-1, 1, -2, 0, 0, 0, 1]),
        ("tridecimal minor sixth", &[2, -1, 0, 0, -1, 1]),
        ("septimal semi-augmented fifth", &[1, 3, -1, -1]),
        ("Zalzal's mujannab", &[1, 3, 0, -2]),
        ("undecimal semi-augmented fifth", &[-2, -2, 1, 0, 1]),
        ("undecimal semi-augmented whole tone", &[-4, -1, 1, 0, 1]),
        ("quasi-equal major second", &[0, 0, 1, -2, 1]),
        ("telepathma", &[-1, -3, 1, 0, 1]),
        ("septimal narrow major third", &[3, -2, -1, 1]),
        ("undecimal diesis, konbini comma", &[3, 0, -1, 1, -1]),
        ("undevicesimal minor seventh", &[-5, 1, 0, 0, 0, 0, 0, 1]),
        ("Hendrix comma", &[-3, 1, 0, -1, 0, 0, 0, 1]),
        ("smaller approximation to neutral third", &[2, 1, 1, -2]),
        ("quasi-equal major tenth, BP eleventh", &[0, 2, -2, 1]),
        ("octave - septimal comma", &[-5, 2, 0, 1]),
        ("submajor seventh", &[-1, 2, 0, 1, 0, 0, -1]),
        ("narrow minor sixth", &[-3, 2, -1, 1]),
        ("quasi-equal major third", &[-1, 2, -2, 1]),
        ("33rd subharmonic", &[6, -1, 0, 0, -1]),
        ("septimal neutral seventh", &[6, 0, -1, -1]),
        ("37th subharmonic", &[6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1]),
        ("39th subharmonic", &[6, -1, 0, 0, 0, -1]),
        ("2nd tritone", &[6, -2, -1]),
        ("2 septatones or septatonic major third", &[6, 0, 0, -2]),
        ("septimal comma, Archytas' comma", &[6, -2, 0, -1]),
        ("13th-partial chroma", &[-6, 0, 1, 0, 0, 1]),
        ("Winmeanma", &[1, 1, -1, 0, 1, -1]),
        ("23/4-tone", &[2, 0, -1, -1, 0, 0, 1]),
        ("supraminor second", &[2, -2, 0, -1, 0, 0, 1]),
        ("Valentine semitone", &[2, 0, -1, 0, 0, -1, 1]),
        ("Arabic lute grave fifth", &[3, 2, 0, -2]),
        ("undecimal semi-diminished fourth", &[3, 2, -1, 0, -1]),
        (
            "Ibn Sina's neutral third",
            &[3, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1],
        ),
        (
            "approximation to Pythagorean comma",
            &[
                1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, -1,
            ],
        ),
        ("BP fifth", &[0, 1, 2, -2]),
        ("marvelous fourth", &[-3, 1, 2, -1]),
        ("classic augmented second", &[-6, 1, 2]),
        ("Keemun minor third", &[-6, 0, 0, 1, 1]),
        ("undecimal secor", &[-3, -2, 0, 1, 1]),
        (
            "approximation to 53-tone comma",
            &[-2, 0, 0, 1, 1, 0, 0, -1],
        ),
        (
            "porcupine neutral second",
            &[1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1],
        ),
        ("tridecimal minor third comma", &[1, 1, 0, -1, -1, 1]),
        ("smaller approximation to neutral sixth", &[4, 0, 1, -2]),
        ("wide major third", &[4, -2, 1, -1]),
        ("2nd undecimal neutral seventh", &[-2, 4, 0, 0, -1]),
        ("acute minor sixth", &[-1, 4, -2]),
        ("Pythagorean major third", &[-6, 4]),
        ("Persian wosta", &[-2, 4, 0, 0, 0, 0, -1]),
        ("Al-Hwarizmi's lute middle finger ", &[-1, 4, -1, -1]),
        ("syntonic comma, Didymus comma", &[-4, 4, -1]),
        ("septendecimal minor third", &[-3, -2, 1, 0, 0, 0, 1]),
        ("undecimal minor seventh", &[3, 0, 0, -2, 1]),
        ("2nd undecimal neutral second", &[3, -4, 0, 0, 1]),
        (
            "quasi-equal semitone",
            &[
                -2, -1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        ),
        (
            "15/4-tone",
            &[0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1],
        ),
        ("medium tridecimal comma, superleap", &[-1, -2, -1, 1, 0, 1]),
        ("19th-partial chroma", &[5, 1, -1, 0, 0, 0, 0, -1]),
        ("quasi-equal minor seventh", &[1, 0, -1, 2, -1]),
        ("2nd quasi-equal tritone", &[-1, 2, -1, -1, 1]),
        ("small undecimal comma", &[-1, 2, 0, -2, 1]),
        ("quasi-equal minor sixth", &[2, -2, 2, -1]),
        ("grave major third", &[2, -4, 2]),
        ("Ptolemy's comma", &[2, -2, 2, 0, -1]),
        ("septimal neutral sixth", &[-6, 1, 1, 1]),
        ("small tridecimal comma", &[-3, 1, 1, 1, 0, -1]),
        ("marvelous fifth", &[4, -1, -2, 1]),
        ("tridecimal gentle fourth", &[-3, 2, 0, 0, -1, 1]),
        ("undecimal seconds comma, biyatisma", &[-3, -1, -1, 0, 2]),
        (
            "classic augmented seventh, octave - minor diesis",
            &[-6, 0, 3],
        ),
        ("classic augmented sixth", &[-3, -2, 3]),
        ("classic augmented third", &[-5, -1, 3]),
        ("semi-augmented whole tone", &[-2, -3, 3]),
        ("classic augmented semitone", &[-4, 0, 3, -1]),
        ("septimal semicomma, Starling comma", &[1, 2, -3, 1]),
        ("diminished seventh", &[7, -1, -2]),
        ("Pythagorean minor sixth", &[7, -4]),
        ("septimal neutral third", &[7, -1, -1, -1]),
        ("undecimal semitone", &[7, 0, 0, 0, -2]),
        ("minor diesis, diesis", &[7, 0, -3]),
        ("septimal wide minor third", &[-4, 3, 1, -1]),
        ("major chroma, major limma", &[-7, 3, 1]),
        ("quasi-equal tritone", &[2, -2, 1, 1, -1]),
        ("classic diminished third", &[4, 2, -3]),
        ("Grossma", &[4, 2, 0, 0, -1, -1]),
        ("29th-partial chroma", &[-4, -2, 1, 0, 0, 0, 0, 0, 0, 1]),
        ("7/4-tone", &[0, 2, -3, 0, 0, 0, 1]),
        ("Ganassi's comma", &[-3, 2, 0, 0, 0, 0, 1, -1]),
        ("octave - syntonic comma", &[5, -4, 1]),
        ("19/4-tone", &[0, -1, 0, 1, 0, 0, 0, 0, 1, 0, -1]),
        (
            "Persian neutral second",
            &[
                1, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, -1,
            ],
        ),
        (
            "quasi-equal major seventh",
            &[
                3, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1,
            ],
        ),
        ("Schulter's comma", &[-3, -1, 0, -1, 0, 2]),
        ("valinorsma", &[4, 0, -2, -1, 1]),
        ("classic diminished sixth", &[6, 1, -3]),
        ("septimal 4/5-tone", &[6, 1, -2, -1]),
        ("mynucuma", &[2, -1, -1, 2, 0, -1]),
        ("spleen comma", &[1, 1, 1, 1, -1, 0, 0, -1]),
        ("semi-augmented sixth", &[3, 3, -3]),
        ("narrow septimal major sixth", &[5, -3, -1, 1]),
        ("augmented sixth", &[-7, 2, 2]),
        ("septimal kleisma", &[-5, 2, 2, -1]),
        ("5/4-tone", &[-3, 1, -2, 1, 1]),
        (
            "Meshaqah's 3/4-tone",
            &[
                0, 0, 0, 0, 0, -1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        ),
        ("octave - maximal diesis", &[0, 5, -3]),
        ("Pythagorean major seventh", &[-7, 5]),
        ("acute fifth", &[-5, 5, -1]),
        ("acute minor third", &[-3, 5, -2]),
        ("Archytas' 2/3-tone", &[-5, 5, 0, -1]),
        ("neutral third comma, rastma", &[-1, 5, 0, 0, -2]),
        ("Nautilus comma", &[-1, 0, 1, 2, -2]),
        ("minor BP diesis, Sensamagic comma", &[0, -5, 1, 2]),
        (
            "Meshaqah's 1/4-tone",
            &[
                1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1,
            ],
        ),
        ("tricesoprimal comma", &[3, -5, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        ("17/4-tone", &[1, -2, 3, 0, 0, 0, -1]),
        ("maximal diesis, Porcupine comma", &[1, -5, 3]),
        ("octave - major chroma", &[8, -3, -1]),
        ("diminished third", &[8, -2, -2]),
        ("limma, Pythagorean minor second", &[8, -5]),
        ("septimal minor semitone", &[8, 0, -1, -2]),
        ("septendecimal kleisma", &[8, -1, -1, 0, 0, 0, -1]),
        ("vicesimononal comma", &[-8, 2, 0, 0, 0, 0, 0, 0, 0, 1]),
        ("Kirnberger's sixth", &[1, 3, 1, -1, 0, 0, 0, 0, -1]),
        ("Persian whole tone", &[4, -5, 0, 0, 0, 0, 1]),
        ("Ibn Sina's minor second", &[-8, 1, 0, 1, 0, 1]),
        ("Tannisma", &[-4, 1, 0, 1, 0, 1, -1]),
        ("Garibert comma", &[0, -1, 2, -1, 1, -1]),
        ("septendecimal minor second comma", &[-5, -2, 0, 0, 0, 0, 2]),
        ("grave fourth", &[6, -5, 1]),
        ("marveltwin", &[-2, -4, 2, 0, 0, 1]),
        ("ratwolf comma", &[-1, 3, -2, -1, 0, 1]),
        ("supracomma", &[5, 0, 0, -3, 1]),
        ("minthma", &[5, -3, 0, 0, 1, -1]),
        ("Dudon comma", &[-3, -2, -1, 0, 0, 0, 0, 2]),
        ("gentle comma", &[2, -1, 0, 1, -2, 1]),
        ("double augmented fourth", &[-8, 1, 3]),
        ("BP major semitone, minor BP chroma", &[0, 1, 3, -3]),
        ("undecimal kleisma, Keemun comma", &[-7, -1, 1, 1, 1]),
        ("grave major sixth", &[4, -5, 2]),
        ("wide augmented fifth", &[-8, 4, 1]),
        ("greenwoodma", &[-3, 4, 1, -2]),
        (
            "Werckmeister's undecimal septenarian schisma",
            &[-3, 2, -1, 2, -1],
        ),
        ("3 septatones or septatonic fifth", &[9, 0, 0, -3]),
        ("double diminished fifth", &[9, -1, -3]),
        ("narrow diminished fourth", &[9, -4, -1]),
        ("tridecimal neutral third comma", &[9, -1, 0, 0, 0, -2]),
        (
            "undevicesimal comma, Boethius' comma",
            &[-9, 3, 0, 0, 0, 0, 0, 1],
        ),
        ("Avicenna enharmonic diesis", &[-9, 1, 2, 1]),
        ("Swets' comma", &[2, 3, 1, -2, -1]),
        ("octave - major diesis", &[-2, -4, 4]),
        ("classic neutral third", &[-9, 0, 4]),
        ("BP great semitone, major BP chroma", &[0, -4, 4, -1]),
        ("huntma", &[7, 0, 1, -2, 0, -1]),
        ("major diesis", &[3, 4, -4]),
        ("wide augmented third", &[-9, 3, 2]),
        ("island comma", &[2, -3, -2, 0, 0, 2]),
        ("senga", &[1, -3, -2, 3]),
        (
            "11/4-tone",
            &[
                -2, 1, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            ],
        ),
        ("septendecimal bridge comma", &[-1, -1, 1, -1, 1, 1, -1]),
        ("acute minor seventh", &[-4, 6, -2]),
        ("Pythagorean tritone", &[-9, 6]),
        ("acute major second", &[-7, 6, -1]),
        ("undecimal major diesis", &[-6, 6, 0, 0, -1]),
        ("squbema", &[-3, 6, 0, -1, 0, -1]),
        ("vicesimotertial comma", &[5, -6, 0, 0, 0, 0, 0, 0, 1]),
        (
            "ancient Chinese quasi-equal fifth",
            &[
                -2, 0, -3, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                1,
            ],
        ),
        (
            "ancient Chinese tempering",
            &[
                1, 1, 3, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                -1,
            ],
        ),
        ("grave whole tone", &[5, -6, 2]),
        ("Cuthbert comma", &[0, 0, -1, 1, 2, -2]),
        ("keema", &[-5, -3, 3, 1]),
        ("undecimal semicomma, pentacircle", &[7, -4, 0, 1, -1]),
        ("fairytale comma", &[-3, 0, -3, 1, 1, 1]),
        ("narrow diminished sixth", &[10, -3, -2]),
        ("Pythagorean diminished fifth", &[10, -6]),
        ("keega", &[-3, 1, -3, 3]),
        ("gamelan residue", &[-10, 1, 0, 3]),
        ("tridecimal major diesis", &[-10, 4, 0, 0, 0, 1]),
        ("double augmented prime", &[-10, 2, 3]),
        ("kestrel comma", &[2, 3, 0, -1, 1, -2]),
        ("wide augmented second", &[-10, 5, 1]),
        ("Eratosthenes' comma", &[6, -5, -1, 0, 0, 0, 0, 1]),
        ("sensmus", &[4, -5, -1, 1, 1]),
        ("grave minor seventh", &[8, -6, 1]),
        ("triaphonisma", &[3, -2, 0, 1, -1, -1, 0, 0, 1]),
        ("aphrowe", &[0, -3, 0, -2, 3]),
        ("hemimin", &[6, 1, 0, 1, -3]),
        ("moctdel", &[-2, 0, 3, -3, 1]),
        ("Nicola", &[0, 2, 2, 1, -2, -1]),
        ("lummic comma", &[2, 1, -1, -3, 1, 1]),
        ("Orwell comma", &[6, 3, -1, -3]),
        // Skipped: "approximation to 1 cent"
        ("double augmented sixth", &[-10, 1, 4]),
        ("2 tritones", &[-10, 4, 2]),
        ("double diminished octave", &[11, -2, -3]),
        ("narrow diminished seventh", &[11, -5, -1]),
        ("double diminished third", &[11, -1, -4]),
        ("diaschisma", &[11, -4, -2]),
        ("Blume comma", &[-11, 0, 0, 0, 2, 0, 1]),
        ("xenisma", &[1, 1, 0, 3, -2, 0, -1]),
        ("ibnsinma", &[5, -3, 1, -1, -1, 1]),
        ("acute major sixth", &[-8, 7, -1]),
        ("Gorgo limma", &[-4, 7, -3]),
        ("apotome", &[-11, 7]),
        ("septendecimal comma", &[-7, 7, 0, 0, 0, 0, -1]),
        ("Parizek comma, petrma", &[3, 0, 2, 0, 1, -3]),
        ("Breedsma", &[-5, -1, -2, 4]),
        ("nuwell comma", &[1, 5, 1, -4]),
        ("grave minor third", &[9, -7, 1]),
        ("Lehmerisma", &[-4, -3, 2, -1, 2]),
        ("small diesis, magic comma", &[-10, -1, 5]),
        ("major BP diesis", &[0, -2, 5, -3]),
        ("middle second comma", &[6, 0, -5, 2]),
        ("double augmented fifth", &[-11, 3, 3]),
        ("myhemiwell", &[2, -3, -3, 1, 2]),
        ("wide augmented sixth", &[-11, 6, 1]),
        ("small septimal comma", &[5, -4, 3, -2]),
        ("undecimal schisma", &[5, -1, 3, 0, -3]),
        ("Pythagorean diminished octave", &[12, -7]),
        ("4 septatones or septatonic major sixth", &[12, 0, 0, -4]),
        ("double diminished fourth", &[12, -3, -3]),
        ("narrow diminished third", &[12, -6, -1]),
        (
            "tridecimal schisma, Sagittal schismina",
            &[12, -2, -1, -1, 0, -1],
        ),
        ("Hunt flat 2 comma", &[-12, 5, 0, 0, 0, 0, 1]),
        ("leprechaun comma", &[-7, -1, 2, 0, -1, 2]),
        ("ragisma", &[-1, -7, 4, 1]),
        ("Arabic neutral second", &[9, 2, -1, -1, -2]),
        ("Beta 5, Garibaldi comma", &[10, -6, 1, -1]),
        ("double augmented third", &[-12, 2, 4]),
        ("octave - small diesis", &[11, 1, -5]),
        ("porwell comma", &[11, 1, -3, -2]),
        ("Pythagorean augmented fifth", &[-12, 8]),
        ("acute major third", &[-10, 8, -1]),
        ("BP major link", &[0, 8, -3, -2]),
        ("ripple", &[-1, 8, -5]),
        ("Mathieu superdiesis", &[-8, 8, -2]),
        ("Triple BP comma", &[0, -8, 1, 0, 3]),
        ("jacobin comma", &[9, 0, -1, 0, -3, 1]),
        ("double diminished sixth", &[13, -2, -4]),
        ("Pythagorean diminished fourth", &[13, -8]),
        ("undecimal minor diesis", &[13, -6, 0, 0, -1]),
        ("kalisma, Gauss' comma", &[-3, 4, -2, -2, 2]),
        ("double augmented second", &[-13, 4, 3]),
        ("grave minor sixth", &[11, -8, 1]),
        ("harmonisma", &[3, -2, 0, -1, 3, -2]),
        (
            "fourth + schisma, 5-limit approximation to ET fourth",
            &[-13, 7, 1],
        ),
        ("hemimage", &[5, -7, -1, 3]),
        ("cantonisma", &[-5, 0, 1, -3, 0, 3]),
        ("great BP diesis", &[0, -7, 6, -1]),
        ("kleisma, semicomma majeur", &[-6, -5, 6]),
        ("double diminished seventh", &[14, -4, -3]),
        (
            "fifth - schisma, 5-limit approximation to ET fifth",
            &[14, -7, -1],
        ),
        ("cloudy", &[-14, 0, 0, 5]),
        ("double augmentation diesis, Negri comma", &[-14, 3, 4]),
        ("small BP diesis, mirkwai comma", &[0, 3, 4, -5]),
        ("septimal major diesis", &[3, 7, 0, -5]),
        ("minimal BP chroma", &[0, 6, 2, -5]),
        // Skipped: "greater harmonisma"
        ("octave - minimal diesis", &[-4, 9, -4]),
        ("acute major seventh", &[-11, 9, -1]),
        ("Pythagorean augmented second", &[-14, 9]),
        ("cataharry comma", &[-4, 9, -2, -2]),
        ("minimal diesis", &[5, -9, 4]),
        ("grave minor second", &[12, -9, 1]),
        ("maximal BP chroma", &[0, -9, 5, 1]),
        // Skipped: "lesser harmonisma",
        ("mechanism comma", &[-2, -8, 0, 4, 1]),
        ("Secorian", &[12, -7, 0, 1, 0, -1]),
        ("octave - double augmentation diesis", &[15, -3, -4]),
        ("Pythagorean diminished seventh", &[15, -9]),
        (
            "5 septatones or septatonic diminished octave",
            &[15, 0, 0, -5],
        ),
        ("schisma", &[-15, 8, 1]),
        ("mirwomo comma", &[-15, 3, 2, 2]),
        ("hemigail", &[-7, 1, 0, -3, 4]),
        ("trimyna", &[-4, 1, -5, 5]),
        // Skipped: "Mersenne's quasi-equal semitone"
        ("Pythagorean augmented sixth", &[-15, 10]),
        ("Harrison's comma", &[-13, 10, 0, -1]),
        ("Squalentine", &[-9, 3, -3, 4]),
        ("octave - schisma", &[16, -8, -1]),
        ("Pythagorean diminished third", &[16, -10]),
        ("orgonisma", &[16, 0, 0, -2, -3]),
        ("horwell comma", &[-16, 1, 5, 1]),
        ("Woolhouse semitone", &[-13, -2, 7]),
        ("medium semicomma, Sensi comma", &[2, 9, -7]),
        ("BP minor link", &[0, 5, -7, 3]),
        ("stearnsma", &[1, 10, 0, -6]),
        ("chalmersia", &[-6, 6, -2, -1, -1, 2]),
        ("Hunt 19-cycle comma", &[17, 0, 0, 0, 0, 0, 0, -4]),
        ("Woolhouse major seventh", &[14, 2, -7]),
        ("odiheim", &[-1, 2, -4, 5, -2]),
        ("Pythagorean augmented third", &[-17, 11]),
        ("tolerma", &[10, -11, 2, 1]),
        ("supraminor scintillisma", &[-4, 4, -1, 4, -1, -1, -1]),
        ("sesdecal", &[-4, 1, 7, 0, -4]),
        ("Landscape comma", &[-4, 6, -6, 3]),
        ("Pythagorean diminished sixth", &[18, -11]),
        ("Passion comma", &[18, -4, -5]),
        ("varunisma", &[-9, 8, -4, 2]),
        ("octave - Würschmidt's comma", &[-16, -1, 8]),
        ("doublewide", &[-9, -6, 8]),
        ("dimcomp comma", &[-1, -4, 8, -4]),
        ("Würschmidt's comma", &[17, 1, -8]),
        ("BP small link", &[0, 10, -8, 1]),
        ("wizma", &[-6, -8, 2, 5]),
        ("Pythagorean augmented seventh", &[-18, 12]),
        ("Pythagorean comma, ditonic comma", &[-19, 12]),
        ("quince", &[-15, 0, -2, 7]),
        ("complementary BP diesis", &[0, -8, -3, 7]),
        ("Pythagorean diminished ninth", &[20, -12]),
        ("Pythagorean double augmented fourth", &[-20, 13]),
        ("Unicorn comma", &[-2, 13, -8]),
        ("Amity comma, kleisma - schisma", &[9, -13, 5]),
        ("Immunity comma", &[16, -13, 2]),
        ("Shibboleth comma", &[-5, -10, 9]),
        ("Pythagorean double diminished fifth", &[21, -13]),
        ("semicomma, Fokker's comma", &[-21, 3, 7]),
        ("Pythagorean double augmented prime", &[-22, 14]),
        ("sevond", &[6, -14, 7]),
        ("Pythagorean double diminished octave", &[23, -14]),
        ("Fifives comma", &[-1, -14, 10]),
        ("mynic", &[9, 9, -10]),
        ("Pythagorean double augmented fifth", &[-23, 15]),
        ("Pythagorean double diminished fourth", &[24, -15]),
        ("Freivald comma", &[22, -1, -10, 1]),
        ("Beta 2, septimal schisma", &[25, -14, 0, -1]),
        ("Ampersand's comma", &[-25, 7, 6]),
        ("Pythagorean double augmented second", &[-25, 16]),
        ("Sycamore comma", &[-16, -6, 11]),
        ("nusecond", &[5, 13, -11]),
        ("Pythagorean double diminished seventh", &[26, -16]),
        ("Misty comma, diaschisma - schisma", &[26, -12, -3]),
        ("Pythagorean double augmented sixth", &[-26, 17]),
        ("gravity comma", &[-13, 17, -6]),
        ("roda", &[20, -17, 3]),
        (
            "whole tone - 2 schismas, 5-limit approximation to ET whole tone",
            &[27, -14, -2],
        ),
        ("Pythagorean double diminished third", &[27, -17]),
        ("wadisma", &[-26, -1, 1, 9]),
        ("Pythagorean double augmented third", &[-28, 18]),
        ("Pythagorean double diminished sixth", &[29, -18]),
        ("Blackjack comma", &[-10, 7, 8, -7]),
        ("Pythagorean double augmented seventh", &[-29, 19]),
        ("Pythagorean-19 comma", &[-30, 19]),
        ("Trithagorean comma", &[0, -19, 13]),
        ("ditonma", &[-27, -2, 13]),
        ("parakleisma", &[8, 14, -13]),
        ("Vishnu comma", &[23, 6, -14]),
        ("semithirds comma", &[38, -2, -15]),
        ("ennealimmal comma", &[1, -27, 18]),
        ("'19-tone' comma", &[-14, -19, 19]),
        ("monzisma", &[54, -37, 2]),
        ("'41-tone' comma", &[65, -41]),
        ("Mercator's comma", &[-84, 53]),
    ];

    commas
        .iter()
        .map(|&(description, prime_factors)| Comma::new(description, prime_factors))
        .collect()
}
