//! Linear and logarithmic operations on frequency ratios.

use crate::math;
use crate::{parse, pitch::Pitched};
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::str::FromStr;

/// Struct representing the relative distance between two pitches.
///
/// Mathematically, this distance can be interpreted as the factor between the two pitches in
/// linear frequency space or as the offset between them in logarithmic frequency space.
///
/// The [`Ratio`] struct offers both linear and logarithmic accessors to the encapsulated distance.
/// It is possible to convert between the different representations by using `from_<repr1>` and `as_<repr2>` in
/// combination where `<reprN>` can be a linear (`float`) or logarithmic (`cents`, `semitones`, `octaves`) quantity.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::ratio::Ratio;
/// assert_approx_eq!(Ratio::from_float(1.5).as_cents(), 701.955);
/// assert_approx_eq!(Ratio::from_cents(400.0).as_semitones(), 4.0);
/// assert_approx_eq!(Ratio::from_semitones(3.0).as_octaves(), 0.25);
/// assert_approx_eq!(Ratio::from_octaves(3.0).as_float(), 8.0);
/// ```
///
/// # Panics
///
/// Panics if the *linear* value is not a finite positive number.
///
/// ```
/// # use tune::ratio::Ratio;
/// Ratio::from_cents(0.0); // This is Ok
/// Ratio::from_cents(-3.0); // This is Ok
/// ```
///
/// ```should_panic
/// # use tune::ratio::Ratio;
/// Ratio::from_float(0.0); // But this isn't. Should be positive.
/// ```
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Ratio {
    float_value: f64,
}

impl Ratio {
    pub fn from_float(float_value: f64) -> Self {
        assert!(
            float_value.is_finite() && float_value > 0.0,
            "Ratio must be finite and positive but was {}",
            float_value
        );
        Self { float_value }
    }

    pub fn from_cents(cents_value: f64) -> Self {
        Self::from_octaves(cents_value / 1200.0)
    }

    pub fn from_semitones(semitones: impl Into<f64>) -> Self {
        Self::from_octaves(semitones.into() / 12.0)
    }

    pub fn from_octaves(octaves: impl Into<f64>) -> Self {
        Self::from_float(octaves.into().exp2())
    }

    pub fn octave() -> Self {
        Self::from_float(2.0)
    }

    /// Creates a new [`Ratio`] instance based on the relative distance between two [`Pitched`] entities.
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::pitch::Pitch;
    /// # use tune::ratio::Ratio;
    /// let pitch_330_hz = Pitch::from_hz(330.0);
    /// let pitch_440_hz = Pitch::from_hz(440.0);
    /// assert_approx_eq!(Ratio::between_pitches(pitch_330_hz, pitch_440_hz).as_float(), 4.0 / 3.0);
    /// ```
    pub fn between_pitches(pitch_a: impl Pitched, pitch_b: impl Pitched) -> Self {
        Ratio::from_float(pitch_b.pitch().as_hz() / pitch_a.pitch().as_hz())
    }

    /// Stretches `self` by the provided `stretch`.
    ///
    /// This reverses [`Ratio::deviation_from`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::octave().stretched_by(Ratio::from_cents(10.0)).as_cents(), 1210.0);
    /// ```
    pub fn stretched_by(self, stretch: Ratio) -> Ratio {
        Ratio::from_float(self.as_float() * stretch.as_float())
    }

    /// Calculates the difference between the provided `reference` and `self`.
    ///
    /// This reverses [`Ratio::stretched_by`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::from_cents(1210.0).deviation_from(Ratio::octave()).as_cents(), 10.0);
    /// ```
    pub fn deviation_from(self, reference: Ratio) -> Ratio {
        Ratio::from_float(self.as_float() / reference.as_float())
    }

    /// Creates a new [`Ratio`] instance by applying `self` `num_repetitions` times.
    ///
    /// This reverses [`Ratio::divided_into_equal_steps`] or [`Ratio::num_equal_steps_of_size`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::from_semitones(2.0).repeated(3).as_semitones(), 6.0);
    /// ```
    pub fn repeated(self, num_repetitions: impl Into<f64>) -> Ratio {
        Ratio::from_octaves(self.as_octaves() * num_repetitions.into())
    }

    /// Returns the [`Ratio`] resulting from dividing `self` into `num_steps` equal steps.
    ///
    /// This reverses [`Ratio::repeated`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::octave().divided_into_equal_steps(15).as_cents(), 80.0);
    /// ```
    pub fn divided_into_equal_steps(self, num_steps: impl Into<f64>) -> Ratio {
        Ratio::from_octaves(self.as_octaves() / num_steps.into())
    }

    /// Determines how many equal steps of size `step_size` fit into `self`.
    ///
    /// This reverses [`Ratio::repeated`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::octave().num_equal_steps_of_size(Ratio::from_cents(80.0)), 15.0);
    /// ```
    pub fn num_equal_steps_of_size(self, step_size: Ratio) -> f64 {
        self.as_octaves() / step_size.as_octaves()
    }

    pub fn as_float(self) -> f64 {
        self.float_value
    }

    pub fn as_cents(self) -> f64 {
        self.as_semitones() * 100.0
    }

    pub fn as_semitones(self) -> f64 {
        self.as_octaves() * 12.0
    }

    pub fn as_octaves(self) -> f64 {
        self.float_value.log2()
    }

    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// assert_approx_eq!(Ratio::from_float(4.0).inv().as_float(), 0.25);
    /// assert_approx_eq!(Ratio::from_cents(150.0).inv().as_cents(), -150.0);
    /// ```
    pub fn inv(self) -> Ratio {
        Self {
            float_value: 1.0 / self.float_value,
        }
    }

    /// Check whether the given [`Ratio`] is is_negligible.
    ///
    /// The threshold is around a 500th of a cent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tune::ratio::Ratio;
    /// assert!(!Ratio::from_cents(0.002).is_negligible());
    /// assert!(Ratio::from_cents(0.001).is_negligible());
    /// assert!(Ratio::from_cents(0.000).is_negligible());
    /// assert!(Ratio::from_cents(-0.001).is_negligible());
    /// assert!(!Ratio::from_cents(-0.002).is_negligible());
    /// ```
    pub fn is_negligible(self) -> bool {
        (0.999999..1.000001).contains(&self.float_value)
    }

    /// Finds a rational number approximation of the current [Ratio] instance.
    ///
    /// The largest acceptable numerator or denominator can be controlled using the `limit` parameter.
    /// Only odd factors are compared against the `limit` which means that 12 is 3, effectively, while 11 stays 11.
    /// Read the documentation of [`math::odd_factors_u16`] for more examples.
    ///
    /// # Examples
    ///
    /// A minor seventh can be approximated by 16/9.
    ///
    ///```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// let minor_seventh = Ratio::from_semitones(10);
    /// let limit = 11;
    /// let f = minor_seventh.nearest_fraction(9);
    /// assert_eq!((f.numer, f.denom), (16, 9));
    /// assert_eq!(f.num_octaves, 0);
    /// assert_approx_eq!(f.deviation.as_cents(), 3.910002); // Quite good!
    /// ```
    ///
    /// Reducing the `limit` saves computation time but may lead to a bad approximation.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// # let minor_seventh = Ratio::from_semitones(10);
    /// let limit = 5;
    /// let f = minor_seventh.nearest_fraction(limit);
    /// assert_eq!((f.numer, f.denom), (5, 3));
    /// assert_eq!(f.num_octaves, 0);
    /// assert_approx_eq!(f.deviation.as_cents(), 115.641287); // Pretty bad!
    /// ```
    ///
    /// The approximation is normalized to values within an octave. The number of octaves is reported separately.
    ///
    /// ```
    /// # use assert_approx_eq::assert_approx_eq;
    /// # use tune::ratio::Ratio;
    /// let lower_than_an_octave = Ratio::from_float(3.0 / 4.0);
    /// let f = lower_than_an_octave.nearest_fraction(11);
    /// assert_eq!((f.numer, f.denom), (3, 2));
    /// assert_eq!(f.num_octaves, -1);
    /// assert_approx_eq!(f.deviation.as_cents(), 0.0);
    /// ```
    pub fn nearest_fraction(self, limit: u16) -> NearestFraction {
        NearestFraction::for_float_with_limit(self.as_float(), limit)
    }
}

/// The default [`Ratio`] is the ratio that respresents equivalence of two frequencies, i.e. no distance at all.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::ratio::Ratio;
/// assert_approx_eq!(Ratio::default().as_float(), 1.0); // Neutral element for multiplication
/// assert_approx_eq!(Ratio::default().as_cents(), 0.0); // Neutral element for addition
/// ```
impl Default for Ratio {
    fn default() -> Self {
        Self::from_float(1.0)
    }
}

/// [`Ratio`]s can be formatted as float or cents.
///
/// # Examples
//
/// ```
/// # use tune::ratio::Ratio;
/// // As float
/// assert_eq!(format!("{}", Ratio::from_float(1.5)), "1.5000");
/// assert_eq!(format!("{}", Ratio::from_float(1.0 / 1.5)), "0.6667");
/// assert_eq!(format!("{:.2}", Ratio::from_float(1.0 / 1.5)), "0.67");
///
/// // As cents
/// assert_eq!(format!("{:#}", Ratio::from_float(1.5)), "+702.0c");
/// assert_eq!(format!("{:#}", Ratio::from_float(1.0 / 1.5)), "-702.0c");
/// assert_eq!(format!("{:#.2}", Ratio::from_float(1.0 / 1.5)), "-701.96c");
///
/// // With padding
/// assert_eq!(format!("{:=^#14.2}", Ratio::from_float(1.5)), "===+701.96c===");
/// ```
impl Display for Ratio {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatted = if f.alternate() {
            format!(
                "{:+.precision$}c",
                self.as_cents(),
                precision = f.precision().unwrap_or(1)
            )
        } else {
            format!(
                "{:.precision$}",
                self.as_float(),
                precision = f.precision().unwrap_or(4)
            )
        };
        f.pad_integral(true, "", &formatted)
    }
}

/// [`Ratio`]s can be parsed using `tune`'s built-in expression language.
///
/// # Examples
///
/// ```
/// # use assert_approx_eq::assert_approx_eq;
/// # use tune::ratio::Ratio;
/// assert_approx_eq!("1.5".parse::<Ratio>().unwrap().as_float(), 1.5);
/// assert_approx_eq!("3/2".parse::<Ratio>().unwrap().as_float(), 1.5);
/// assert_approx_eq!("7:12:2".parse::<Ratio>().unwrap().as_semitones(), 7.0);
/// assert_approx_eq!("702c".parse::<Ratio>().unwrap().as_cents(), 702.0);
/// assert_eq!("foo".parse::<Ratio>().unwrap_err(), "Invalid expression \'foo\': Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)");
impl FromStr for Ratio {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<RatioExpression>().map(RatioExpression::ratio)
    }
}

/// Target type for successfully parsed and validated ratio expressions.
#[derive(Copy, Clone, Debug)]
pub struct RatioExpression {
    ratio: Ratio,
    representation: RatioExpressionVariant,
}

impl RatioExpression {
    pub fn ratio(self) -> Ratio {
        self.ratio
    }

    pub fn variant(self) -> RatioExpressionVariant {
        self.representation
    }
}

/// The only way to construct a [`RatioExpression`] is via the [`FromStr`] trait.
impl FromStr for RatioExpression {
    type Err = String;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        s = s.trim();
        parse_ratio(s)
            .and_then(|representation| {
                representation.as_ratio().map(|ratio| Self {
                    ratio,
                    representation,
                })
            })
            .map_err(|e| format!("Invalid expression '{}': {}", s, e))
    }
}

/// Type used to distinguish which particular outer expression was given as string input bevor parsing.
#[derive(Copy, Clone, Debug)]
pub enum RatioExpressionVariant {
    Float {
        float_value: f64,
    },
    Fraction {
        numer: f64,
        denom: f64,
    },
    IntervalFraction {
        numer: f64,
        denom: f64,
        interval: f64,
    },
    Cents {
        cents_value: f64,
    },
}

impl RatioExpressionVariant {
    pub fn as_ratio(self) -> Result<Ratio, String> {
        let float_value = self.as_float()?;
        if float_value > 0.0 {
            Ok(Ratio { float_value })
        } else {
            Err(format!(
                "Evaluates to {} but should be positive",
                float_value
            ))
        }
    }

    fn as_float(self) -> Result<f64, String> {
        let as_float = match self {
            Self::Float { float_value } => float_value,
            Self::Fraction { numer, denom } => numer / denom,
            Self::IntervalFraction {
                numer,
                denom,
                interval,
            } => interval.powf(numer / denom),
            Self::Cents { cents_value } => Ratio::from_cents(cents_value).as_float(),
        };
        if as_float.is_finite() {
            Ok(as_float)
        } else {
            Err(format!("Evaluates to {}", as_float))
        }
    }
}

fn parse_ratio(s: &str) -> Result<RatioExpressionVariant, String> {
    let s = s.trim();
    if let [numer, denom, interval] = parse::split_balanced(&s, ':').as_slice() {
        Ok(RatioExpressionVariant::IntervalFraction {
            numer: parse_ratio_as_float(numer, "interval numerator")?,
            denom: parse_ratio_as_float(denom, "interval denominator")?,
            interval: parse_ratio_as_float(interval, "interval")?,
        })
    } else if let [numer, denom] = parse::split_balanced(&s, '/').as_slice() {
        Ok(RatioExpressionVariant::Fraction {
            numer: parse_ratio_as_float(numer, "numerator")?,
            denom: parse_ratio_as_float(denom, "denominator")?,
        })
    } else if let [cents_value, ""] = parse::split_balanced(&s, 'c').as_slice() {
        Ok(RatioExpressionVariant::Cents {
            cents_value: parse_ratio_as_float(cents_value, "cents value")?,
        })
    } else if s.starts_with('(') && s.ends_with(')') {
        parse_ratio(&s[1..s.len() - 1])
    } else {
        Ok(RatioExpressionVariant::Float {
            float_value: s.parse().map_err(|_| {
                "Must be a float (e.g. 1.5), fraction (e.g. 3/2), \
                 interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)"
                    .to_string()
            })?,
        })
    }
}

fn parse_ratio_as_float(s: &str, name: &str) -> Result<f64, String> {
    parse_ratio(s)
        .and_then(RatioExpressionVariant::as_float)
        .map_err(|e| format!("Invalid {} '{}': {}", name, s, e))
}

#[derive(Copy, Clone, Debug)]
pub struct NearestFraction {
    pub numer: u16,
    pub denom: u16,
    pub deviation: Ratio,
    pub num_octaves: i32,
}

impl NearestFraction {
    fn for_float_with_limit(number: f64, limit: u16) -> Self {
        #[derive(Copy, Clone)]
        enum Sign {
            Pos,
            Neg,
        }

        let num_octaves = number.log2().floor();
        let normalized_ratio = number / num_octaves.exp2();

        let (mut best_numer, mut best_denom, mut abs_deviation, mut deviation_sign) =
            (1, 1, normalized_ratio, Sign::Pos);

        for denom in 1..=limit {
            let numer = denom as f64 * normalized_ratio;
            for &(ratio, numer, sign) in [
                (numer / numer.floor(), numer.floor() as u16, Sign::Pos),
                (numer.ceil() / numer, numer.ceil() as u16, Sign::Neg),
            ]
            .iter()
            {
                if math::odd_factors_u16(numer) <= limit && ratio < abs_deviation {
                    best_numer = numer;
                    best_denom = denom;
                    abs_deviation = ratio;
                    deviation_sign = sign;
                }
            }
        }

        let deviation = Ratio::from_float(abs_deviation);

        let (numer, denom) = math::simplify_u16(best_numer, best_denom);

        NearestFraction {
            numer,
            denom,
            deviation: match deviation_sign {
                Sign::Pos => deviation,
                Sign::Neg => deviation.inv(),
            },
            num_octaves: num_octaves as i32,
        }
    }
}

impl Display for NearestFraction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} [{:+.0}c] ({:+}o)",
            self.numer,
            self.denom,
            self.deviation.as_cents(),
            self.num_octaves
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parses_successfully() {
        let test_cases = [
            ("1", 1.0000),
            ("99.9", 99.9000),
            ("(1.25)", 1.2500),
            ("(1.25)", 1.2500),
            ("10/3", 3.3333),
            ("10/(10/3)", 3.0000),
            ("(10/3)/10", 0.3333),
            ("(3/4)/(5/6)", 0.9000),
            ("(3/4)/(5/6)", 0.9000),
            ("0:12:2", 1.000),
            ("7:12:2", 1.4983),   // 2^(7/12) - 12-edo perfect fifth
            ("7/12:1:2", 1.4983), // 2^(7/12) - 12-edo perfect fifth
            ("12:12:2", 2.000),
            ("-12:12:2", 0.500),
            ("4:1:3/2", 5.0625),   // (3/2)^4 - pyhthagorean major third
            ("1:1/4:3/2", 5.0625), // (3/2)^4 - pyhthagorean major third
            ("1/2:3/2:(1:2:64)", 2.0000),
            ("((1/2):(3/2):(1:2:64))", 2.0000),
            (" (    (1 /2)  :(3 /2):   (1: 2:   64  ))     ", 2.0000),
            ("12:7:700c", 2.000),
            ("0c", 1.0000),
            ("(0/3)c", 1.0000),
            ("702c", 1.5000),  // 2^(702/1200) - pythgorean fifth
            ("-702c", 0.6666), // 2^(-702/1200) - pythgorean fifth downwards
            ("1200c", 2.0000),
            ("702c/3", 0.5000),    // 2^(702/1200)/3 - 702 cents divided by 3
            ("3/702c", 2.0000),    // 3/2^(702/1200) - 3 divided by 702 cents
            ("(1404/2)c", 1.5000), // 2^(702/1200) - 1402/2 cents
        ];

        for (input, expected) in test_cases.iter() {
            let parsed = input.parse::<Ratio>().unwrap().as_float();
            assert!(
                (parsed - expected).abs() < 0.0001,
                "`{}` should evaluate to {} but was {:.4}",
                input,
                expected,
                parsed
            );
        }
    }

    #[test]
    fn parses_with_error() {
        let test_cases = [
            (
                "0.0",
                "Invalid expression '0.0': Evaluates to 0 but should be positive",
            ),
            (
                "-1.2345",
                "Invalid expression '-1.2345': Evaluates to -1.2345 but should be positive",
            ),
            ("1/0", "Invalid expression '1/0': Evaluates to inf"),
            (
                "(1/0)c",
                "Invalid expression '(1/0)c': Invalid cents value '(1/0)': Evaluates to inf",
            ),
            (
                "(1/x)c",
                "Invalid expression '(1/x)c': Invalid cents value '(1/x)': Invalid denominator 'x': \
                 Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)",
            ),
            (
                "   (1   /x )c ",
                "Invalid expression '(1   /x )c': Invalid cents value '(1   /x )': Invalid denominator 'x': \
                 Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cents value (e.g. 702c)",
            ),
        ];

        for (input, expected) in test_cases.iter() {
            let parse_error = input.parse::<Ratio>().unwrap_err();
            assert_eq!(parse_error, *expected);
        }
    }

    #[test]
    fn parse_variant() {
        assert!(matches!(
            "1".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Float { .. }
        ));
        assert!(matches!(
            "10/3".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Fraction { .. }
        ));
        assert!(matches!(
            "(3/4)/(5/6)".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Fraction { .. }
        ));
        assert!(matches!(
            "12:7:700c".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::IntervalFraction { .. }
        ));
        assert!(matches!(
            "(0/3)c".parse::<RatioExpression>().unwrap().variant(),
            RatioExpressionVariant::Cents { .. }
        ));
    }

    #[test]
    fn render_ratio_of_rational_numbers() {
        let test_cases = [
            (0.9, "9/5 [+0c] (-1o)"),
            (1.0, "1/1 [+0c] (+0o)"),
            (1.1, "11/10 [+0c] (+0o)"),
            (1.2, "6/5 [+0c] (+0o)"),
            (1.3, "9/7 [+19c] (+0o)"),
            (1.4, "7/5 [+0c] (+0o)"),
            (1.5, "3/2 [+0c] (+0o)"),
            (1.6, "8/5 [+0c] (+0o)"),
            (1.7, "12/7 [-14c] (+0o)"),
            (1.8, "9/5 [+0c] (+0o)"),
            (1.9, "11/6 [+62c] (+0o)"),
            (2.0, "1/1 [+0c] (+1o)"),
            (2.1, "12/11 [-66c] (+1o)"),
        ];

        for &(number, formatted) in test_cases.iter() {
            assert_eq!(
                Ratio::from_float(number).nearest_fraction(11).to_string(),
                formatted
            );
        }
    }

    #[test]
    fn render_ratio_of_irrational_numbers() {
        let test_cases = [
            (-1, "11/6 [+51c] (-1o)"),
            (0, "1/1 [+0c] (+0o)"),
            (1, "12/11 [-51c] (+0o)"),
            (2, "9/8 [-4c] (+0o)"),
            (3, "6/5 [-16c] (+0o)"),
            (4, "5/4 [+14c] (+0o)"),
            (5, "4/3 [+2c] (+0o)"),
            (6, "7/5 [+17c] (+0o)"),
            (7, "3/2 [-2c] (+0o)"),
            (8, "8/5 [-14c] (+0o)"),
            (9, "5/3 [+16c] (+0o)"),
            (10, "16/9 [+4c] (+0o)"),
            (11, "11/6 [+51c] (+0o)"),
            (12, "1/1 [+0c] (+1o)"),
            (13, "12/11 [-51c] (+1o)"),
        ];

        for &(semitones, formatted) in test_cases.iter() {
            assert_eq!(
                Ratio::from_semitones(semitones)
                    .nearest_fraction(11)
                    .to_string(),
                formatted
            );
        }
    }
}
