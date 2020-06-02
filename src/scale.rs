//! Scale format according to [http://www.huygens-fokker.org/scala/scl_format.html](http://www.huygens-fokker.org/scala/scl_format.html).

use crate::key_map::KeyMap;
use crate::math;
use crate::pitch::Pitch;
use crate::{
    key::PianoKey,
    ratio::Ratio,
    tuning::{Approximation, Tuning},
};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::Neg;

#[derive(Clone, Debug)]
pub struct Scale {
    description: String,
    period: Ratio,
    pitch_values: Vec<PitchValue>,
}

impl Scale {
    pub fn with_name<S: Into<String>>(name: S) -> ScaleBuilder {
        ScaleBuilder(Scale {
            description: name.into(),
            period: Ratio::default(),
            pitch_values: Vec::new(),
        })
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn period(&self) -> Ratio {
        self.period
    }

    pub fn size(&self) -> usize {
        self.pitch_values.len()
    }

    pub fn with_key_map<'a, 'b>(&'a self, key_map: &'b KeyMap) -> ScaleWithKeyMap<'a, 'b> {
        (self, key_map)
    }

    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// # use tune::scale;
    /// assert_approx_eq!(scale::create_equal_temperament_scale("1:12:2".parse().unwrap()).normal_pitch(0).as_cents(), 0.0);
    /// ```
    pub fn normal_pitch(&self, degree: i32) -> Ratio {
        let (num_periods, phase) = math::i32_dr_u32(degree, self.size() as u32);
        let phase_factor = if phase == 0 {
            Ratio::default()
        } else {
            self.pitch_values[(phase - 1) as usize].as_ratio()
        };
        Ratio::from_float(self.period.as_float().powi(num_periods) * phase_factor.as_float())
    }

    pub fn as_scl(&self) -> FormattedScale<'_> {
        FormattedScale(self)
    }
}

pub type ScaleWithKeyMap<'a, 'b> = (&'a Scale, &'b KeyMap);

impl Tuning<PianoKey> for ScaleWithKeyMap<'_, '_> {
    fn pitch_of(&self, key: PianoKey) -> Pitch {
        let degree = self.1.root_key.num_keys_before(key);
        self.pitch_of(degree)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<PianoKey> {
        let degree: Approximation<i32> = self.find_by_pitch(pitch);
        let key = PianoKey::from_midi_number(self.1.root_key.midi_number() + degree.approx_value);
        Approximation {
            approx_value: key,
            deviation: degree.deviation,
        }
    }
}

impl Tuning<i32> for ScaleWithKeyMap<'_, '_> {
    fn pitch_of(&self, degree: i32) -> Pitch {
        let scale = self.0;
        let key_map = self.1;
        let reference_pitch =
            scale.normal_pitch(key_map.root_key.num_keys_before(key_map.ref_pitch.key()));
        let normalized_pitch = scale.normal_pitch(degree);
        key_map.ref_pitch.pitch() / reference_pitch * normalized_pitch
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<i32> {
        let scale = self.0;

        let root_pitch = self.pitch_of(0);
        let total_ratio = Ratio::between_pitches(root_pitch, pitch);

        let num_periods = total_ratio
            .as_octaves()
            .div_euclid(scale.period.as_octaves());

        let ratio_to_find = Ratio::from_octaves(
            total_ratio
                .as_octaves()
                .rem_euclid(scale.period.as_octaves()),
        );

        let mut pitch_index = scale
            .pitch_values
            .binary_search_by(|probe| probe.as_ratio().partial_cmp(&ratio_to_find).unwrap())
            .unwrap_or_else(|inexact_match| inexact_match);

        // From a mathematical perspective, binary_search should always return an index smaller than the scale size.
        // However, since floating-point arithmetic is imprecise this cannot be guaranteed.
        if pitch_index == scale.pitch_values.len() {
            pitch_index -= 1;
        }

        let lower_ratio = if pitch_index == 0 {
            Ratio::default()
        } else {
            scale.pitch_values[pitch_index - 1].as_ratio()
        };
        let upper_ratio = scale.pitch_values[pitch_index].as_ratio();

        let (lower_deviation, upper_deviation) = (
            Ratio::from_float(ratio_to_find.as_float() / lower_ratio.as_float()),
            Ratio::from_float(upper_ratio.as_float() / ratio_to_find.as_float()),
        );

        if lower_deviation < upper_deviation {
            Approximation {
                approx_value: pitch_index as i32 + num_periods as i32 * scale.size() as i32,
                deviation: lower_deviation,
            }
        } else {
            Approximation {
                approx_value: (pitch_index + 1) as i32 + num_periods as i32 * scale.size() as i32,
                deviation: upper_deviation.inv(),
            }
        }
    }
}

pub struct ScaleBuilder(Scale);

impl ScaleBuilder {
    pub fn push_ratio(&mut self, ratio: Ratio) {
        self.push_cents(ratio.as_cents());
    }

    pub fn push_float(&mut self, float_value: f64) {
        self.push_ratio(Ratio::from_float(float_value));
    }

    pub fn push_cents(&mut self, cents_value: f64) {
        self.push_pitch_value(PitchValue::Cents(cents_value));
    }

    pub fn push_fraction(&mut self, numer: u32, denom: u32) {
        self.push_pitch_value(PitchValue::Fraction(numer, denom));
    }

    fn push_pitch_value(&mut self, pitch_value: PitchValue) {
        assert!(
            pitch_value.as_ratio() > self.0.period,
            "Scale must be strictly increasing"
        );

        self.0.pitch_values.push(pitch_value);
        self.0.period = pitch_value.as_ratio();
    }

    pub fn build(self) -> Scale {
        assert!(!self.0.pitch_values.is_empty(), "Scale must be non-empty");

        self.0
    }
}

pub struct FormattedScale<'a>(&'a Scale);

impl<'a> Display for FormattedScale<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "{}", self.0.description())?;
        writeln!(f, "{}", self.0.pitch_values.len())?;
        for pitch_value in &self.0.pitch_values {
            writeln!(f, "{}", pitch_value)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
enum PitchValue {
    Cents(f64),
    Fraction(u32, u32),
}

impl PitchValue {
    fn as_ratio(self) -> Ratio {
        match self {
            PitchValue::Cents(cents_value) => Ratio::from_cents(cents_value),
            PitchValue::Fraction(numer, denom) => {
                Ratio::from_float(f64::from(numer) / f64::from(denom))
            }
        }
    }
}

impl Display for PitchValue {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            PitchValue::Cents(cents) => write!(f, "{:.3}", cents),
            PitchValue::Fraction(numer, denom) => write!(f, "{}/{}", numer, denom),
        }
    }
}

pub fn create_equal_temperament_scale(step_size: Ratio) -> Scale {
    let mut scale = Scale::with_name(format!(
        "equal steps of {:#} ({:.2}-EDO)",
        step_size,
        1.0 / step_size.as_octaves()
    ));
    scale.push_ratio(step_size);
    scale.build()
}

pub fn create_rank2_temperament_scale(
    generator: Ratio,
    num_pos_generations: u16,
    num_neg_generations: u16,
    period: Ratio,
) -> Scale {
    assert!(
        period.as_float() > 1.0,
        "Ratio must be greater than 1 but was {}",
        period
    );

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

    pitch_values.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("Comparison yielded an invalid result")
    });

    let mut scale = Scale::with_name(format!(
        "{0} positive and {1} negative generations of generator {2} ({2:#}) with period {3}",
        num_pos_generations, num_neg_generations, generator, period
    ));
    for pitch_value in pitch_values {
        scale.push_ratio(pitch_value)
    }

    scale.build()
}

pub fn create_harmonics_scale(
    lowest_harmonic: u32,
    number_of_notes: u32,
    subharmonics: bool,
) -> Scale {
    assert!(
        lowest_harmonic > 0,
        "Lowest harmonic must be greater than 0 but was {}",
        lowest_harmonic
    );

    let debug_text = if subharmonics {
        "subharmonics"
    } else {
        "harmonics"
    };

    let mut scale = Scale::with_name(format!(
        "{} {} starting with {}",
        number_of_notes, debug_text, lowest_harmonic
    ));
    let highest_harmonic = lowest_harmonic + number_of_notes;
    if subharmonics {
        for harmonic in (lowest_harmonic..highest_harmonic).rev() {
            scale.push_fraction(highest_harmonic, harmonic);
        }
    } else {
        for harmonic in lowest_harmonic..highest_harmonic {
            scale.push_fraction(harmonic + 1, lowest_harmonic);
        }
    }

    scale.build()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{key::PianoKey, note::NoteLetter};
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn equal_temperament_scale_correctness() {
        let bohlen_pierce = create_equal_temperament_scale("1:13:3".parse().unwrap());

        assert_eq!(bohlen_pierce.size(), 1);
        assert_approx_eq!(bohlen_pierce.period().as_cents(), 146.304_231);

        AssertScale(bohlen_pierce.with_key_map(&KeyMap::root_at(NoteLetter::A.in_octave(4))))
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
        let pythagorean_major =
            create_rank2_temperament_scale(Ratio::from_float(1.5), 5, 1, Ratio::from_octaves(1.0));

        assert_eq!(pythagorean_major.size(), 7);
        assert_approx_eq!(pythagorean_major.period().as_octaves(), 1.0);

        AssertScale(pythagorean_major.with_key_map(&KeyMap::root_at(NoteLetter::A.in_octave(4))))
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
        let harmonics = create_harmonics_scale(8, 8, false);

        assert_eq!(harmonics.size(), 8);
        assert_approx_eq!(harmonics.period().as_float(), 2.0);

        AssertScale(harmonics.with_key_map(&KeyMap::root_at(NoteLetter::A.in_octave(4))))
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
    fn best_fit_correctness() {
        let harmonics = create_harmonics_scale(8, 8, false);
        AssertScale(harmonics.with_key_map(&KeyMap::root_at(NoteLetter::A.in_octave(4))))
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

    struct AssertScale<'a, 'b>(ScaleWithKeyMap<'a, 'b>);

    impl AssertScale<'_, '_> {
        fn maps_key_to_pitch(&self, midi_number: i32, expected_pitch_hz: f64) -> &Self {
            assert_approx_eq!(
                self.0
                    .pitch_of(PianoKey::from_midi_number(midi_number))
                    .as_hz(),
                expected_pitch_hz
            );
            &self
        }

        fn exports_lines(&self, expected_lines: &[&str]) -> &Self {
            let as_string = (self.0).0.as_scl().to_string();
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
            let approximation = Pitch::from_hz(freq_hz).find_in::<PianoKey, _>(self.0);
            assert_eq!(
                approximation.approx_value,
                PianoKey::from_midi_number(midi_number)
            );
            assert_approx_eq!(approximation.deviation.as_float(), deviation_as_float);
            self
        }
    }
}
