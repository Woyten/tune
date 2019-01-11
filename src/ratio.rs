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
        if s.starts_with('{') && s.ends_with('}') {
            s[1..s.len() - 1].parse::<Ratio>()
        } else if let [numer, denom] = s.split(balanced('/')).collect::<Vec<_>>().as_slice() {
            let numer = numer
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid numerator '{}': {}", numer, e))?;
            let denom = denom
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid denominator '{}': {}", denom, e))?;
            Ratio::from_float(numer.as_float() / denom.as_float())
        } else if let [numer, denom, interval] =
            s.split(balanced(':')).collect::<Vec<_>>().as_slice()
        {
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
        } else if let [cents, ""] = s.split(balanced('c')).collect::<Vec<_>>().as_slice() {
            let cents = cents
                .parse::<Ratio>()
                .map_err(|e| format!("Invalid cent value '{}': {}", cents, e))?;
            Ratio::from_float((cents.as_float() / 1200.0).exp2())
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
