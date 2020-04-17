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
