pub fn div_mod_i32(numer: i32, denom: u32) -> (i32, u32) {
    let denom = denom as i32;
    assert!(denom >= 0, "Invalid conversion from u32 to i32");

    let division = numer / denom;
    let remainder = numer % denom;

    if remainder < 0 {
        (division - 1, (remainder + denom) as u32)
    } else {
        (division, remainder as u32)
    }
}

pub fn mod_f64(numer: f64, denom: f64) -> f64 {
    let remainder = numer % denom;
    if remainder < 0.0 {
        remainder + denom
    } else {
        remainder
    }
}

#[cfg(test)]
mod test {
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn div_mod_i32() {
        let test_cases = [
            (-6, 5, -2, 4),
            (-5, 5, -1, 0),
            (-1, 5, -1, 4),
            (0, 5, 0, 0),
            (1, 5, 0, 1),
            (4, 5, 0, 4),
            (5, 5, 1, 0),
            (6, 5, 1, 1),
        ];
        for &(numer, denom, expected_div, expected_mod) in test_cases.iter() {
            assert_eq!(
                super::div_mod_i32(numer, denom),
                (expected_div, expected_mod)
            );
        }
    }

    #[test]
    fn div_mod_f64() {
        let test_cases = [
            (-6.0, 5.0, 4.0),
            (-5.0, 5.0, 0.0),
            (-1.0, 5.0, 4.0),
            (0.0, 5.0, 0.0),
            (1.0, 5.0, 1.0),
            (4.0, 5.0, 4.0),
            (5.0, 5.0, 0.0),
            (6.0, 5.0, 1.0),
        ];
        for &(numer, denom, expected_mod) in test_cases.iter() {
            assert_approx_eq!(super::mod_f64(numer, denom), expected_mod);
        }
    }
}
