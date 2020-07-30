//! An implementation of [Scala](http://www.huygens-fokker.org/scala/)'s scale format.

use crate::math;
use crate::pitch::{Pitch, ReferencePitch};
use crate::{
    key::PianoKey,
    note::PitchedNote,
    ratio::Ratio,
    tuning::{Approximation, Tuning},
};
use io::{BufReader, Read};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::{
    borrow::Borrow,
    io::{self, BufRead},
    mem,
    ops::Neg,
};

/// Scale format according to [http://www.huygens-fokker.org/scala/scl_format.html](http://www.huygens-fokker.org/scala/scl_format.html).
///
/// The [`Scl`] format describes a periodic scale in *relative* pitches. You can access those pitches using [`Scl::relative_pitch_of`].
/// To retrieve *absolute* [`Pitch`] information, you need to pair the [`Scl`] struct with a [`Kbm`] struct (see implementations of the [`Tuning`] trait for more info).
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::key::PianoKey;
/// # use tune::note::Note;
/// # use tune::ratio::Ratio;
/// # use tune::pitch::Pitch;
/// # use tune::scala;
/// # use tune::scala::Kbm;
/// use tune::tuning::Tuning;
///
/// let scl = scala::create_harmonics_scale(8, 8, false);
/// let kbm = Kbm::root_at(Note::from_midi_number(43).at_pitch(Pitch::from_hz(100.0)));
/// let tuning = (scl, kbm);
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(43)).as_hz(), 100.0);
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(44)).as_hz(), 112.5);
/// assert_approx_eq!(tuning.pitch_of(PianoKey::from_midi_number(45)).as_hz(), 125.0);
/// ```
#[derive(Clone, Debug)]
pub struct Scl {
    description: String,
    period: Ratio,
    pitch_values: Vec<PitchValue>,
}

impl Scl {
    pub fn with_name<S: Into<String>>(name: S) -> SclBuilder {
        SclBuilder(
            Scl {
                description: name.into(),
                period: Ratio::default(),
                pitch_values: Vec::new(),
            },
            true,
        )
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

    /// Retrieves relative pitches without requiring any [`Kbm`] reference.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// # use tune::scala;
    /// let scl = scala::create_equal_temperament_scale("1:24:2".parse().unwrap());
    /// assert_approx_eq!(scl.relative_pitch_of(0).as_cents(), 0.0);
    /// assert_approx_eq!(scl.relative_pitch_of(7).as_cents(), 350.0);
    /// ```
    pub fn relative_pitch_of(&self, degree: i32) -> Ratio {
        let (num_periods, phase) = math::i32_dr_u32(degree, self.size() as u32);
        let phase_factor = if phase == 0 {
            Ratio::default()
        } else {
            self.pitch_values[(phase - 1) as usize].as_ratio()
        };
        self.period.repeated(num_periods).stretched_by(phase_factor)
    }

    pub fn import(reader: impl Read) -> Result<Self, SclImportError> {
        let mut scl_importer = SclImporter {
            state: ParserState::ExpectingDescription,
        };

        for (line_number, line) in BufReader::new(reader).lines().enumerate() {
            let line = line?;
            let trimmed = line.trim();
            if !trimmed.starts_with('!') {
                scl_importer.consume(line_number + 1, trimmed)?;
            }
        }

        scl_importer.finalize()
    }

    pub fn export(&self) -> SclExport<'_> {
        SclExport(self)
    }
}

struct SclImporter {
    state: ParserState,
}

enum ParserState {
    ExpectingDescription,
    ExpectingNumberOfNotes(String),
    ConsumingPitchLines(usize, SclBuilder),
}

impl SclImporter {
    fn consume(&mut self, line_number: usize, line: &str) -> Result<(), SclImportError> {
        // Get ownership of the current state
        let mut state = ParserState::ExpectingDescription;
        mem::swap(&mut self.state, &mut state);

        self.state = match state {
            ParserState::ExpectingDescription => {
                ParserState::ExpectingNumberOfNotes(line.to_owned())
            }
            ParserState::ExpectingNumberOfNotes(description) => {
                let builder = Scl::with_name(description);
                let num_notes = line.parse().map_err(|_| SclImportError::ParseError {
                    line_number,
                    description: "Number of notes not parseable",
                })?;
                ParserState::ConsumingPitchLines(num_notes, builder)
            }
            ParserState::ConsumingPitchLines(num_notes, mut builder) => {
                let main_item = line.split_ascii_whitespace().next().ok_or_else(|| {
                    SclImportError::ParseError {
                        line_number,
                        description: "Line is empty",
                    }
                })?;
                if main_item.contains('.') {
                    let cents_value =
                        main_item.parse().map_err(|_| SclImportError::ParseError {
                            line_number,
                            description: "Cents value not parseable",
                        })?;
                    builder.push_cents(cents_value);
                } else if main_item.contains('/') {
                    let mut splitted = main_item.splitn(2, '/');
                    let numer = splitted.next().unwrap().parse().map_err(|_| {
                        SclImportError::ParseError {
                            line_number,
                            description: "Numer not parseable",
                        }
                    })?;
                    let denom = splitted.next().unwrap().parse().map_err(|_| {
                        SclImportError::ParseError {
                            line_number,
                            description: "Denom not parseable",
                        }
                    })?;
                    builder.push_fraction(numer, denom);
                } else {
                    let int_value = main_item.parse().map_err(|_| SclImportError::ParseError {
                        line_number,
                        description: "Int value not parseable",
                    })?;
                    builder.push_int(int_value)
                }
                ParserState::ConsumingPitchLines(num_notes, builder)
            }
        };
        Ok(())
    }

    fn finalize(self) -> Result<Scl, SclImportError> {
        match self.state {
            ParserState::ConsumingPitchLines(num_notes, builder) => {
                let scl = builder.build()?;
                if scl.size() == num_notes {
                    Ok(scl)
                } else {
                    Err(SclImportError::InconsistentNumberOfNotes)
                }
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum SclImportError {
    IoError(io::Error),
    ParseError {
        line_number: usize,
        description: &'static str,
    },
    InconsistentNumberOfNotes,
    BuildError(SclBuildError),
}

impl From<io::Error> for SclImportError {
    fn from(v: io::Error) -> Self {
        SclImportError::IoError(v)
    }
}

impl From<SclBuildError> for SclImportError {
    fn from(v: SclBuildError) -> Self {
        SclImportError::BuildError(v)
    }
}

pub struct SclBuilder(Scl, bool);

impl SclBuilder {
    pub fn push_ratio(&mut self, ratio: Ratio) {
        self.push_cents(ratio.as_cents());
    }

    pub fn push_cents(&mut self, cents_value: f64) {
        self.push_pitch_value(PitchValue::Cents(cents_value));
    }

    pub fn push_int(&mut self, int_value: u32) {
        self.push_pitch_value(PitchValue::Fraction(int_value, None));
    }

    pub fn push_fraction(&mut self, numer: u32, denom: u32) {
        self.push_pitch_value(PitchValue::Fraction(numer, Some(denom)));
    }

    fn push_pitch_value(&mut self, pitch_value: PitchValue) {
        self.1 &= pitch_value.as_ratio() > self.0.period;
        self.0.pitch_values.push(pitch_value);
        self.0.period = pitch_value.as_ratio();
    }

    pub fn build(self) -> Result<Scl, SclBuildError> {
        if self.1 && !self.0.pitch_values.is_empty() {
            Ok(self.0)
        } else {
            Err(SclBuildError::ScaleMustBeMonotonic)
        }
    }
}

#[derive(Clone, Debug)]
pub enum SclBuildError {
    ScaleMustBeMonotonic,
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

/// Keyboard mappings according to [http://www.huygens-fokker.org/scala/help.htm#mappings](http://www.huygens-fokker.org/scala/help.htm#mappings).
#[derive(Clone, Debug)]
pub struct Kbm {
    pub ref_pitch: ReferencePitch,
    pub root_key: PianoKey,
}

impl Kbm {
    pub fn root_at(note: impl PitchedNote) -> Self {
        Kbm {
            ref_pitch: ReferencePitch::from_note(note),
            root_key: note.note().as_piano_key(),
        }
    }

    pub fn export(&self) -> KbmExport<'_> {
        KbmExport(self)
    }
}

pub struct KbmExport<'a>(&'a Kbm);

impl<'a> Display for KbmExport<'a> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "1")?;
        writeln!(f, "0")?;
        writeln!(f, "127")?;
        writeln!(f, "{}", self.0.root_key.midi_number())?;
        writeln!(f, "{}", self.0.ref_pitch.key().midi_number())?;
        writeln!(f, "{}", self.0.ref_pitch.pitch().as_hz())?;
        writeln!(f, "1")?;
        writeln!(f, "0")?;
        Ok(())
    }
}

impl<S: Borrow<Scl>, K: Borrow<Kbm>> Tuning<PianoKey> for (S, K) {
    fn pitch_of(&self, key: PianoKey) -> Pitch {
        let degree = self.1.borrow().root_key.num_keys_before(key);
        self.pitch_of(degree)
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<PianoKey> {
        let degree: Approximation<i32> = self.find_by_pitch(pitch);
        let key = PianoKey::from_midi_number(
            self.1.borrow().root_key.midi_number() + degree.approx_value,
        );
        Approximation {
            approx_value: key,
            deviation: degree.deviation,
        }
    }
}

impl<S: Borrow<Scl>, K: Borrow<Kbm>> Tuning<i32> for (S, K) {
    fn pitch_of(&self, degree: i32) -> Pitch {
        let scale = self.0.borrow();
        let key_map = self.1.borrow();
        let reference_pitch =
            scale.relative_pitch_of(key_map.root_key.num_keys_before(key_map.ref_pitch.key()));
        let normalized_pitch = scale.relative_pitch_of(degree);
        key_map.ref_pitch.pitch() / reference_pitch * normalized_pitch
    }

    fn find_by_pitch(&self, pitch: Pitch) -> Approximation<i32> {
        let scale = self.0.borrow();

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
            ratio_to_find.deviation_from(lower_ratio),
            upper_ratio.deviation_from(ratio_to_find),
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

pub fn create_equal_temperament_scale(step_size: Ratio) -> Scl {
    let mut scale = Scl::with_name(format!(
        "equal steps of {:#} ({:.2}-EDO)",
        step_size,
        Ratio::octave().num_equal_steps_of_size(step_size)
    ));
    scale.push_ratio(step_size);
    scale.build().unwrap()
}

pub fn create_rank2_temperament_scale(
    generator: Ratio,
    num_pos_generations: u16,
    num_neg_generations: u16,
    period: Ratio,
) -> Scl {
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

    let mut scale = Scl::with_name(format!(
        "{0} positive and {1} negative generations of generator {2} ({2:#}) with period {3}",
        num_pos_generations, num_neg_generations, generator, period
    ));
    for pitch_value in pitch_values {
        scale.push_ratio(pitch_value)
    }

    scale.build().unwrap()
}

pub fn create_harmonics_scale(
    lowest_harmonic: u32,
    number_of_notes: u32,
    subharmonics: bool,
) -> Scl {
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

    let mut scale = Scl::with_name(format!(
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

    scale.build().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{key::PianoKey, note::NoteLetter, pitch::ReferencePitch};
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn equal_temperament_scale_correctness() {
        let bohlen_pierce = create_equal_temperament_scale("1:13:3".parse().unwrap());

        assert_eq!(bohlen_pierce.size(), 1);
        assert_approx_eq!(bohlen_pierce.period().as_cents(), 146.304_231);

        AssertScale(bohlen_pierce, Kbm::root_at(NoteLetter::A.in_octave(4)))
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

        AssertScale(pythagorean_major, Kbm::root_at(NoteLetter::A.in_octave(4)))
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

        AssertScale(harmonics, Kbm::root_at(NoteLetter::A.in_octave(4)))
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
    fn import_scl() {
        let input = &b"!A comment
            ! A second comment
            Test scale
            7
            100.
            150.0 ignore any text
            !175.0 ignore comment
            200.0 .ignore dots
            6/5
            5/4 (ignore parentheses)
            3/2 /ignore additional slashes
            2"[..];

        let scl = Scl::import(input).unwrap();
        assert_eq!(scl.description(), "Test scale");
        assert_eq!(scl.size(), 7);
        assert_approx_eq!(scl.period().as_octaves(), 1.0);
        assert_approx_eq!(scl.relative_pitch_of(0).as_cents(), 0.0);
        assert_approx_eq!(scl.relative_pitch_of(1).as_cents(), 100.0);
        assert_approx_eq!(scl.relative_pitch_of(2).as_cents(), 150.0);
        assert_approx_eq!(scl.relative_pitch_of(3).as_cents(), 200.0);
        assert_approx_eq!(scl.relative_pitch_of(4).as_float(), 6.0 / 5.0);
        assert_approx_eq!(scl.relative_pitch_of(5).as_float(), 5.0 / 4.0);
        assert_approx_eq!(scl.relative_pitch_of(6).as_float(), 3.0 / 2.0);
        assert_approx_eq!(scl.relative_pitch_of(7).as_float(), 2.0);
    }

    #[test]
    fn import_scl_error_cases() {
        assert!(matches!(
            Scl::import(&b"Description\n3x\n100.0\n5/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 2,
                description: "Number of notes not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                description: "Line is empty"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0x\n5/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 3,
                description: "Cents value not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n5x/4\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                description: "Numer not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n5/4x\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                description: "Denom not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n5/4/3\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                description: "Denom not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n5/\n2"[..]),
            Err(SclImportError::ParseError {
                line_number: 4,
                description: "Denom not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n3\n100.0\n5/4\n2x"[..]),
            Err(SclImportError::ParseError {
                line_number: 5,
                description: "Int value not parseable"
            })
        ));
        assert!(matches!(
            Scl::import(&b"Description\n7\n100.0\n5/4\n2"[..]),
            Err(SclImportError::InconsistentNumberOfNotes)
        ));
    }

    #[test]
    fn best_fit_correctness() {
        let harmonics = create_harmonics_scale(8, 8, false);
        AssertScale(harmonics, Kbm::root_at(NoteLetter::A.in_octave(4)))
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

    struct AssertScale(Scl, Kbm);

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
            let approximation = Pitch::from_hz(freq_hz).find_in::<PianoKey, _>(&(&self.0, &self.1));
            assert_eq!(
                approximation.approx_value,
                PianoKey::from_midi_number(midi_number)
            );
            assert_approx_eq!(approximation.deviation.as_float(), deviation_as_float);
            self
        }
    }

    #[test]
    fn format_key_map() {
        let key_map = Kbm {
            root_key: PianoKey::from_midi_number(60),
            ref_pitch: ReferencePitch::from_key_and_pitch(
                NoteLetter::A.in_octave(4).as_piano_key(),
                Pitch::from_hz(430.0),
            ),
        };

        assert_eq!(
            key_map.export().to_string().lines().collect::<Vec<_>>(),
            ["1", "0", "127", "60", "69", "430", "1", "0"]
        )
    }
}
