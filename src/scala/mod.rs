//! Interop with [Scala](http://www.huygens-fokker.org/scala/) tuning files.

mod import;

use std::{
    borrow::Borrow,
    fmt::{self, Display, Formatter, Write},
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
/// # use tune::scala::SegmentType;
/// use tune::tuning::Tuning;
///
/// let scl = scala::create_harmonics_scale(None, SegmentType::Otonal, 8, 8, None).unwrap();
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
    sorted_pitch_values: Vec<SortedPitchValue>,
}

#[derive(Copy, Clone, Debug)]
struct SortedPitchValue {
    pitch_index: usize,
    reduced_ratio: Ratio,
    num_wraparounds: i32,
}

impl Scl {
    pub fn builder() -> SclBuilder {
        SclBuilder {
            pitch_values: Vec::new(),
        }
        .push_ratio(Ratio::default())
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
    ///     .push_cents(175.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.period().as_cents(), 150.0);
    /// assert_approx_eq!(scl.relative_pitch_of(-1).as_cents(), 25.0); // 25.0 = previous period + 175.0
    /// assert_approx_eq!(scl.relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.relative_pitch_of(1).as_cents(), 100.0);
    /// assert_approx_eq!(scl.relative_pitch_of(2).as_cents(), 50.0);
    /// assert_approx_eq!(scl.relative_pitch_of(3).as_cents(), 175.0);
    /// assert_approx_eq!(scl.relative_pitch_of(4).as_cents(), 150.0);
    /// assert_approx_eq!(scl.relative_pitch_of(5).as_cents(), 250.0); // 250.0 = next period + 100.0
    /// ```
    pub fn relative_pitch_of(&self, degree: i32) -> Ratio {
        let (num_periods, pitch_index) = self.num_periods_and_pitch_index_for_degree(degree);

        self.period
            .repeated(num_periods)
            .stretched_by(self.pitch_values[pitch_index].as_ratio())
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
    ///     .push_cents(175.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.period().as_cents(), 150.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(-1).as_cents(), -50.0); // -50.0 = previous period + 100.0
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(1).as_cents(), 25.0); // 25.0 = previous period + 175.0
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(2).as_cents(), 50.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(3).as_cents(), 100.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(4).as_cents(), 150.0);
    /// assert_approx_eq!(scl.sorted_relative_pitch_of(5).as_cents(), 175.0);
    /// ```
    pub fn sorted_relative_pitch_of(&self, degree: i32) -> Ratio {
        let (num_periods, pitch_index) = self.num_periods_and_pitch_index_for_degree(degree);

        self.period
            .repeated(num_periods)
            .stretched_by(self.sorted_pitch_values[pitch_index].reduced_ratio)
    }

    fn num_periods_and_pitch_index_for_degree(&self, degree: i32) -> (i32, usize) {
        if self.num_items() == 0 {
            return (0, 0);
        }

        let (num_periods, scale_index) = math::i32_dr_u(degree, self.num_items());
        (num_periods, usize::from(scale_index))
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
    ///     .push_cents(175.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.period().as_cents(), 150.0);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(0.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(10.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(20.0)).approx_value, -1); // 25.0 = previous period + 175.0
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(30.0)).approx_value, -1); // 25.0 = previous period + 175.0
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(40.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(50.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(60.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(70.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(80.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(90.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(100.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(110.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(120.0)).approx_value, 1);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(130.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(140.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(150.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(160.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(170.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(180.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(190.0)).approx_value, 6); // 200.0 = next period + 100.0
    /// assert_eq!(scl.find_by_relative_pitch(Ratio::from_cents(200.0)).approx_value, 6); // 200.0 = next period + 100.0
    /// ```
    pub fn find_by_relative_pitch(&self, relative_pitch: Ratio) -> Approximation<i32> {
        let approximation = self.find_by_relative_pitch_internal(relative_pitch);
        let (num_periods, sorted_pitch_index) = approximation.approx_value;
        let sorted_pitch_value = self.sorted_pitch_values[sorted_pitch_index];
        Approximation {
            approx_value: i32::try_from(sorted_pitch_value.pitch_index).unwrap()
                + (num_periods - sorted_pitch_value.num_wraparounds) * i32::from(self.num_items()),
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
    ///     .push_cents(175.0)
    ///     .push_cents(150.0)
    ///     .build().unwrap();
    ///
    /// assert_approx_eq!(scl.period().as_cents(), 150.0);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(0.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(10.0)).approx_value, 0);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(20.0)).approx_value, 1); // 25.0 = previous period + 175.0
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(30.0)).approx_value, 1); // 25.0 = previous period + 175.0
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(40.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(50.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(60.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(70.0)).approx_value, 2);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(80.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(90.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(100.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(110.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(120.0)).approx_value, 3);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(130.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(140.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(150.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(160.0)).approx_value, 4);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(170.0)).approx_value, 5);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(180.0)).approx_value, 5);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(190.0)).approx_value, 6);
    /// assert_eq!(scl.find_by_relative_pitch_sorted(Ratio::from_cents(200.0)).approx_value, 6); // 200.0 = next period + 50.0
    /// ```
    pub fn find_by_relative_pitch_sorted(&self, relative_pitch: Ratio) -> Approximation<i32> {
        let approximation = self.find_by_relative_pitch_internal(relative_pitch);
        let (num_periods, sorted_pitch_index) = approximation.approx_value;
        Approximation {
            approx_value: i32::try_from(sorted_pitch_index).unwrap()
                + num_periods * i32::from(self.num_items()),
            deviation: approximation.deviation,
        }
    }

    fn find_by_relative_pitch_internal(
        &self,
        relative_pitch: Ratio,
    ) -> Approximation<(i32, usize)> {
        if self.num_items() == 0 {
            return Approximation {
                approx_value: (0, 0),
                deviation: relative_pitch,
            };
        }

        let (num_periods, ratio_to_find) = if self.period.is_negligible() {
            (0, relative_pitch)
        } else {
            let pitch_as_octaves = relative_pitch.as_octaves();
            let period_as_octaves = self.period.as_octaves();
            (
                pitch_as_octaves.div_euclid(period_as_octaves) as i32,
                Ratio::from_octaves(pitch_as_octaves.rem_euclid(period_as_octaves)),
            )
        };

        let upper_index_in_sorted_pitch_list = self
            .sorted_pitch_values
            .binary_search_by(|sorted_pitch_value| {
                sorted_pitch_value.reduced_ratio.total_cmp(&ratio_to_find)
            })
            .unwrap_or_else(|inexact_match| inexact_match)
            .max(1)
            .min(self.sorted_pitch_values.len() - 1);
        let upper_pitch_value = self.sorted_pitch_values[upper_index_in_sorted_pitch_list];
        let upper_deviation = upper_pitch_value
            .reduced_ratio
            .deviation_from(ratio_to_find);

        let lower_index_in_sorted_pitch_list = upper_index_in_sorted_pitch_list - 1;
        let lower_pitch_value = self.sorted_pitch_values[lower_index_in_sorted_pitch_list];
        let lower_deviation = ratio_to_find.deviation_from(lower_pitch_value.reduced_ratio);

        // Prefer positive (i.e. lower) deviation if both deviations are equal within some tolerance interval.
        let tolerance_interval = Ratio::from_float(0.999999);

        let (sorted_pitch_index, deviation) =
            if lower_deviation.stretched_by(tolerance_interval) < upper_deviation {
                (lower_index_in_sorted_pitch_list, lower_deviation)
            } else {
                (upper_index_in_sorted_pitch_list, upper_deviation.inv())
            };

        Approximation {
            approx_value: (num_periods, sorted_pitch_index),
            deviation,
        }
    }

    /// Imports the given file in SCL format.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::scala::Scl;
    /// let scl_file = [
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
    pitch_values: Vec<PitchValue>,
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
        self.pitch_values.push(pitch_value);
        self
    }

    pub fn build(self) -> Result<Scl, SclBuildError> {
        let description = if let [_, single_pitch_value] = self.pitch_values.as_slice() {
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
        self,
        description: impl Into<String>,
    ) -> Result<Scl, SclBuildError> {
        let num_items_usize = self.pitch_values.len() - 1;
        let num_items = u16::try_from(num_items_usize).map_err(|_| SclBuildError::ScaleTooLarge)?;

        let period = self.pitch_values[num_items_usize].as_ratio();
        let period_in_octaves = period.as_octaves();

        let mut sorted_pitch_values: Vec<SortedPitchValue> = self
            .pitch_values
            .iter()
            .enumerate()
            .map(|(pitch_index, pitch_value)| {
                let ratio = pitch_value.as_ratio();

                if period.is_negligible() {
                    SortedPitchValue {
                        pitch_index,
                        reduced_ratio: ratio,
                        num_wraparounds: 0,
                    }
                } else {
                    let ratio_in_octave = ratio.as_octaves();
                    SortedPitchValue {
                        pitch_index,
                        reduced_ratio: Ratio::from_octaves(
                            ratio_in_octave.rem_euclid(period_in_octaves),
                        ),
                        num_wraparounds: ratio_in_octave.div_euclid(period_in_octaves) as i32,
                    }
                }
            })
            .collect::<Vec<_>>();

        // Ensure reduced ratios cover the entire search range, i.e. all values [1/1, .., period] are present.
        if period >= Ratio::default() {
            sorted_pitch_values[num_items_usize].reduced_ratio = period;
            sorted_pitch_values[num_items_usize].num_wraparounds = 0;
        } else {
            sorted_pitch_values[0].reduced_ratio = period.inv();
            sorted_pitch_values[0].num_wraparounds = 1;
        }

        sorted_pitch_values.sort_by(|a, b| a.reduced_ratio.total_cmp(&b.reduced_ratio));

        dedup_pitch_values_prefer_first_declared(&mut sorted_pitch_values);

        Ok(Scl {
            description: description.into(),
            period,
            num_items,
            pitch_values: self.pitch_values,
            sorted_pitch_values,
        })
    }
}

fn dedup_pitch_values_prefer_first_declared(sorted_pitch_values: &mut [SortedPitchValue]) {
    let mut possible_duplicate = sorted_pitch_values[0];
    for pitch_value in sorted_pitch_values {
        if pitch_value
            .reduced_ratio
            .deviation_from(possible_duplicate.reduced_ratio)
            .is_negligible()
        {
            *pitch_value = possible_duplicate;
        } else {
            possible_duplicate = *pitch_value;
        }
    }
}

/// Error reported when building an [`Scl`] fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SclBuildError {
    /// There are too many items in this scale.
    ///
    /// ```
    /// # use tune::scala::Scl;
    /// # use tune::scala::SclBuildError;
    /// // The number of items is below the threshold.
    /// let mut below = Scl::builder();
    /// for i in 0..65535 {
    ///     below = below.push_cents(f64::from(i));
    /// }
    /// assert!(below.build().is_ok());
    ///
    /// // The number of items is above the threshold.
    /// let mut above = Scl::builder();
    /// for i in 0..65536 {
    ///     above = above.push_cents(f64::from(i));
    /// }
    /// assert_eq!(above.build().unwrap_err(), SclBuildError::ScaleTooLarge);
    /// ```
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
            PitchValue::Cents(cents) => write!(f, "{cents:.3}"),
            PitchValue::Fraction(numer, Some(denom)) => write!(f, "{numer}/{denom}"),
            PitchValue::Fraction(numer, None) => write!(f, "{numer}"),
        }
    }
}

/// Format / [`Display`] wrapper created by [`Scl::export`].
pub struct SclExport<'a>(&'a Scl);

impl<'a> Display for SclExport<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let pitch_values_to_export = &self.0.pitch_values[1..];
        writeln!(f, "{}", self.0.description())?;
        writeln!(f, "{}", pitch_values_to_export.len())?;
        for pitch_value in pitch_values_to_export {
            writeln!(f, "{pitch_value}")?;
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
            formal_octave: None,
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

    pub fn range_iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = PianoKey> + ExactSizeIterator<Item = PianoKey> + 'static
    {
        self.range().start.keys_before(self.range().end)
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
        let key_degree = self.kbm_root.ref_key.num_keys_before(key);
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
    /// let input = [
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
    /// assert_eq!(kbm.kbm_root().ref_key.midi_number(), 69);
    /// assert_approx_eq!(kbm.kbm_root().ref_pitch.as_hz(), 432.0);
    /// assert_eq!(kbm.kbm_root().root_offset, -7);
    /// assert_eq!(kbm.range().start.midi_number(), 10);
    /// assert_eq!(kbm.range().end.midi_number(), 100);
    /// assert_eq!(kbm.formal_octave(), 17);
    /// assert_eq!(kbm.num_items(), 6);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(69)), Some(0));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(70)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(71)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(72)), Some(4));
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(73)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(74)), None);
    /// assert_eq!(kbm.scale_degree_of(PianoKey::from_midi_number(75)), Some(17));
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
    /// # use tune::scala::Kbm;
    /// # use tune::scala::KbmRoot;
    /// # use tune::pitch::Pitch;
    /// let mut kbm_root = KbmRoot {
    ///     ref_key: PianoKey::from_midi_number(69),
    ///     ref_pitch: Pitch::from_hz(432.0),
    ///     root_offset: -9,
    /// };
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
    /// If a [`Kbm`] surrounding is used with the first entry being *n*, `ref_key` maps to scale degree *n*.
    pub ref_key: PianoKey,

    /// A [`Pitch`] that is guaranteed to be present in a [`Tuning`] but which might be skipped in the [`KeyboardMapping`] spanned by the [`Kbm`] surrounding.
    pub ref_pitch: Pitch,

    /// The amount by which the scale's root is displaced wrt. to `ref_key`.
    pub root_offset: i32,
}

impl KbmRoot {
    /// Shifts the `ref_key` of a scale by `num_degrees` correcting the scale's vertical location.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::key::PianoKey;
    /// # use tune::pitch::Pitch;
    /// # use tune::scala::KbmRoot;
    /// let kbm_root =  KbmRoot {
    ///     ref_key: PianoKey::from_midi_number(67),
    ///     ref_pitch: Pitch::from_hz(432.0),
    ///     root_offset: -2,
    /// };
    ///
    /// let shifted = kbm_root.shift_ref_key_by(-7);
    ///
    /// assert_eq!(shifted.ref_key, PianoKey::from_midi_number(60));
    /// assert_approx_eq!(shifted.ref_pitch.as_hz(), 288.325409);
    /// assert_eq!(shifted.root_offset, -2);
    /// ```
    pub fn shift_ref_key_by(self, num_degrees: i32) -> Self {
        Self {
            ref_key: self.ref_key.plus_steps(num_degrees),
            ref_pitch: self.ref_pitch * Ratio::from_semitones(num_degrees),
            root_offset: self.root_offset,
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
            ref_key: note.note().as_piano_key(),
            ref_pitch: note.pitch(),
            root_offset: 0,
        }
    }
}

impl FromStr for KbmRoot {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let [note, pitch] = parse::split_balanced(s, '@').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{note}': Must be an integer"))?;
            let pitch: Pitch = pitch
                .parse()
                .map_err(|e| format!("Invalid pitch '{pitch}': {e}"))?;
            Ok(Note::from_midi_number(midi_number).at_pitch(pitch).into())
        } else if let [note, delta] = parse::split_balanced(s, '+').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{note}': Must be an integer"))?;
            let delta = delta
                .parse()
                .map_err(|e| format!("Invalid delta '{delta}': {e}"))?;
            Ok(Note::from_midi_number(midi_number)
                .alter_pitch_by(delta)
                .into())
        } else if let [note, delta] = parse::split_balanced(s, '-').as_slice() {
            let midi_number = note
                .parse::<i32>()
                .map_err(|_| format!("Invalid note '{note}': Must be an integer"))?;
            let delta = delta
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid delta '{delta}': {e}"))?;
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
    formal_octave: Option<i16>,
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
        self.formal_octave = Some(formal_octave);
        self
    }

    pub fn build(self) -> Result<Kbm, KbmBuildError> {
        if !self.key_mapping.is_empty() && self.formal_octave.is_none() {
            return Err(KbmBuildError::FormalOctaveMissing);
        }
        Ok(Kbm {
            kbm_root: self.kbm_root,
            range: self.range,
            num_items: u16::try_from(self.key_mapping.len())
                .map_err(|_| KbmBuildError::MappingTooLarge)?,
            key_mapping: self.key_mapping,
            formal_octave: self.formal_octave.unwrap_or(0),
        })
    }
}

/// Error reported when building a [`Kbm`] fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KbmBuildError {
    /// No formal octave parameter has been set.
    ///
    /// The formal octave parameter is mandatory if at least one key is pushed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// # use tune::scala::KbmBuildError;
    /// // No key pushed. The mapping is linear and the formal octave parameter is optional.
    /// let optional = Kbm::builder(Note::from_midi_number(0));
    /// assert!(optional.build().is_ok());
    ///
    /// // At least one key pushed. The formal octave parameter is mandatory.
    /// let mandatory = Kbm::builder(Note::from_midi_number(0)).push_mapped_key(0);
    /// assert_eq!(mandatory.build().unwrap_err(), KbmBuildError::FormalOctaveMissing);
    /// ```
    FormalOctaveMissing,

    /// There are too many items in this mapping.
    ///
    /// ```
    /// # use tune::note::Note;
    /// # use tune::scala::Kbm;
    /// # use tune::scala::KbmBuildError;
    /// // The number of items is below the threshold.
    /// let mut below = Kbm::builder(Note::from_midi_number(62)).formal_octave(0);
    /// for _ in 0..65535 {
    ///     below = below.push_mapped_key(0);
    /// }
    /// assert!(below.build().is_ok());
    ///
    /// // The number of items is above the threshold.
    /// let mut above = Kbm::builder(Note::from_midi_number(62)).formal_octave(0);
    /// for _ in 0..65536 {
    ///     above = above.push_mapped_key(0);
    /// }
    /// assert_eq!(above.build().unwrap_err(), KbmBuildError::MappingTooLarge);
    /// ```
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
        writeln!(
            f,
            "{}",
            kbm_root.ref_key.midi_number() + kbm_root.root_offset
        )?;
        writeln!(f, "{}", kbm_root.ref_key.midi_number())?;
        writeln!(f, "{:.3}", kbm_root.ref_pitch.as_hz())?;
        writeln!(f, "{}", self.0.formal_octave())?;
        for degree in &self.0.key_mapping {
            match degree {
                Some(degree) => {
                    writeln!(f, "{degree}")?;
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
    kbm.ref_pitch / scl.relative_pitch_of(-kbm.root_offset)
}

impl<S: Borrow<Scl>, K: Borrow<KbmRoot>> Tuning<PianoKey> for (S, K) {
    fn pitch_of(&self, key: PianoKey) -> Pitch {
        let degree = self.1.borrow().ref_key.num_keys_before(key);
        self.pitch_of(degree)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<PianoKey> {
        let degree: Approximation<i32> = self.find_by_pitch(pitch);
        let key =
            PianoKey::from_midi_number(self.1.borrow().ref_key.midi_number() + degree.approx_value);
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
        let origin = self.1.borrow().kbm_root().ref_key;
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

    let mut pitch_values = vec![period];

    let pos_range = (1..=num_pos_generations).map(f64::from);
    let neg_range = (1..=num_neg_generations).map(f64::from).map(Neg::neg);
    for generation in pos_range.chain(neg_range) {
        let unbounded_note = generation * generator_in_cents;
        let bounded_note = unbounded_note.rem_euclid(period_in_cents);
        pitch_values.push(Ratio::from_cents(bounded_note));
    }

    pitch_values.sort_by(|a, b| a.total_cmp(b));

    let mut builder = Scl::builder();
    for pitch_value in pitch_values {
        builder = builder.push_ratio(pitch_value)
    }

    let description = description.into().unwrap_or_else(|| {
        format!(
            "{num_pos_generations} positive and {num_neg_generations} negative generations of generator {generator} ({generator:#}) with period {period}"
        )
    });
    builder.build_with_description(description)
}

/// Creates a harmonics or subharmonics scale.
///
/// # Examples
///
/// ## Create harmonics segment scale
///
///
/// ```
/// # use tune::scala;
/// # use tune::scala::SegmentType;
/// let segment_start = 9;
/// let segment_size = 7;
///
/// let harmonics = scala::create_harmonics_scale(
///     None,
///     SegmentType::Otonal,
///     segment_start,
///     segment_size,
///     None,
/// ).unwrap();
///
/// assert_eq!(
///     format!("{}", harmonics.export()).lines().collect::<Vec<_>>(),
///     ["JI scale 9:10:11:12:13:14:15:16",
///      "7", "10/9", "11/9", "12/9", "13/9", "14/9", "15/9", "16/9"]
/// );
///
/// let subharmonics = scala::create_harmonics_scale(
///     None,
///     SegmentType::Utonal,
///     segment_start,
///     segment_size,
///     None,
/// ).unwrap();
///
/// assert_eq!(
///     format!("{}", subharmonics.export()).lines().collect::<Vec<_>>(),
///     ["JI scale 16/(16:15:14:13:12:11:10:9)",
///      "7", "16/15", "16/14", "16/13", "16/12", "16/11", "16/10", "16/9"]
/// );
/// ```
///
/// ## Create NEJI scale
///
/// ```
/// # use tune::scala;
/// # use tune::scala::SegmentType;
/// let primodal_limit = 27;
/// let neji_divisions = 12;
///
/// let harmonics = scala::create_harmonics_scale(
///     None,
///     SegmentType::Otonal,
///     primodal_limit,
///     primodal_limit,
///     neji_divisions,
/// ).unwrap();
///
/// assert_eq!(
///     format!("{}", harmonics.export()).lines().collect::<Vec<_>>(),
///     ["JI scale 27:29:30:32:34:36:38:40:43:45:48:51:54",
///      "12", "29/27", "30/27", "32/27", "34/27", "36/27", "38/27",
///      "40/27", "43/27", "45/27", "48/27", "51/27", "54/27"]
/// );
///
/// let subharmonics = scala::create_harmonics_scale(
///     None,
///     SegmentType::Utonal,
///     primodal_limit,
///     primodal_limit,
///     neji_divisions,
/// ).unwrap();
///
/// assert_eq!(
///     format!("{}", subharmonics.export()).lines().collect::<Vec<_>>(),
///     ["JI scale 54/(54:51:48:45:43:40:38:36:34:32:30:29:27)",
///      "12", "54/51", "54/48", "54/45", "54/43", "54/40", "54/38",
///      "54/36", "54/34", "54/32", "54/30", "54/29", "54/27"]
/// );
/// ```
pub fn create_harmonics_scale(
    description: impl Into<Option<String>>,
    segment_type: SegmentType,
    segment_start: u16,
    segment_size: u16,
    neji_divisions: impl Into<Option<u16>>,
) -> Result<Scl, SclBuildError> {
    let mut builder = Scl::builder();
    let mut builtin_description = "JI scale ".to_string();

    if let Some(neji_divisions) = neji_divisions.into() {
        let equivalence_interval =
            (f64::from(segment_start) + f64::from(segment_size)) / f64::from(segment_start);
        let step_size =
            Ratio::from_float(equivalence_interval).divided_into_equal_steps(neji_divisions);

        match segment_type {
            SegmentType::Otonal => {
                write!(builtin_description, "{segment_start}").unwrap();
                for division in 0..neji_divisions {
                    let scale_step_to_approximate = step_size.repeated(u32::from(division) + 1);
                    let harmonic_to_approximate =
                        scale_step_to_approximate.as_float() * f64::from(segment_start);
                    let lowest_candidate = harmonic_to_approximate.floor();
                    let highest_candidate = harmonic_to_approximate.ceil();
                    let harmonic = if harmonic_to_approximate / lowest_candidate
                        < highest_candidate / harmonic_to_approximate
                    {
                        lowest_candidate
                    } else {
                        highest_candidate
                    } as u32;
                    builder = builder.push_fraction(harmonic, u32::from(segment_start));
                    write!(builtin_description, ":{harmonic}").unwrap();
                }
            }
            SegmentType::Utonal => {
                let denom_end = u32::from(segment_start) + u32::from(segment_size);

                write!(builtin_description, "{denom_end}/({denom_end}").unwrap();
                for division in 0..neji_divisions {
                    let scale_step_to_approximate = step_size.repeated(u32::from(division) + 1);
                    let harmonic_to_approximate =
                        f64::from(denom_end) / scale_step_to_approximate.as_float();
                    let lowest_candidate = harmonic_to_approximate.floor();
                    let highest_candidate = harmonic_to_approximate.ceil();
                    let harmonic = if harmonic_to_approximate / lowest_candidate
                        < highest_candidate / harmonic_to_approximate
                    {
                        lowest_candidate
                    } else {
                        highest_candidate
                    } as u32;
                    builder = builder.push_fraction(denom_end, harmonic);
                    write!(builtin_description, ":{harmonic}").unwrap();
                }
                write!(builtin_description, ")").unwrap();
            }
        }
    } else {
        match segment_type {
            SegmentType::Otonal => {
                let numer_start = u32::from(segment_start) + 1;
                let numer_end = numer_start + u32::from(segment_size);

                write!(builtin_description, "{segment_start}").unwrap();
                for numer in numer_start..numer_end {
                    builder = builder.push_fraction(numer, u32::from(segment_start));
                    write!(builtin_description, ":{numer}").unwrap();
                }
            }
            SegmentType::Utonal => {
                let denom_start = u32::from(segment_start);
                let denom_end = denom_start + u32::from(segment_size);

                write!(builtin_description, "{denom_end}/({denom_end}").unwrap();
                for denom in (denom_start..denom_end).rev() {
                    builder = builder.push_fraction(denom_end, denom);
                    write!(builtin_description, ":{denom}").unwrap();
                }
                write!(builtin_description, ")").unwrap();
            }
        }
    }

    builder.build_with_description(description.into().unwrap_or(builtin_description))
}

/// Type of harmonic series segment to use.
#[derive(Copy, Clone, Debug)]
pub enum SegmentType {
    /// Harmonic segment of kind `n:n+1:n+2:..`.
    Otonal,

    /// Harmonic segment of kind `n/(n:n-1:n-2:..)`.
    Utonal,
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::{key::PianoKey, note::NoteLetter, pitch::Pitched};

    use super::*;

    #[test]
    fn build_empty_scale() {
        let empty_scale = Scl::builder().build().unwrap();

        AssertScale(empty_scale, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(67, 440.0)
            .maps_key_to_pitch(68, 440.0)
            .maps_key_to_pitch(69, 440.0)
            .maps_key_to_pitch(70, 440.0)
            .maps_key_to_pitch(71, 440.0)
            .maps_frequency_to_key_and_deviation(330.0, 69, 330.0 / 440.0)
            .maps_frequency_to_key_and_deviation(339.0, 69, 339.0 / 440.0)
            .maps_frequency_to_key_and_deviation(440.0, 69, 440.0 / 440.0)
            .maps_frequency_to_key_and_deviation(441.0, 69, 441.0 / 440.0)
            .maps_frequency_to_key_and_deviation(550.0, 69, 550.0 / 440.0)
            .exports_lines(&["Custom scale", "0"]);
    }

    #[test]
    fn build_non_monotonic_scale() {
        let non_monotonic_scale = Scl::builder()
            .push_fraction(7, 5)
            .push_fraction(9, 5)
            .push_fraction(8, 5)
            .push_fraction(6, 5)
            .push_fraction(10, 5)
            .build()
            .unwrap();

        assert_approx_eq!(non_monotonic_scale.period().as_octaves(), 1.0);

        AssertScale(
            non_monotonic_scale,
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
    fn build_overshooting_scale() {
        let overshooting_scale = Scl::builder()
            .push_fraction(7, 5)
            .push_fraction(3, 5)
            .push_fraction(9, 5)
            .push_fraction(11, 5)
            .push_fraction(10, 5)
            .build()
            .unwrap();

        assert_approx_eq!(overshooting_scale.period().as_octaves(), 1.0);

        AssertScale(
            overshooting_scale,
            NoteLetter::G
                .in_octave(2)
                .at_pitch(Pitch::from_hz(100.0))
                .into(),
        )
        .maps_key_to_pitch(42, 110.0)
        .maps_key_to_pitch(43, 100.0)
        .maps_key_to_pitch(44, 140.0)
        .maps_key_to_pitch(45, 60.0)
        .maps_key_to_pitch(46, 180.0)
        .maps_key_to_pitch(47, 220.0)
        .maps_key_to_pitch(48, 200.0)
        .maps_key_to_pitch(49, 280.0)
        .maps_key_to_pitch(50, 120.0)
        .maps_frequency_to_key_and_deviation(99.0, 43, 99.0 / 100.0)
        .maps_frequency_to_key_and_deviation(101.0, 43, 101.0 / 100.0)
        .maps_frequency_to_key_and_deviation(109.0, 42, 109.0 / 110.0)
        .maps_frequency_to_key_and_deviation(111.0, 42, 111.0 / 110.0)
        .maps_frequency_to_key_and_deviation(119.0, 50, 119.0 / 120.0)
        .maps_frequency_to_key_and_deviation(121.0, 50, 121.0 / 120.0)
        .maps_frequency_to_key_and_deviation(129.0, 50, 129.0 / 120.0)
        .maps_frequency_to_key_and_deviation(131.0, 44, 131.0 / 140.0)
        .maps_frequency_to_key_and_deviation(139.0, 44, 139.0 / 140.0)
        .maps_frequency_to_key_and_deviation(141.0, 44, 141.0 / 140.0)
        .maps_frequency_to_key_and_deviation(149.0, 44, 149.0 / 140.0)
        .maps_frequency_to_key_and_deviation(151.0, 44, 151.0 / 140.0)
        .maps_frequency_to_key_and_deviation(159.0, 46, 159.0 / 180.0)
        .maps_frequency_to_key_and_deviation(161.0, 46, 161.0 / 180.0)
        .maps_frequency_to_key_and_deviation(169.0, 46, 169.0 / 180.0)
        .maps_frequency_to_key_and_deviation(171.0, 46, 171.0 / 180.0)
        .maps_frequency_to_key_and_deviation(179.0, 46, 179.0 / 180.0)
        .maps_frequency_to_key_and_deviation(181.0, 46, 181.0 / 180.0)
        .maps_frequency_to_key_and_deviation(189.0, 46, 189.0 / 180.0)
        .maps_frequency_to_key_and_deviation(191.0, 48, 191.0 / 200.0)
        .maps_frequency_to_key_and_deviation(199.0, 48, 199.0 / 200.0)
        .maps_frequency_to_key_and_deviation(201.0, 48, 201.0 / 200.0)
        .exports_lines(&["Custom scale", "5", "7/5", "3/5", "9/5", "11/5", "10/5"]);
    }

    #[test]
    fn build_non_periodic_scale() {
        let non_periodic_scale = Scl::builder()
            .push_fraction(3, 2)
            .push_fraction(5, 4)
            .push_fraction(3, 4)
            .push_fraction(1, 1)
            .build()
            .unwrap();

        AssertScale(non_periodic_scale, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(64, 330.0)
            .maps_key_to_pitch(65, 440.0)
            .maps_key_to_pitch(66, 660.0)
            .maps_key_to_pitch(67, 550.0)
            .maps_key_to_pitch(68, 330.0)
            .maps_key_to_pitch(69, 440.0)
            .maps_key_to_pitch(70, 660.0)
            .maps_key_to_pitch(71, 550.0)
            .maps_key_to_pitch(72, 330.0)
            .maps_key_to_pitch(73, 440.0)
            .maps_key_to_pitch(74, 660.0)
            .maps_frequency_to_key_and_deviation(220.0, 72, 220.0 / 330.0)
            .maps_frequency_to_key_and_deviation(329.0, 72, 329.0 / 330.0)
            .maps_frequency_to_key_and_deviation(330.0, 72, 330.0 / 330.0)
            .maps_frequency_to_key_and_deviation(331.0, 72, 331.0 / 330.0)
            .maps_frequency_to_key_and_deviation(439.0, 69, 439.0 / 440.0)
            .maps_frequency_to_key_and_deviation(440.0, 69, 440.0 / 440.0)
            .maps_frequency_to_key_and_deviation(441.0, 69, 441.0 / 440.0)
            .maps_frequency_to_key_and_deviation(549.0, 71, 549.0 / 550.0)
            .maps_frequency_to_key_and_deviation(550.0, 71, 550.0 / 550.0)
            .maps_frequency_to_key_and_deviation(551.0, 71, 551.0 / 550.0)
            .maps_frequency_to_key_and_deviation(659.0, 70, 659.0 / 660.0)
            .maps_frequency_to_key_and_deviation(660.0, 70, 660.0 / 660.0)
            .maps_frequency_to_key_and_deviation(661.0, 70, 661.0 / 660.0)
            .maps_frequency_to_key_and_deviation(770.0, 70, 770.0 / 660.0)
            .exports_lines(&["Custom scale", "4", "3/2", "5/4", "3/4", "1/1"]);
    }

    #[test]
    fn build_negative_period_scale() {
        let negative_period_scale = Scl::builder().push_cents(-100.0).build().unwrap();

        AssertScale(negative_period_scale, NoteLetter::A.in_octave(4).into())
            .maps_key_to_pitch(67, 493.883_301)
            .maps_key_to_pitch(68, 466.163_762)
            .maps_key_to_pitch(69, 440.000_000)
            .maps_key_to_pitch(70, 415.304_698)
            .maps_key_to_pitch(71, 391.995_436)
            .maps_frequency_to_key_and_deviation(392.0, 71, 392.0 / 391.995_436)
            .maps_frequency_to_key_and_deviation(391.0, 71, 391.0 / 391.995_436)
            .maps_frequency_to_key_and_deviation(416.0, 70, 416.0 / 415.304_698)
            .maps_frequency_to_key_and_deviation(415.0, 70, 415.0 / 415.304_698)
            .maps_frequency_to_key_and_deviation(439.0, 69, 439.0 / 440.0)
            .maps_frequency_to_key_and_deviation(441.0, 69, 441.0 / 440.0)
            .maps_frequency_to_key_and_deviation(466.0, 68, 466.0 / 466.163_762)
            .maps_frequency_to_key_and_deviation(467.0, 68, 467.0 / 466.163_762)
            .maps_frequency_to_key_and_deviation(493.0, 67, 493.0 / 493.883_301)
            .maps_frequency_to_key_and_deviation(494.0, 67, 494.0 / 493.883_301)
            .exports_lines(&["equal steps of -100.0c (-12.00-EDO)", "1", "-100.000"]);
    }

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
        let harmonics = create_harmonics_scale(None, SegmentType::Otonal, 8, 8, None).unwrap();

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
                "JI scale 8:9:10:11:12:13:14:15:16",
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
    fn best_fit_stability_prefer_positive_deviation() {
        let equal_tempered_scale = Scl::builder().push_cents(100.0).build().unwrap();

        let scale_root = NoteLetter::A.in_octave(4);

        AssertScale(equal_tempered_scale, scale_root.into())
            .maps_frequency_to_key_and_deviation(
                scale_root
                    .alter_pitch_by(Ratio::from_cents(-50.0))
                    .pitch()
                    .as_hz(),
                68,
                Ratio::from_cents(50.0).as_float(),
            )
            .maps_frequency_to_key_and_deviation(
                scale_root
                    .alter_pitch_by(Ratio::from_cents(0.0))
                    .pitch()
                    .as_hz(),
                69,
                Ratio::from_cents(0.0).as_float(),
            )
            .maps_frequency_to_key_and_deviation(
                scale_root
                    .alter_pitch_by(Ratio::from_cents(50.0))
                    .pitch()
                    .as_hz(),
                69,
                Ratio::from_cents(50.0).as_float(),
            );
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
            self
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
