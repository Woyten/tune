/// Returns the integer division and remainder with `numer` being an `i32` and `denom` being an `u32`.
///
/// The resulting remainder is a *positive* number between 0 and `numer-1` with `result.0 * denom + result.1 = numer`.
/// Overflows are handled correctly for almost every `(i32, u32)` pair.
///
/// # Panics
///
/// Panics if `numer == i32::MIN` or `denom == 0`.
///
/// # Examples
///
/// ```
/// # use std::i32;
/// # use std::u32;
/// # use tune::math;
/// // numer is positive
/// assert_eq!(math::div_mod_i32(1, 5), (0, 1));
/// assert_eq!(math::div_mod_i32(4, 5), (0, 4));
/// assert_eq!(math::div_mod_i32(5, 5), (1, 0));
/// assert_eq!(math::div_mod_i32(6, 5), (1, 1));
///
/// // numer is negative
/// assert_eq!(math::div_mod_i32(-6, 5), (-2, 4));
/// assert_eq!(math::div_mod_i32(-5, 5), (-1, 0));
/// assert_eq!(math::div_mod_i32(-4, 5), (-1, 1));
/// assert_eq!(math::div_mod_i32(-1, 5), (-1, 4));
///
/// // numer is zero
/// assert_eq!(math::div_mod_i32(0, 5), (0, 0));
///
/// // denom is u32::MAX
/// assert_eq!(math::div_mod_i32(-6, u32::MAX), (-1, u32::MAX - 6));
/// assert_eq!(math::div_mod_i32(-5, u32::MAX), (-1, u32::MAX - 5));
/// assert_eq!(math::div_mod_i32(-1, u32::MAX), (-1, u32::MAX - 1));
/// assert_eq!(math::div_mod_i32(0, u32::MAX), (0, 0));
/// assert_eq!(math::div_mod_i32(1, u32::MAX), (0, 1));
/// assert_eq!(math::div_mod_i32(5, u32::MAX), (0, 5));
/// assert_eq!(math::div_mod_i32(6, u32::MAX), (0, 6));
///
/// // numer is i32::MIN or i32::MAX
/// assert_eq!(math::div_mod_i32(i32::MIN + 1, u32::MAX), (-1, i32::MAX as u32 + 1));
/// assert_eq!(math::div_mod_i32(i32::MAX, u32::MAX), (0, i32::MAX as u32));
/// ```

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
            (-division, 0)
        }
    }
}

/// Simplifies a fraction of `u16`s.
///
/// # Examples
///
/// ```
/// # use tune::math;
/// // With simplification
/// assert_eq!(math::simplify_u16(35, 20), (7, 4));
/// assert_eq!(math::simplify_u16(35, 21), (5, 3));
///
/// // Simplification is idempotent
/// assert_eq!(math::simplify_u16(7, 4), (7, 4));
/// assert_eq!(math::simplify_u16(5, 3), (5, 3));
///
/// // Degenerate cases
/// assert_eq!(math::simplify_u16(0, 0), (0, 0));
/// assert_eq!(math::simplify_u16(35, 0), (1, 0));
/// assert_eq!(math::simplify_u16(0, 21), (0, 1));
pub fn simplify_u16(mut numer: u16, mut denom: u16) -> (u16, u16) {
    let gcd = gcd_u16(numer, denom);
    if gcd != 0 {
        numer /= gcd;
        denom /= gcd;
    }
    (numer, denom)
}

/// Determines the greatest common divisor of two `u16`s.
///
/// # Examples
///
/// ```
/// # use tune::math;
/// // Regular cases
/// assert_eq!(math::gcd_u16(35, 20), 5);
/// assert_eq!(math::gcd_u16(35, 21), 7);
/// assert_eq!(math::gcd_u16(35, 22), 1);
///
/// // When one number is equal to 1
/// assert_eq!(math::gcd_u16(1, 21), 1);
/// assert_eq!(math::gcd_u16(35, 1), 1);
///
/// // When one number is equal to 0
/// assert_eq!(math::gcd_u16(35, 0), 35);
/// assert_eq!(math::gcd_u16(0, 21), 21);
/// ```
pub fn gcd_u16(mut x: u16, mut y: u16) -> u16 {
    while y != 0 {
        let t = y;
        y = x % y;
        x = t;
    }
    x
}

/// Removes all powers of two from a `u16`.
///
/// # Examples
///
/// ```
/// # use tune::math;
/// assert_eq!(math::odd_factors_u16(0), 0);
/// assert_eq!(math::odd_factors_u16(1), 1);
/// assert_eq!(math::odd_factors_u16(2), 1);
/// assert_eq!(math::odd_factors_u16(3), 3);
/// assert_eq!(math::odd_factors_u16(10), 5);
/// assert_eq!(math::odd_factors_u16(24), 3);
/// assert_eq!(math::odd_factors_u16(35), 35);
/// ```
pub fn odd_factors_u16(mut number: u16) -> u16 {
    if number != 0 {
        while number % 2 == 0 {
            number /= 2;
        }
    }
    number
}
