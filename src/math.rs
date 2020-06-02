//! Specialized integer operations missing from the standard library.

use std::convert::TryInto;

/// Returns the euclidean division of a signed `numer` and an unsigned `denom`.
///
/// The result is a signed integer between `-numer` and `numer`.
/// The function returns valid results for every `(numer, denom)` pair where `denom != 0`.
///
/// # Panics
///
/// Panics if `denom == 0`.
///
/// # Examples
///
/// ```
/// # use std::i32;
/// # use std::u32;
/// # use tune::math;
/// assert_eq!(math::i32_div_u32(0, 5), 0);
/// assert_eq!(math::i32_div_u32(1, 5), 0);
/// assert_eq!(math::i32_div_u32(4, 5), 0);
/// assert_eq!(math::i32_div_u32(5, 5), 1);
/// assert_eq!(math::i32_div_u32(6, 5), 1);
///
/// // When numer is negative
/// assert_eq!(math::i32_div_u32(-1, 5), -1);
/// assert_eq!(math::i32_div_u32(-4, 5), -1);
/// assert_eq!(math::i32_div_u32(-5, 5), -1);
/// assert_eq!(math::i32_div_u32(-6, 5), -2);
///
/// // Integer limits
/// assert_eq!(math::i32_div_u32(i32::MIN, u32::MAX), -1);
/// assert_eq!(math::i32_div_u32(-1, u32::MAX), -1);
/// assert_eq!(math::i32_div_u32(1, u32::MAX), 0);
/// assert_eq!(math::i32_div_u32(i32::MAX, u32::MAX), 0);
/// ```
pub fn i32_div_u32(numer: i32, denom: u32) -> i32 {
    i64::from(numer)
        .div_euclid(i64::from(denom))
        .try_into()
        .unwrap()
}

/// Returns the euclidean remainder of a signed `numer` and an unsigned `denom`.
///
/// The result is an unsigned integer between `0` and `denom-1`.
/// The function returns valid results for every `(numer, denom)` pair where `denom != 0`.
///
/// # Panics
///
/// Panics if `denom == 0`.
///
/// # Examples
///
/// ```
/// # use std::i32;
/// # use std::u32;
/// # use tune::math;
/// assert_eq!(math::i32_rem_u32(0, 5), 0);
/// assert_eq!(math::i32_rem_u32(1, 5), 1);
/// assert_eq!(math::i32_rem_u32(4, 5), 4);
/// assert_eq!(math::i32_rem_u32(5, 5), 0);
/// assert_eq!(math::i32_rem_u32(6, 5), 1);
///
/// // When numer is negative
/// assert_eq!(math::i32_rem_u32(-1, 5), 4);
/// assert_eq!(math::i32_rem_u32(-4, 5), 1);
/// assert_eq!(math::i32_rem_u32(-5, 5), 0);
/// assert_eq!(math::i32_rem_u32(-6, 5), 4);
///
/// // Integer limits
/// assert_eq!(math::i32_rem_u32(i32::MIN, u32::MAX), i32::MAX as u32);
/// assert_eq!(math::i32_rem_u32(-1, u32::MAX), u32::MAX - 1);
/// assert_eq!(math::i32_rem_u32(1, u32::MAX), 1);
/// assert_eq!(math::i32_rem_u32(i32::MAX, u32::MAX), i32::MAX as u32);
/// ```
pub fn i32_rem_u32(numer: i32, denom: u32) -> u32 {
    i64::from(numer)
        .rem_euclid(i64::from(denom))
        .try_into()
        .unwrap()
}

/// Evaluates [`i32_div_u32`] and [`i32_rem_u32`] in one call.
pub fn i32_dr_u32(numer: i32, denom: u32) -> (i32, u32) {
    (i32_div_u32(numer, denom), i32_rem_u32(numer, denom))
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
