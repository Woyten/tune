//! Specialized integer operations missing from the standard library.

use std::convert::{TryFrom, TryInto};

/// All `u8` prime numbers.
pub static U8_PRIMES: &[u8] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199, 211, 223, 227, 229, 233, 239, 241, 251,
];

/// Marks unsigned integer types that can be safely used in 32-bit integer divisions.
pub trait U32Denom: Copy + Into<i64> + TryFrom<i64> {}

impl U32Denom for u8 {}
impl U32Denom for u16 {}
impl U32Denom for u32 {}

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
/// assert_eq!(math::i32_div_u(0, 5u32), 0);
/// assert_eq!(math::i32_div_u(1, 5u32), 0);
/// assert_eq!(math::i32_div_u(4, 5u32), 0);
/// assert_eq!(math::i32_div_u(5, 5u32), 1);
/// assert_eq!(math::i32_div_u(6, 5u32), 1);
///
/// // When numer is negative
/// assert_eq!(math::i32_div_u(-1, 5u32), -1);
/// assert_eq!(math::i32_div_u(-4, 5u32), -1);
/// assert_eq!(math::i32_div_u(-5, 5u32), -1);
/// assert_eq!(math::i32_div_u(-6, 5u32), -2);
///
/// // Integer limits
/// assert_eq!(math::i32_div_u(i32::MIN, u32::MAX), -1);
/// assert_eq!(math::i32_div_u(-1, u32::MAX), -1);
/// assert_eq!(math::i32_div_u(1, u32::MAX), 0);
/// assert_eq!(math::i32_div_u(i32::MAX, u32::MAX), 0);
/// ```
pub fn i32_div_u<D: U32Denom>(numer: i32, denom: D) -> i32 {
    i64::from(numer)
        .div_euclid(denom.into())
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
/// assert_eq!(math::i32_rem_u(0, 5u32), 0);
/// assert_eq!(math::i32_rem_u(1, 5u32), 1);
/// assert_eq!(math::i32_rem_u(4, 5u32), 4);
/// assert_eq!(math::i32_rem_u(5, 5u32), 0);
/// assert_eq!(math::i32_rem_u(6, 5u32), 1);
///
/// // When numer is negative
/// assert_eq!(math::i32_rem_u(-1, 5u32), 4);
/// assert_eq!(math::i32_rem_u(-4, 5u32), 1);
/// assert_eq!(math::i32_rem_u(-5, 5u32), 0);
/// assert_eq!(math::i32_rem_u(-6, 5u32), 4);
///
/// // Integer limits
/// assert_eq!(math::i32_rem_u(i32::MIN, u32::MAX), i32::MAX as u32);
/// assert_eq!(math::i32_rem_u(-1, u32::MAX), u32::MAX - 1);
/// assert_eq!(math::i32_rem_u(1, u32::MAX), 1);
/// assert_eq!(math::i32_rem_u(i32::MAX, u32::MAX), i32::MAX as u32);
/// ```
pub fn i32_rem_u<D: U32Denom>(numer: i32, denom: D) -> D {
    i64::from(numer)
        .rem_euclid(denom.into())
        .try_into()
        .ok()
        .unwrap()
}

/// Evaluates [`i32_div_u`] and [`i32_rem_u`] in one call.
pub fn i32_dr_u<D: U32Denom>(numer: i32, denom: D) -> (i32, D) {
    (i32_div_u(numer, denom), i32_rem_u(numer, denom))
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
/// // When numbers are equal to 1
/// assert_eq!(math::gcd_u16(1, 21), 1);
/// assert_eq!(math::gcd_u16(35, 1), 1);
/// assert_eq!(math::gcd_u16(1, 1), 1);
///
/// // When numbers are equal to 0
/// assert_eq!(math::gcd_u16(35, 0), 35);
/// assert_eq!(math::gcd_u16(0, 21), 21);
/// assert_eq!(math::gcd_u16(0, 0), 1);
/// ```
pub fn gcd_u16(mut x: u16, mut y: u16) -> u16 {
    while y != 0 {
        let t = y;
        y = x % y;
        x = t;
    }
    x.max(1)
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
