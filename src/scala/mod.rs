//! Interop with [Scala](http://www.huygens-fokker.org/scala/) tuning files.

mod import;

use std::{
    borrow::Borrow,
    convert::TryFrom,
    fmt::{self, Display, Formatter},
    io::Read,
    ops::{Neg, Range},
    str::FromStr,
};

use crate::{
    key::PianoKey,
    math,
    note::{Note, PitchedNote},
    parse,
    pitch::{Pitch, Ratio},
    tuning::{Approximation, KeyboardMapping, Scale, Tuning},
};

pub use self::import::*;

/// Scale format according to <http://www.huygens-fokker.org/scala/scl_format.html>.
///
/// The [`Scl`] format describes a periodic scale in *relative* pitches. You can access those pitches using [`Scl::relative_pitch_of`].
/// To retrieve *absolute* [`Pitch`] information, you need to pair the [`Scl`] struct with a [`Kbm`] or [`KbmRoot`] struct (see implementations of the [`Tuning`] or [`KeyboardMapping`] trait for more info).
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::key::PianoKey;
/// # use tune::note::Note;
/// # use tune::pitch::Ratio;
/// # use tune::pitch::Pitch;
/// # use tune::scala;
/// # use tune::scala::KbmRoot;
/// use tune::tuning::Tuning;
///
/// let scl = scala::create_harmonics_scale(None, 8, 8, false).unwrap();
/// let kbm = KbmRoot::from(Note::from_midi_number(43).at_pitch(Pitch::from_hz(100.0)));
/// let tuning = (scl, kbm);
///
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(43)).as_hz(), 100.0);
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(44)).as_hz(), 112.5);
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(45)).as_hz(), 125.0);
/// ```
#[derive(Clone, Debug)]
pub struct Scl {
    description: String,
    period: Ratio,
    num_items: u16,
    pitch_values: Vec<PitchValue>,
    pitch_value_ordering: Vec<usize>,
}

impl Scl {
    pub fn builder() -> SclBuilder {
        SclBuilder {
            period: Ratio::default(),
            pitch_values: Vec::new(),
            pitch_value_ordering: Vec::new(),
        }
    }

    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into()
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn period(&self) -> Ratio {
        self.period
    }

    pub fn num_items(&self) -> u16 {
        self.num_items
    }

    /// Retrieves relative pitches without requiring any [`Kbm`] reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_cents(50.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.relative_pitch_of(1).as_cents(), 100.0);
    /// assert_approx_eq!(scl.relative_pitch_of(2).as_cents(), 50.0);
    /// assert_approx_eq!(scl.relative_pitch_of(3).as_cents(), 150.0);
    /// assert_approx_eq!(scl.relative_pitch_of(4).as_cents(), 250.0);
    /// ```
    pub fn relative_pitch_of(&self, degree: i32) -> Ratio {
        // DRY
        let (num_periods, degree_within_period) = math::i32_dr_u(degree, self.num_items());
        let ratio_within_period = if degree_within_period == 0 {
            Ratio::default()
        } else {
            self.pitch_values[usize::from(degree_within_period - 1)].as_ratio()
        };
        self.period
            .repeated(num_periods)
            .stretched_by(ratio_within_period)
    }

    /// Retrieves relative pitches in ascending order without requiring any [`Kbm`] reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_cents(50.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(1).as_cents(), 50.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(2).as_cents(), 100.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(3).as_cents(), 150.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(4).as_cents(), 200.0);
    /// ```
    pub fn sorted_relative_pitch_of(&self, degree: i32) -> Ratio {
        // DRY
        let (num_periods, degree_within_period) = math::i32_dr_u(degree, self.num_items());
        let ratio_within_period = if degree_within_period == 0 {
            Ratio::default()
        } else {
            let index = self.pitch_value_ordering[usize::from(degree_within_period - 1)];
            self.pitch_values[index].as_ratio()
        };
        self.period
            .repeated(num_periods)
            .stretched_by(ratio_within_period)
    }

    /// Finds the approximate degree of a relative pitch without requiring any [`Kbm`] reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_cents(50.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(0.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(10.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(40.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(50.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(100.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(150.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(200.0)).approx_value, 5);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(250.0)).approx_value, 4);
    /// ```
    pub fn find_by_relative_pitch(&self, relative_pitch: Ratio) -> Approximation<i32> {
        let approximation = self.find_by_relative_pitch_internal(relative_pitch);
        let (_, scale_degree, num_periods) = approximation.approx_value;
        Approximation {
            approx_value: scale_degree + num_periods * i32::from(self.num_items()),
            deviation: approximation.deviation,
        }
    }

    /// Finds the approximate degree of a relative pitch in ascending order without requiring any [`Kbm`] reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Ratio;
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_cents(50.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(0.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(10.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(40.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(50.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(100.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(150.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(200.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(250.0)).approx_value, 5);
    /// ```
    pub fn find_by_relative_pitch_sorted(&self, relative_pitch: Ratio) -> Approximation<i32> {
        let approximation = self.find_by_relative_pitch_internal(relative_pitch);
        let (scale_degree, _, num_periods) = approximation.approx_value;
        Approximation {
            approx_value: scale_degree + num_periods * i32::from(self.num_items()),
            deviation: approximation.deviation,
        }
    }

    fn find_by_relative_pitch_internal(
        &self,
        relative_pitch: Ratio,
    ) -> Approximation<(i32, i32, i32)> {
        let ratio_to_find = Ratio::from_octaves(
            relative_pitch
                .as_octaves()
                .rem_euclid(self.period.as_octaves()),
        );

        let lower_index_in_sorted_pitch_list = self
            .pitch_value_ordering
            .binary_search_by(|&probe| {
                self.pitch_values[probe]
                    .as_ratio()
                    .partial_cmp(&ratio_to_find)
                    .expect("Comparison failed")
            })
            .unwrap_or_else(|inexact_match| inexact_match)
            // Due to floating-point errors there is no guarantee that binary_search returns an index smaller than the scale size.
            .min(self.pitch_value_ordering.len() - 1);

        let lower_scale_degree = match lower_index_in_sorted_pitch_list {
            0 => 0,
            _ => self.pitch_value_ordering[lower_index_in_sorted_pitch_list - 1] + 1,
        };
        let lower_ratio = match lower_scale_degree {
            0 => Ratio::default(),
            _ => self.pitch_values[lower_scale_degree - 1].as_ratio(),
        };

        let upper_scale_degree = self.pitch_value_ordering[lower_index_in_sorted_pitch_list] + 1;
        let upper_ratio = self.pitch_values[upper_scale_degree - 1].as_ratio();

        let lower_deviation = ratio_to_find.deviation_from(lower_ratio);
        let upper_deviation = upper_ratio.deviation_from(ratio_to_find);

        let round_to_lower_step = Ratio::from_float(1.000001);
        let (index_in_sorted_pitch_list, scale_degree, deviation) =
            if lower_deviation < upper_deviation.stretched_by(round_to_lower_step) {
                (
                    lower_index_in_sorted_pitch_list,
                    lower_scale_degree,
                    lower_deviation,
                )
            } else {
                (
                    lower_index_in_sorted_pitch_list + 1,
                    upper_scale_degree,
                    upper_deviation.inv(),
                )
            };

        let num_periods = relative_pitch
            .as_octaves()
            .div_euclid(self.period.as_octaves());

        Approximation {
            approx_value: (
                index_in_sorted_pitch_list as i32,
                scale_degree as i32,
                num_periods as i32,
            ),
            deviation,
        }
    }

    /// Imports the given file in SCL format.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::scala::Scl;
    /// let scl_file = &[
    ///     "!A comment",
    ///     "  !An indented comment",
    ///     "  Example scale  ",
    ///     "7",
    ///     "100.",
    ///     "150.0 ignore text after first whitespace",
    ///     "  ", // ignore blank line
    ///     "!175.0 ignore whole line",
    ///     "200.0 .ignore additional dots",
    ///     "  6/5  ",
    ///     "5/4 (ignore parentheses)",
    ///     "3/2 /ignore additional slashes",
    ///     "2",
    /// ];
    ///
    /// let scl = Scl::import(scl_file.join("\n").as_bytes()).unwrap();
    ///
    /// assert_eq!(scl.description(), "Example scale");
    /// assert_eq!(scl.num_items(), 7);
    /// assert_approx_eq!(scl.relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.relative_pitch_of(1).as_cents(), 100.0);
    /// assert_approx_eq!(scl.relative_pitch_of(2).as_cents(), 150.0);
    /// assert_approx_eq!(scl.relative_pitch_of(3).as_cents(), 200.0);
    /// assert_approx_eq!(scl.relative_pitch_of(4).as_float(), 6.0 / 5.0);
    /// assert_approx_eq!(scl.relative_pitch_of(5).as_float(), 5.0 / 4.0);
    /// assert_approx_eq!(scl.relative_pitch_of(6).as_float(), 3.0 / 2.0);
    /// assert_approx_eq!(scl.relative_pitch_of(7).as_float(), 2.0);
    /// assert_approx_eq!(scl.period().as_float(), 2.0);
    /// ```
    pub fn import(reader: impl Read) -> Result<Self, SclImportError> {
        import::import_scl(reader)
    }

    /// Exports the current scale in SCL file format.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::scala::Scl;
    /// let scl = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_ratio("1:13:3".parse().unwrap())
    ///     .push_fraction(4, 3)
    ///     .push_int(2)
    ///     .build_with_description("Example scale")
    ///     .unwrap();
    ///
    /// assert_eq!(
    ///     format!("{}", scl.export()).lines().collect::<Vec<_>>(),
    ///     ["Example scale", "4", "100.000", "146.304", "4/3", "2"]
    /// );
    /// ```
    pub fn export(&self) -> SclExport {
        SclExport(self)
    }
}

/// Builder created by [`Scl::builder`].
pub struct SclBuilder {
    period: Ratio,
    pitch_values: Vec<PitchValue>,
    pitch_value_ordering: Vec<(usize, Ratio)>,
}

impl SclBuilder {
    pub fn push_ratio(self, ratio: Ratio) -> Self {
        self.push_cents(ratio.as_cents())
    }

    pub fn push_cents(self, cents_value: f64) -> Self {
        self.push_pitch_value(PitchValue::Cents(cents_value))
    }

    pub fn push_int(self, int_value: u32) -> Self {
        self.push_pitch_value(PitchValue::Fraction(int_value, None))
    }

    pub fn push_fraction(self, numer: u32, denom: u32) -> Self {
        self.push_pitch_value(PitchValue::Fraction(numer, Some(denom)))
    }

    fn push_pitch_value(mut self, pitch_value: PitchValue) -> Self {
        self.period = pitch_value.as_ratio();
        self.pitch_values.push(pitch_value);
        self.pitch_value_ordering
            .push((self.pitch_value_ordering.len(), pitch_value.as_ratio()));
        self
    }

    pub fn build(self) -> Result<Scl, SclBuildError> {
        let description = if let [single_pitch_value] = self.pitch_values.as_slice() {
            let step_size = single_pitch_value.as_ratio();
            format!(
                "equal steps of {:#} ({:.2}-EDO)",
                step_size,
                Ratio::octave().num_equal_steps_of_size(step_size)
            )
        } else {
            "Custom scale".to_owned()
        };
        self.build_with_description(description)
    }

    pub fn build_with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Scl, SclBuildError> {
        self.pitch_value_ordering
            .sort_by(|a, b| a.1.partial_cmp(&b.1).expect("Comparison failed"));

        if let [(_, first), .., (_, last)] = self.pitch_value_ordering.as_slice() {
            if first < &Ratio::default() || last > &self.period {
                return Err(SclBuildError::ItemOutOfRange);
            }
        }
        if self.period == Ratio::default() {
            Err(SclBuildError::ScaleIsTrivial)
        } else {
            let pitch_value_ordering = self
                .pitch_value_ordering
                .into_iter()
                .map(|(order, _)| order)
                .collect::<Vec<_>>();

            let num_items = u16::try_from(pitch_value_ordering.len())
                .map_err(|_| SclBuildError::ScaleTooLarge)?;

            Ok(Scl {
                description: description.into(),
                period: self.period,
                num_items,
                pitch_values: self.pitch_values,
                pitch_value_ordering,
            })
        }
    }
}

/// Error reported when building an [`Scl`] fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SclBuildError {
    /// The scale does not contain any items except for the default ratio (0 cents).
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::scala::Scl;
    /// # use tune::scala::SclBuildError;
    /// let empty_scale = Scl::builder().build();
    /// assert_eq!(empty_scale.unwrap_err(), SclBuildError::ScaleIsTrivial);
    ///
    /// let zero_entries_scale = Scl::builder().push_cents(0.0).push_cents(0.0).build();
    /// assert_eq!(
    ///     zero_entries_scale.unwrap_err(),
    ///     SclBuildError::ScaleIsTrivial
    /// );
    /// ```
    ScaleIsTrivial,

    /// The scale contains an item that is smaller than the default ratio (0 cents) or larger than the period.
    /// ```
    /// # use tune::scala::Scl;
    /// # use tune::scala::SclBuildError;
    /// let item_greater_than_period = Scl::builder()
    ///     .push_cents(100.0)
    ///     .push_cents(200.0)
    ///     .push_cents(150.0)
    ///     .push_cents(50.0)
    ///     .build();
    /// assert_eq!(
    ///     item_greater_than_period.unwrap_err(),
    ///     SclBuildError::ItemOutOfRange
    /// );
    ///
    /// let item_smaller_than_zero = Scl::builder()
    ///     .push_cents(-100.0)
    ///     .push_cents(50.0)
    ///     .push_cents(150.0)
    ///     .push_cents(200.0)
    ///     .build();
    /// assert_eq!(
    ///     item_smaller_than_zero.unwrap_err(),
    ///     SclBuildError::ItemOutOfRange
    /// );
    ///
    /// let item_greater_than_zero_period = Scl::builder().push_cents(50.0).push_cents(0.0).build();
    /// assert_eq!(
    ///     item_greater_than_zero_period.unwrap_err(),
    ///     SclBuildError::ItemOutOfRange
    /// );

    /// ```
    ItemOutOfRange,

    /// There are too many items in this scale.
    ///
    /// No example is given since it would require building a scale with 65536 or more items which is incredibly slow in test mode.
    ScaleTooLarge,
}

#[derive(Copy, Clone, Debug)]
enum PitchValue {
    Cents(f64),
    Fraction(u32, Option<u32>),
}

impl PitchValue {
    fn as_ratio(self) -> Ratio {
        match self {
            PitchValue::Cents(cents_value) => Ratio::from_cents(cents_value),
            PitchValue::Fraction(numer, denom) => {
                Ratio::from_float(f64::from(numer) / f64::from(denom.unwrap_or(1)))
            }
        }
    }
}

impl Display for PitchValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PitchValue::Cents(cents) => write!(f, "{:.3}", cents),
            PitchValue::Fraction(numer, Some(denom)) => write!(f, "{}/{}", numer, denom),
            PitchValue::Fraction(numer, None) => write!(f, "{}", numer),
        }
    }
}

/// Format / [`Display`] wrapper created by [`Scl::export`].
pub struct SclExport<'a>(&'a Scl);

impl<'a> Display for SclExport<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{}", self.0.description())?;
        writeln!(f, "{}", self.0.pitch_values.len())?;
        for pitch_value in &self.0.pitch_values {
            writeln!(f, "{}", pitch_value)?;
        }
        Ok(())
    }
}

/// Keyboard mappings according to <http://www.huygens-fokker.org/scala/help.htm#mappings>.
///
/// To better understand how keyboard mappings work have a look at the documented methods of this struct.
///
/// For more specialized linear keyboard mappings use [`KbmRoot`].
#[derive(Clone, Debug)]
pub struct Kbm {
    kbm_root: KbmRoot,
    range: Range<PianoKey>,
    num_items: u16,
    key_mapping: Vec<Option<i16>>,
    formal_octave: i16,
}

impl Kbm {
    pub fn builder(kbm_root: impl Into<KbmRoot>) -> KbmBuilder {
        KbmBuilder {
            kbm_root: kbm_root.into(),
            range: PianoKey::from_midi_number(0)..PianoKey::from_midi_number(128),
            key_mapping: Vec::new(),
            formal_octave: 0,
        }
    }

    pub fn kbm_root(&self) -> KbmRoot {
        self.kbm_root
    }

    pub fn set_kbm_root(&mut self, kbm_root: KbmRoot) {
        self.kbm_root = kbm_root
    }

    pub fn range(&self) -> Range<PianoKey> {
        self.range.clone()
    }

    pub fn formal_octave(&self) -> i16 {
        self.formal_octave
    }

    pub fn num_items(&self) -> u16 {
        self.num_items
    }

    /// Returns the scale degree for the given [`PianoKey`] .
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// let kbm = Kbm::builder(Note::from_midi_number(62))
    ///    .range(PianoKey::from_midi_number(10)..PianoKey::from_midi_number(100))
    ///
    ///    // KBM degree 0 maps to SCL degree 0
    ///    .push_mapped_key(0)
    ///
    ///    // KBM degree 1 maps to SCL degree 4
    ///    .push_mapped_key(4)
    ///
    ///    // KBM degree 2 is unmapped
    ///    .push_unmapped_key()
    ///
    ///    // KBM degree 3 maps to SCL degree 4 again (!)
    ///    .push_mapped_key(4)
    ///
    ///    // A KBM degree shift of 4 (num_items) leads to an SCL degree shift of 17 (formal_octave)
    ///    .formal_octave(17)
    ///
    ///    .build()
    ///    .unwrap();
    ///
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(10)), Some(-221));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(60)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(61)), Some(-13));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(62)), Some(0));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(63)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(64)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(65)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(66)), Some(17));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(67)), Some(21));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(68)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(99)), Some(157));
    ///
    /// // Not in the range 10..100
    /// for midi_number in (0..10).chain(100..128) {
    ///     assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(midi_number)), None);
    /// }
    ///
    /// // If the mapping is empty, a linear mapping is assumed.
    /// let empty_kbm = Kbm::builder(Note::from_midi_number(62))
    ///
    ///     // This has no effect
    ///     .formal_octave(42)
    ///
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(empty_kbm.scale_degree_of(PianoKey::from_midi_number(61)), Some(-1));
    /// assert_eq!(empty_kbm.scale_degree_of(PianoKey::from_midi_number(62)), Some(0));
    /// assert_eq!(empty_kbm.scale_degree_of(PianoKey::from_midi_number(63)), Some(1));
    /// ```
    pub fn scale_degree_of(&self, key: PianoKey) -> Option<i32> {
        if !self.range.contains(&key) {
            return None;
        }
        let key_degree = self.kbm_root.origin.num_keys_before(key);
        if self.num_items == 0 {
            return Some(key_degree);
        }
        let (factor, index) = math::i32_dr_u(key_degree, self.num_items);
        self.key_mapping[usize::from(index)]
            .map(|deg| i32::from(deg) + factor * i32::from(self.formal_octave))
    }

    /// Imports the given file in KBM format.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::key::PianoKey;
    /// # use tune::scala::Kbm;
    /// let input = &[
    ///     "!A comment",
    ///     "  !An indented comment",
    ///     "6 <- Official map size. Can be larger than the number of provided mapping entries!",
    ///     "10",
    ///     "99 (Rust's Range type is right exclusive. The upper bound becomes 100.)",
    ///     "62",
    ///     "  69  ",
    ///     "432.0 = healing frequency",
    ///     "17",
    ///     "! Start of the mapping table",
    ///     "0",
    ///     "4",
    ///     "x means unmapped",
    ///     "4",
    ///     "X - uppercase is supported",
    ///     "! End of the mapping table. 'x'es are added to match the official map size.",
    /// ];
    ///
    /// let kbm = Kbm::import(input.join("\n").as_bytes()).unwrap();
    ///
    /// assert_eq!(kbm.kbm_root().origin.midi_number(), 62);
    /// assert_approx_eq!(kbm.kbm_root().ref_pitch.as_hz(), 432.0);
    /// assert_eq!(kbm.kbm_root().ref_degree, 7);
    /// assert_eq!(kbm.range().start.midi_number(), 10);
    /// assert_eq!(kbm.range().end.midi_number(), 100);
    /// assert_eq!(kbm.formal_octave(), 17);
    /// assert_eq!(kbm.num_items(), 6);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(62)), Some(0));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(63)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(64)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(65)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(66)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(67)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(68)), Some(17));
    /// ```
    pub fn import(reader: impl Read) -> Result<Self, KbmImportError> {
        import::import_kbm(reader)
    }

    /// Exports the current keyboard mapping in KBM file format.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// # use tune::scala::KbmRoot;
    /// # use tune::pitch::Pitch;
    /// let mut kbm_root = KbmRoot::from(Note::from_midi_number(69).at_pitch(Pitch::from_hz(432.0)))
    ///     .shift_origin_by(-9);
    ///
    /// // White keys on 22-edo
    /// let kbm = Kbm::builder(kbm_root)
    ///    .range(PianoKey::from_midi_number(10)..PianoKey::from_midi_number(100))
    ///    .push_mapped_key(0)
    ///    .push_unmapped_key()
    ///    .push_mapped_key(4)
    ///    .push_unmapped_key()
    ///    .push_mapped_key(8)
    ///    .push_mapped_key(9)
    ///    // ... etc.
    ///    .formal_octave(22)
    ///    .build()
    ///    .unwrap();
    ///
    /// assert_eq!(
    ///     format!("{}", kbm.export()).lines().collect::<Vec<_>>(),
    ///     ["6", "10", "99", "60", "69", "432.000", "22", "0", "x", "4", "x", "8", "9"]
    /// );
    /// ```
    pub fn export(&self) -> KbmExport {
        KbmExport(self)
    }
}

/// Defines an absolute horizontal and vertical location of a scale.
///
/// [`KbmRoot`] is intended to be used in combination with [`Scl`] to form a [`Tuning`].
/// The interesting thing about a [`Tuning`] is that it offers a bidirectional key-to-pitch mapping.
/// This means it is possible to find the best matching [`PianoKey`] for a given [`Pitch`] input.
/// The pitch input can be a continuous value, e.g. the location of a mouse pointer.
///
/// In order to enable invertibility the mapping described by [`KbmRoot`] is linear.
/// In other words, the keyboard mapping degree and the scale degree are the same number.
/// If the mapping is required to be non-linear [`KbmRoot`] needs to be surrounded by the more general [`Kbm`] struct.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KbmRoot {
    /// The [`PianoKey`] that maps to degree 0 of the keyboard mapping.
    /// If a [`Kbm`] surrounding is used with the first entry being *n*, `origin` maps to scale degree *n*.
    pub origin: PianoKey,

    /// A [`Pitch`] that is guaranteed to be present in a [`Tuning`] but which might be skipped in the [`KeyboardMapping`] spanned by the [`Kbm`] surrounding.
    pub ref_pitch: Pitch,

    /// The scale degree for which `ref_pitch` is given as a reference.
    pub ref_degree: i32,
}

impl KbmRoot {
    /// Shifts the `origin` of a scale by `num_degrees` conserving the scale's vertical location.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::pitch::Pitch;
    /// # use tune::scala::KbmRoot;
    /// let kbm_root =  KbmRoot {
    ///     origin: PianoKey::from_midi_number(67),
    ///     ref_pitch: Pitch::from_hz(432.0),
    ///     ref_degree: 2,
    /// };
    ///
    /// assert_eq!(
    ///     kbm_root.shift_origin_by(-7),
    ///     KbmRoot {
    ///         origin: PianoKey::from_midi_number(60),
    ///         ref_pitch: Pitch::from_hz(432.0),
    ///         ref_degree: 9,
    ///     }
    /// );
    /// ```
    pub fn shift_origin_by(self, num_degrees: i32) -> Self {
        Self {
            origin: self.origin.plus_steps(num_degrees),
            ref_pitch: self.ref_pitch,
            ref_degree: self.ref_degree - num_degrees,
        }
    }

    /// Creates a quasi-equivalent [`Kbm`] surrounding which can be exported.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::key::PianoKey;
    /// # use tune::note::Note;
    /// # use tune::scala::KbmRoot;
    /// let kbm_root = KbmRoot::from(Note::from_midi_number(62));
    /// let kbm = kbm_root.to_kbm();
    ///
    /// assert_eq!(kbm.kbm_root(), kbm_root);
    /// assert_eq!(kbm.range(), PianoKey::from_midi_number(0)..PianoKey::from_midi_number(128));
    /// assert_eq!(kbm.formal_octave(), 1);
    /// assert_eq!(kbm.num_items(), 1);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(61)), Some(-1));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(62)), Some(0));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(63)), Some(1));
    ///
    /// let exported = kbm.export();
    /// ```
    pub fn to_kbm(self) -> Kbm {
        Kbm::builder(self)
            .push_mapped_key(0)
            .formal_octave(1)
            .build()
            .unwrap()
    }
}

impl<N: PitchedNote> From<N> for KbmRoot {
    fn from(note: N) -> Self {
        Self {
            origin: note.note().as_piano_key(),
            ref_pitch: note.pitch(),
            ref_degree: 0,
        }
    }
}

impl FromStr for KbmRoot {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let [note, pitch] = parse::split_balanced(s, '@').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let pitch: Pitch = pitch
                .parse()
                .map_err(|e| format!("Invalid pitch '{}': {}", pitch, e))?;
            Ok(Note::from_midi_number(midi_number).at_pitch(pitch).into())
        } else if let [note, delta] = parse::split_balanced(s, '+').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(Note::from_midi_number(midi_number)
                .alter_pitch_by(delta)
                .into())
        } else if let [note, delta] = parse::split_balanced(s, '-').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{}': Must be an integer", note))?;
            let delta = delta
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid delta '{}': {}", delta, e))?;
            Ok(Note::from_midi_number(midi_number)
                .alter_pitch_by(delta.inv())
                .into())
        } else {
            let note_number = s
                .parse::<i32>()
                .map_err(|_| "Must be an expression of type 69, 69@440Hz or 69+100c".to_string())?;
            Ok(Note::from_midi_number(note_number).into())
        }
    }
}

/// Builder created by [`Kbm::builder`].
pub struct KbmBuilder {
    kbm_root: KbmRoot,
    range: Range<PianoKey>,
    key_mapping: Vec<Option<i16>>,
    formal_octave: i16,
}

impl KbmBuilder {
    pub fn range(mut self, range: Range<PianoKey>) -> Self {
        self.range = range;
        self
    }

    pub fn push_mapped_key(mut self, scale_degree: i16) -> Self {
        self.key_mapping.push(Some(scale_degree));
        self
    }

    pub fn push_unmapped_key(mut self) -> Self {
        self.key_mapping.push(None);
        self
    }

    pub fn formal_octave(mut self, formal_octave: i16) -> Self {
        self.formal_octave = formal_octave;
        self
    }

    pub fn build(self) -> Result<Kbm, KbmBuildError> {
        Ok(Kbm {
            kbm_root: self.kbm_root,
            range: self.range,
            num_items: u16::try_from(self.key_mapping.len())
                .map_err(|_| KbmBuildError::MappingTooLarge)?,
            key_mapping: self.key_mapping,
            formal_octave: self.formal_octave,
        })
    }
}

/// Error reported when building a [`Kbm`] fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KbmBuildError {
    MappingTooLarge,
}

/// Format / [`Display`] wrapper created by [`Kbm::export`].
pub struct KbmExport<'a>(&'a Kbm);

impl<'a> Display for KbmExport<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let kbm_root = self.0.kbm_root();
        writeln!(f, "{}", self.0.num_items())?;
        writeln!(f, "{}", self.0.range().start.midi_number())?;
        writeln!(f, "{}", self.0.range().end.midi_number() - 1)?;
        writeln!(f, "{}", kbm_root.origin.midi_number())?;
        writeln!(
            f,
            "{}",
            kbm_root
                .origin
                .plus_steps(kbm_root.ref_degree)
                .midi_number()
        )?;
        writeln!(f, "{:.3}", kbm_root.ref_pitch.as_hz())?;
        writeln!(f, "{}", self.0.formal_octave())?;
        for degree in &self.0.key_mapping {
            match degree {
                Some(degree) => {
                    writeln!(f, "{}", degree)?;
                }
                None => {
                    writeln!(f, "x")?;
                }
            }
        }

        Ok(())
    }
}

fn root_pitch(scl: &Scl, kbm: &KbmRoot) -> Pitch {
    kbm.ref_pitch / scl.relative_pitch_of(kbm.ref_degree)
}

impl<S: Borrow<Scl>, K: Borrow<KbmRoot>> Tuning<PianoKey> for (S, K) {
    fn pitch_of(&self, key: PianoKey) -> Pitch {
        let degree = self.1.borrow().origin.num_keys_before(key);
        self.pitch_of(degree)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<PianoKey> {
        let degree: Approximation<i32> = self.find_by_pitch(pitch);
        let key =
            PianoKey::from_midi_number(self.1.borrow().origin.midi_number() + degree.approx_value);
        Approximation {
            approx_value: key,
            deviation: degree.deviation,
        }
    }
}

impl<S: Borrow<Scl>, K: Borrow<KbmRoot>> Tuning<i32> for (S, K) {
    fn pitch_of(&self, degree: i32) -> Pitch {
        let scl = self.0.borrow();
        let kbm = self.1.borrow();
        root_pitch(scl, kbm) * scl.relative_pitch_of(degree)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<i32> {
        let scl = self.0.borrow();
        let kbm = self.1.borrow();
        let total_ratio = Ratio::between_pitches(root_pitch(scl, kbm), pitch);
        scl.find_by_relative_pitch(total_ratio)
    }
}

impl<S: Borrow<Scl>, K: Borrow<KbmRoot>> Scale for (S, K) {
    fn sorted_pitch_of(&self, degree: i32) -> Pitch {
        let scl = self.0.borrow();
        let kbm = self.1.borrow();
        root_pitch(scl, kbm) * scl.sorted_relative_pitch_of(degree)
    }

    fn find_by_pitch_sorted(&self, pitch: Pitch) -> Approximation<i32> {
        let scl = self.0.borrow();
        let kbm = self.1.borrow();
        let total_ratio = Ratio::between_pitches(root_pitch(scl, kbm), pitch);
        scl.borrow().find_by_relative_pitch_sorted(total_ratio)
    }
}

/// An ([`Scl`], [`Kbm`]) pair has the complete information to define a [`KeyboardMapping`].
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::key::PianoKey;
/// # use tune::note::Note;
/// # use tune::scala::Kbm;
/// # use tune::scala::Scl;
/// use tune::tuning::KeyboardMapping;
///
/// let scl = Scl::builder()
///    .push_cents(100.0)
///    .build()
///    .unwrap();
///
/// let kbm = Kbm::builder(Note::from_midi_number(62))
///    .push_mapped_key(0)
///    .push_mapped_key(4)
///    .push_unmapped_key()
///    .push_mapped_key(4)
///    .formal_octave(12)
///    .build()
///    .unwrap();
///
/// let f = |midi_number| (&scl, &kbm).maybe_pitch_of(PianoKey::from_midi_number(midi_number));
/// assert_approx_eq!(f(62).unwrap().as_hz(), 293.664768);
/// assert_approx_eq!(f(63).unwrap().as_hz(), 369.994423);
/// assert!(f(64).is_none());
/// assert_approx_eq!(f(65).unwrap().as_hz(), 369.994423);
/// assert_approx_eq!(f(66).unwrap().as_hz(), 587.329536);
/// ```
impl<S: Borrow<Scl>, K: Borrow<Kbm>> KeyboardMapping<PianoKey> for (S, K) {
    fn maybe_pitch_of(&self, key: PianoKey) -> Option<Pitch> {
        let scl = self.0.borrow();
        let kbm = self.1.borrow();
        kbm.scale_degree_of(key)
            .map(|degree| (scl, kbm.kbm_root()).pitch_of(degree))
    }
}

impl<S: Borrow<Scl>, K: Borrow<Kbm>> KeyboardMapping<i32> for (S, K) {
    fn maybe_pitch_of(&self, mapping_degree: i32) -> Option<Pitch> {
        let origin = self.1.borrow().kbm_root().origin;
        self.maybe_pitch_of(origin.plus_steps(mapping_degree))
    }
}

/// Creates a rank-2-temperament scale.
///
/// # Examples
///
/// ```
/// # use tune::pitch::Ratio;
/// # use tune::scala;
/// let pythagorean_major =
///     scala::create_rank2_temperament_scale(
///         None, Ratio::from_float(1.5), 5, 1, Ratio::octave(),
///     ).unwrap();
///
/// assert_eq!(
///     format!("{}", pythagorean_major.export()).lines().collect::<Vec<_>>(),
///     ["5 positive and 1 negative generations of generator 1.5000 (+702.0c) with period 2.0000",
///      "7", "203.910", "407.820", "498.045", "701.955", "905.865", "1109.775", "1200.000"]
/// );
/// ```
pub fn create_rank2_temperament_scale(
    description: impl Into<Option<String>>,
    generator: Ratio,
    num_pos_generations: u16,
    num_neg_generations: u16,
    period: Ratio,
) -> Result<Scl, SclBuildError> {
    let generator_in_cents = generator.as_cents();
    let period_in_cents = period.as_cents();

    let mut pitch_values = Vec::new();
    pitch_values.push(period);

    let pos_range = (1..=num_pos_generations).map(f64::from);
    let neg_range = (1..=num_neg_generations).map(f64::from).map(Neg::neg);
    for generation in pos_range.chain(neg_range) {
        let unbounded_note = generation * generator_in_cents;
        let bounded_note = unbounded_note.rem_euclid(period_in_cents);
        pitch_values.push(Ratio::from_cents(bounded_note));
    }

    pitch_values.sort_by(|a, b| a.partial_cmp(b).expect("Comparison failed"));

    let mut builder = Scl::builder();
    for pitch_value in pitch_values {
        builder = builder.push_ratio(pitch_value)
    }

    let description = description.into().unwrap_or_else(|| {
        format!(
            "{0} positive and {1} negative generations of generator {2} ({2:#}) with period {3}",
            num_pos_generations, num_neg_generations, generator, period
        )
    });
    builder.build_with_description(description)
}

/// Creates a harmonics or subharmonics scale.
///
/// # Examples
///
/// ```
/// # use tune::scala;
/// let harmonics = scala::create_harmonics_scale(None, 9, 7, false).unwrap();
///
/// assert_eq!(
///     format!("{}", harmonics.export()).lines().collect::<Vec<_>>(),
///     ["7 harmonics starting with 9",
///      "7", "10/9", "11/9", "12/9", "13/9", "14/9", "15/9", "16/9"]
/// );
///
/// let subharmonics = scala::create_harmonics_scale(None, 9, 7, true).unwrap();
///
/// assert_eq!(
///     format!("{}", subharmonics.export()).lines().collect::<Vec<_>>(),
///     ["7 subharmonics starting with 9",
///      "7", "16/15", "16/14", "16/13", "16/12", "16/11", "16/10", "16/9"]
/// );
/// ```
pub fn create_harmonics_scale(
    description: impl Into<Option<String>>,
    lowest_harmonic: u16,
    number_of_notes: u16,
    subharmonics: bool,
) -> Result<Scl, SclBuildError> {
    if lowest_harmonic == 0 {
        return Err(SclBuildError::ItemOutOfRange);
    }

    let lowest_harmonic = u32::from(lowest_harmonic);
    let highest_harmonic = lowest_harmonic + u32::from(number_of_notes);

    let mut builder = Scl::builder();
    if subharmonics {
        for harmonic in (lowest_harmonic..highest_harmonic).rev() {
            builder = builder.push_fraction(highest_harmonic, harmonic);
        }
    } else {
        for harmonic in lowest_harmonic..highest_harmonic {
            builder = builder.push_fraction(harmonic + 1, lowest_harmonic);
        }
    }

    let description = description.into().unwrap_or_else(|| {
        let debug_text = if subharmonics {
            "subharmonics"
        } else {
            "harmonics"
        };
        format!(
            "{} {} starting with {}",
            number_of_notes, debug_text, lowest_harmonic
        )
    });
    builder.build_with_description(description)
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::{key::PianoKey, note::NoteLetter, pitch::Pitched};

    use super::*;

    #[test]
    fn equal_temperament_scale_correctness() {
        let bohlen_pierce = Scl::builder()
            .push_ratio("1:13:3".parse().unwrap())
            .build()
            .unwrap();

        assert_eq!(bohlen_pierce.num_items(), 1);
        assert_approx_eq!(bohlen_pierce.period().as_cents(), 146.304_231);

        AssertScale(bohlen_pierce, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(66, 341.466_239)
            .maps_key_to_pitch(67, 371.577_498)
            .maps_key_to_pitch(68, 404.344_036)
            .maps_key_to_pitch(69, 440.000_000)
            .maps_key_to_pitch(70, 478.800_187)
            .maps_key_to_pitch(71, 521.021_862)
            .maps_key_to_pitch(72, 566.966_738)
            .exports_lines(&["equal steps of +146.3c (8.20-EDO)", "1", "146.304"]);
    }

    #[test]
    fn rank2_temperament_scale_correctness() {
        let pythagorean_major = create_rank2_temperament_scale(
            None,
            Ratio::from_float(1.5),
            5,
            1,
            Ratio::from_octaves(1.0),
        )
        .unwrap();

        assert_eq!(pythagorean_major.num_items(), 7);
        assert_approx_eq!(pythagorean_major.period().as_octaves(), 1.0);

        AssertScale(pythagorean_major, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(59, 165.000_000)
            .maps_key_to_pitch(60, 185.625_000)
            .maps_key_to_pitch(61, 208.828_125)
            .maps_key_to_pitch(62, 220.000_000)
            .maps_key_to_pitch(63, 247.500_000)
            .maps_key_to_pitch(64, 278.437_500)
            .maps_key_to_pitch(65, 293.333_333)
            .maps_key_to_pitch(66, 330.000_000)
            .maps_key_to_pitch(67, 371.250_000)
            .maps_key_to_pitch(68, 417.656_250)
            .maps_key_to_pitch(69, 440.000_000)
            .maps_key_to_pitch(70, 495.000_000)
            .maps_key_to_pitch(71, 556.875_000)
            .maps_key_to_pitch(72, 586.666_666)
            .maps_key_to_pitch(73, 660.000_000)
            .maps_key_to_pitch(74, 742.500_000)
            .maps_key_to_pitch(75, 835.312_500)
            .maps_key_to_pitch(76, 880.000_000)
            .maps_key_to_pitch(77, 990.000_000)
            .maps_key_to_pitch(78, 1_113.750_000)
            .maps_key_to_pitch(79, 1_173.333_333)
            .exports_lines(&[
                "5 positive and 1 negative generations of generator 1.5000 (+702.0c) \
                 with period 2.0000",
                "7",
                "203.910",
                "407.820",
                "498.045",
                "701.955",
                "905.865",
                "1109.775",
                "1200.000",
            ]);
    }

    #[test]
    fn harmonics_scale_correctness() {
        let harmonics = create_harmonics_scale(None, 8, 8, false).unwrap();

        assert_eq!(harmonics.num_items(), 8);
        assert_approx_eq!(harmonics.period().as_float(), 2.0);

        AssertScale(harmonics, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(59, 192.500)
            .maps_key_to_pitch(60, 206.250)
            .maps_key_to_pitch(61, 220.000)
            .maps_key_to_pitch(62, 247.500)
            .maps_key_to_pitch(63, 275.000)
            .maps_key_to_pitch(64, 302.500)
            .maps_key_to_pitch(65, 330.000)
            .maps_key_to_pitch(66, 357.500)
            .maps_key_to_pitch(67, 385.000)
            .maps_key_to_pitch(68, 412.500)
            .maps_key_to_pitch(69, 440.000)
            .maps_key_to_pitch(70, 495.000)
            .maps_key_to_pitch(71, 550.000)
            .maps_key_to_pitch(72, 605.000)
            .maps_key_to_pitch(73, 660.000)
            .maps_key_to_pitch(74, 715.000)
            .maps_key_to_pitch(75, 770.000)
            .maps_key_to_pitch(76, 825.000)
            .maps_key_to_pitch(77, 880.000)
            .maps_key_to_pitch(78, 990.000)
            .maps_key_to_pitch(79, 1100.000)
            .exports_lines(&[
                "8 harmonics starting with 8",
                "8",
                "9/8",
                "10/8",
                "11/8",
                "12/8",
                "13/8",
                "14/8",
                "15/8",
                "16/8",
            ]);
    }

    #[test]
    fn build_non_monotonic_scale() {
        let non_monotonic = Scl::builder()
            .push_fraction(7, 5)
            .push_fraction(9, 5)
            .push_fraction(8, 5)
            .push_fraction(6, 5)
            .push_fraction(10, 5)
            .build()
            .unwrap();

        assert_approx_eq!(non_monotonic.period().as_octaves(), 1.0);

        AssertScale(
            non_monotonic,
            NoteLetter::G
                .in_octave(2)
                .at_pitch(Pitch::from_hz(100.0))
                .into(),
        )
        .maps_key_to_pitch(43, 100.0)
        .maps_key_to_pitch(44, 140.0)
        .maps_key_to_pitch(45, 180.0)
        .maps_key_to_pitch(46, 160.0)
        .maps_key_to_pitch(47, 120.0)
        .maps_key_to_pitch(48, 200.0)
        .maps_key_to_pitch(49, 280.0)
        .maps_frequency_to_key_and_deviation(105.0, 43, 105.0 / 100.0)
        .maps_frequency_to_key_and_deviation(115.0, 47, 115.0 / 120.0)
        .maps_frequency_to_key_and_deviation(125.0, 47, 125.0 / 120.0)
        .maps_frequency_to_key_and_deviation(135.0, 44, 135.0 / 140.0)
        .maps_frequency_to_key_and_deviation(145.0, 44, 145.0 / 140.0)
        .maps_frequency_to_key_and_deviation(155.0, 46, 155.0 / 160.0)
        .maps_frequency_to_key_and_deviation(165.0, 46, 165.0 / 160.0)
        .maps_frequency_to_key_and_deviation(175.0, 45, 175.0 / 180.0)
        .maps_frequency_to_key_and_deviation(185.0, 45, 185.0 / 180.0)
        .maps_frequency_to_key_and_deviation(195.0, 48, 195.0 / 200.0)
        .exports_lines(&["Custom scale", "5", "7/5", "9/5", "8/5", "6/5", "10/5"]);
    }

    #[test]
    fn best_fit_correctness() {
        let harmonics = create_harmonics_scale(None, 8, 8, false).unwrap();
        AssertScale(harmonics, NoteLetter::A.in_octave(4).into())
            .maps_frequency_to_key_and_deviation(219.0, 61, 219.0 / 220.0)
            .maps_frequency_to_key_and_deviation(220.0, 61, 220.0 / 220.0)
            .maps_frequency_to_key_and_deviation(221.0, 61, 221.0 / 220.0)
            .maps_frequency_to_key_and_deviation(233.0, 61, 233.0 / 220.0)
            .maps_frequency_to_key_and_deviation(234.0, 62, 234.0 / 247.5)
            .maps_frequency_to_key_and_deviation(330.0, 65, 330.0 / 330.0)
            .maps_frequency_to_key_and_deviation(439.0, 69, 439.0 / 440.0)
            .maps_frequency_to_key_and_deviation(440.0, 69, 440.0 / 440.0)
            .maps_frequency_to_key_and_deviation(441.0, 69, 441.0 / 440.0)
            .maps_frequency_to_key_and_deviation(660.0, 73, 660.0 / 660.0)
            .maps_frequency_to_key_and_deviation(879.0, 77, 879.0 / 880.0)
            .maps_frequency_to_key_and_deviation(880.0, 77, 880.0 / 880.0)
            .maps_frequency_to_key_and_deviation(881.0, 77, 881.0 / 880.0);
    }

    struct AssertScale(Scl, KbmRoot);

    impl AssertScale {
        fn maps_key_to_pitch(&self, midi_number: i32, expected_pitch_hz: f64) -> &Self {
            assert_approx_eq!(
                (&self.0, &self.1)
                    .pitch_of(PianoKey::from_midi_number(midi_number))
                    .as_hz(),
                expected_pitch_hz
            );
            &self
        }

        fn exports_lines(&self, expected_lines: &[&str]) -> &Self {
            let as_string = self.0.export().to_string();
            let lines = as_string.lines().collect::<Vec<_>>();
            assert_eq!(lines, expected_lines);
            self
        }

        fn maps_frequency_to_key_and_deviation(
            &self,
            freq_hz: f64,
            midi_number: i32,
            deviation_as_float: f64,
        ) -> &Self {
            let approximation =
                Pitch::from_hz(freq_hz).find_in_tuning::<PianoKey, _>((&self.0, &self.1));
            assert_eq!(
                approximation.approx_value,
                PianoKey::from_midi_number(midi_number)
            );
            assert_approx_eq!(approximation.deviation.as_float(), deviation_as_float);
            self
        }
    }
}
