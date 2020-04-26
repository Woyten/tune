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
            float_value > 0.0,
            "Ratio must be positive but was {}",
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

    /// ```
    /// # use tune::pitch::Pitch;
    /// # use tune::ratio::Ratio;
    /// let pitch_330_hz = Pitch::from_hz(330.0);
    /// let pitch_440_hz = Pitch::from_hz(440.0);
    /// assert_eq!(Ratio::between_pitches(pitch_330_hz, pitch_440_hz).as_float(), 4.0 / 3.0);
    /// ```
    pub fn between_pitches(pitch_a: impl Pitched, pitch_b: impl Pitched) -> Self {
        Ratio::from_float(pitch_b.pitch().as_hz() / pitch_a.pitch().as_hz())
    }

    fn from_finite_float(float_value: f64) -> Result<Self, String> {
        if float_value.is_finite() {
            Ok(Self { float_value })
        } else {
            Err(format!("Expression evaluates to {}", float_value))
        }
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

    // (3/2)^4 = 3/2 + oct + cents (comma)
}

impl Default for Ratio {
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
    fn default() -> Self {
        Self::from_float(1.0)
    }
}

impl Display for Ratio {
    /// ```
    /// # use tune::ratio::Ratio;
    /// assert_eq!(Ratio::from_float(1.5).to_string(), "1.5000000 (701.955c)");
    /// assert_eq!(Ratio::from_float(0.8).to_string(), "0.8000000 (-386.314c)");
    /// ```
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:.7} ({:.3}c)", self.as_float(), self.as_cents())
    }
}

impl FromStr for Ratio {
    type Err = String;

    /// Parses a [`Ratio`] using `tune`'s built-in expression language.
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
    /// assert_eq!("foo".parse::<Ratio>().unwrap_err(), "Must be a float (e.g. 1.5), fraction (e.g. 3/2), interval fraction (e.g. 7:12:2) or cent value (e.g. 702c)");
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let [numer, denom, interval] = parse::split_balanced(&s, ':').as_slice() {
            let numer = numer
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval numerator '{}': {}", numer, e))?;
            let denom = denom
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval denominator '{}': {}", denom, e))?;
            let interval = interval
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval '{}': {}", interval, e))?;
            Ratio::from_finite_float(
                interval
                    .as_float()
                    .powf(numer.as_float() / denom.as_float()),
            )
        } else if let [numer, denom] = parse::split_balanced(&s, '/').as_slice() {
            let numer = numer
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid numerator '{}': {}", numer, e))?;
            let denom = denom
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid denominator '{}': {}", denom, e))?;
            Ratio::from_finite_float(numer.as_float() / denom.as_float())
        } else if let [cents, ""] = parse::split_balanced(&s, 'c').as_slice() {
            let cents = cents
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid cent value '{}': {}", cents, e))?;
            Ok(Ratio::from_cents(cents.as_float()))
        } else if s.starts_with('{') && s.ends_with('}') {
            s[1..s.len() - 1].parse::<Ratio>()
        } else {
            Ratio::from_finite_float(s.parse().map_err(|_| {
                "Must be a float (e.g. 1.5), fraction (e.g. 3/2), \
                 interval fraction (e.g. 7:12:2) or cent value (e.g. 702c)"
                    .to_string()
            })?)
        }
    }
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
            ("0", 0.0000),
            ("1", 1.0000),
            ("99.9", 99.9000),
            ("-1.2345", -1.2345),
            ("{1.25}", 1.2500),
            ("{{1.25}}", 1.2500),
            ("0/3", 0.0000),
            ("10/3", 3.3333),
            ("10/{10/3}", 3.0000),
            ("{10/3}/10", 0.3333),
            ("{3/4}/{5/6}", 0.9000),
            ("{{3/4}/{5/6}}", 0.9000),
            ("0:12:2", 1.000),
            ("7:12:2", 1.4983),   // 2^(7/12) - perfect fifth
            ("7/12:1:2", 1.4983), // 2^(7/12) - perfect fifth
            ("12:12:2", 2.000),
            ("-12:12:2", 0.500),
            ("4:1:3/2", 5.0625),   // (3/2)^4 - 4 harmonic fifths
            ("1:1/4:3/2", 5.0625), // (3/2)^4 - 4 harmonic fifths
            ("1/2:3/2:{1:2:64}", 2.0000),
            ("{{1/2}:{3/2}:{1:2:64}}", 2.0000),
            ("12:7:700c", 2.000),
            ("0c", 1.0000),
            ("702c", 1.5000),  // 2^(702/1200) - harmonic fifth
            ("-702c", 0.6666), // 2^(-702/1200) - harmonic fifth downwards
            ("1200c", 2.0000),
            ("702c/3", 0.5000),    // 2^(702/1200)/3 - 702 cents divided by 3
            ("3/702c", 2.0000),    // 3/2^(702/1200) - 3 divided by 702 cents
            ("{1404/2}c", 1.5000), // 2^(702/1200) - 1402/2 cents
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
