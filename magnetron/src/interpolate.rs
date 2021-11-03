pub trait Interpolate {
    fn interpolate(left: Self, right: Self, interpolation: f64) -> Self;
}

impl Interpolate for f64 {
    fn interpolate(left: Self, right: Self, interpolation: f64) -> Self {
        (1.0 - interpolation) * left + interpolation * right
    }
}

impl Interpolate for (f64, f64) {
    fn interpolate(left: Self, right: Self, interpolation: f64) -> Self {
        (
            f64::interpolate(left.0, right.0, interpolation),
            f64::interpolate(left.1, right.1, interpolation),
        )
    }
}
