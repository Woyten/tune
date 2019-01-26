use std::str::FromStr;

#[derive(Copy, Clone, Debug)]
pub struct Ratio {
    float_value: f64,
}

impl Ratio {
    fn from_float(float_value: f64) -> Result<Self, String> {
        if float_value.is_finite() {
            Ok(Ratio { float_value })
        } else {
            Err(format!("Expression evaluates to {}", float_value))
        }
    }

    pub fn as_float(self) -> f64 {
        self.float_value
    }
}

impl FromStr for Ratio {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let [numer, denom, interval] = s.split(balanced(':')).collect::<Vec<_>>().as_slice() {
            let numer = numer
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval numerator '{}': {}", numer, e))?;
            let denom = denom
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval denominator '{}': {}", denom, e))?;
            let interval = interval
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid interval '{}': {}", interval, e))?;
            Ratio::from_float(
                interval
                    .as_float()
                    .powf(numer.as_float() / denom.as_float()),
            )
        } else if let [numer, denom] = s.split(balanced('/')).collect::<Vec<_>>().as_slice() {
            let numer = numer
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid numerator '{}': {}", numer, e))?;
            let denom = denom
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid denominator '{}': {}", denom, e))?;
            Ratio::from_float(numer.as_float() / denom.as_float())
        } else if let [cents, ""] = s.split(balanced('c')).collect::<Vec<_>>().as_slice() {
            let cents = cents
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid cent value '{}': {}", cents, e))?;
            Ratio::from_float((cents.as_float() / 1200.0).exp2())
        } else if s.starts_with('{') && s.ends_with('}') {
            s[1..s.len() - 1].parse::<Ratio>()
        } else {
            Ratio::from_float(s.parse::<f64>().map_err(|_| {
                "Invalid value: Must be a float (e.g. 1.5), fraction (e.g. 3/2),\
                 interval fraction (e.g. 7:12:2) or cent value (e.g. 702c)"
                    .to_string()
            })?)
        }
    }
}

fn balanced(character_to_match: char) -> impl FnMut(char) -> bool {
    let mut num_parens = 0;
    move |c| match c {
        '{' => {
            num_parens += 1;
            false
        }
        '}' => {
            num_parens -= 1;
            false
        }
        other => num_parens == 0 && other == character_to_match,
    }
}

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
