pub fn div_mod_i32(numer: i32, denom: u32) -> (i32, u32) {
    if numer >= 0 {
        let pos_numer = numer as u32;
        let division = (pos_numer / denom) as i32;
        let remainder = pos_numer % denom;
        (division, remainder)
    } else {
        let neg_numer = -numer as u32;
        let division = (neg_numer / denom) as i32;
        let remainder = neg_numer % denom;
        if remainder != 0 {
            (-division - 1, denom - remainder)
        } else {
            (-division, remainder)
        }
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
            (-6, std::u32::MAX, -1, std::u32::MAX - 6),
            (-5, std::u32::MAX, -1, std::u32::MAX - 5),
            (-1, std::u32::MAX, -1, std::u32::MAX - 1),
            (0, std::u32::MAX, 0, 0),
            (1, std::u32::MAX, 0, 1),
            (6, std::u32::MAX, 0, 6),
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
