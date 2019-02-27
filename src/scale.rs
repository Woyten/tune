//! Scale format according to [http://www.huygens-fokker.org/scala/scl_format.html](http://www.huygens-fokker.org/scala/scl_format.html).

use crate::ratio::Ratio;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;

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

    pub fn pitch(&self, note: i32) -> f64 {
        let (num_periods, phase) = div_mod(note - 69, self.pitch_values.len());
        let phase_factor = if phase == 0 {
            1.0
        } else {
            self.pitch_values[phase - 1].as_ratio().as_float()
        };
        440.0 * self.period.as_float().powi(num_periods) * phase_factor
    }

    pub fn format_scl(&self) -> String {
        let mut dest = Vec::new();
        self.write_scl(&mut dest).unwrap();
        String::from_utf8(dest).unwrap()
    }

    pub fn write_scl<W: io::Write>(&self, mut dest: W) -> io::Result<()> {
        writeln!(dest, "{}", self.description())?;
        writeln!(dest, "{}", self.pitch_values.len())?;
        for pitch_value in &self.pitch_values {
            writeln!(dest, "{}", pitch_value)?;
        }
        Ok(())
    }
}

fn div_mod(numer: i32, denom: usize) -> (i32, usize) {
    if numer >= 0 {
        (numer / denom as i32, (numer % denom as i32) as usize)
    } else {
        (
            numer / denom as i32 - 1,
            (numer % denom as i32 + denom as i32) as usize,
        )
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
    let mut scale = Scale::with_name(format!("equal steps of ratio {}", step_size));
    scale.push_ratio(step_size);
    scale.build()
}

pub fn create_rank2_temperament_scale(
    generator: Ratio,
    number_of_notes: u16,
    offset: i16,
    period: Ratio,
) -> Scale {
    assert!(
        period.as_float() > 1.0,
        "Ratio must be greater than 1 but was {}",
        period
    );

    let generator_in_cents = generator.as_cents();
    let period_in_cents = period.as_cents();

    let mut notes = (0..number_of_notes)
        .map(|generation| {
            let exponent = i32::from(generation) + i32::from(offset);
            if exponent == 0 {
                return period_in_cents;
            }

            let generated_note = f64::from(exponent) * generator_in_cents;
            let note_in_period_interval = generated_note % period_in_cents;

            if note_in_period_interval <= 0.0 {
                note_in_period_interval + period_in_cents
            } else {
                note_in_period_interval
            }
        })
        .collect::<Vec<_>>();

    notes.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("Comparison yielded an invalid result")
    });

    let mut scale = Scale::with_name(format!(
        "{} generations of generator {} with period {}",
        number_of_notes, generator, period
    ));
    for note in notes {
        scale.push_cents(note);
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
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn create_equal_temperament_scale() {
        let scale = super::create_equal_temperament_scale(Ratio::from_cents(123.456));

        scale.assert_has_pitches(
            66,
            73,
            &[
                355.257_110,
                381.515_990,
                409.715_799,
                440.000_000,
                472.522_663,
                507.449_242,
                544.957_425,
            ],
        );

        assert_eq!(
            extract_lines(&scale.format_scl()),
            ["equal steps of ratio 1.0739151 (123.456c)", "1", "123.456"]
        );
    }

    #[test]
    fn create_rank2_temperament_scale() {
        let scale = super::create_rank2_temperament_scale(
            Ratio::from_float(1.5),
            7,
            0,
            Ratio::from_float(2.0),
        );

        scale.assert_has_pitches(
            59,
            80,
            &[
                165.000_000,
                185.625_000,
                208.828_125,
                220.000_000,
                247.500_000,
                278.437_500,
                313.242_188,
                330.000_000,
                371.250_000,
                417.656_250,
                440.000_000,
                495.000_000,
                556.875_000,
                626.484_375,
                660.000_000,
                742.500_000,
                835.312_500,
                880.000_000,
                990.000_000,
                1_113.750_000,
                1_252.968_750,
            ],
        );

        assert_eq!(
            extract_lines(&scale.format_scl()),
            [
                "7 generations of generator 1.5000000 (701.955c) with period 2.0000000 (1200.000c)",
                "7",
                "203.910",
                "407.820",
                "611.730",
                "701.955",
                "905.865",
                "1109.775",
                "1200.000"
            ]
        );
    }

    #[test]
    fn create_harmonics_scale() {
        let scale = super::create_harmonics_scale(8, 8, false);

        assert_approx_eq!(scale.period().as_float(), 2.0);

        scale.assert_has_pitches(
            59,
            80,
            &[
                192.500, 206.250, 220.000, 247.500, 275.000, 302.500, 330.000, 357.500, 385.000,
                412.500, 440.000, 495.000, 550.000, 605.000, 660.000, 715.000, 770.000, 825.000,
                880.000, 990.000, 1100.000,
            ],
        );

        assert_eq!(
            extract_lines(&scale.format_scl()),
            [
                "8 harmonics starting with 8",
                "8",
                "9/8",
                "10/8",
                "11/8",
                "12/8",
                "13/8",
                "14/8",
                "15/8",
                "16/8"
            ]
        );
    }

    impl Scale {
        fn assert_has_pitches(&self, from: i32, to: i32, expected_pitches: &[f64]) {
            for (i, pitch) in (from..to).map(|note| self.pitch(note)).enumerate() {
                assert_approx_eq!(pitch, expected_pitches[i]);
            }
        }
    }

    fn extract_lines(input: &str) -> Vec<&str> {
        input.lines().collect()
    }
}