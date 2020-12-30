use std::f64::consts::TAU;

pub fn sin(phase: f64) -> f64 {
    (phase * TAU).sin()
}

pub fn sin3(phase: f64) -> f64 {
    let sin = sin(phase);
    sin * sin * sin
}

pub fn triangle(phase: f64) -> f64 {
    (((0.75 + phase).fract() - 0.5).abs() - 0.25) * 4.0
}

pub fn square(phase: f64) -> f64 {
    (0.5 - phase).signum()
}

pub fn sawtooth(phase: f64) -> f64 {
    ((0.5 + phase).fract() - 0.5) * 2.0
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn waveform_correctness() {
        let eps = 1e-10;

        assert_approx_eq!(sin(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin(1.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin(3.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin(5.0 / 8.0), -(1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin(7.0 / 8.0), -(1.0f64 / 2.0).sqrt());

        assert_approx_eq!(sin3(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(1.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin3(3.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(5.0 / 8.0), -(1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin3(7.0 / 8.0), -(1.0f64 / 8.0).sqrt());

        assert_approx_eq!(triangle(0.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(1.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(2.0 / 8.0), 1.0);
        assert_approx_eq!(triangle(3.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(4.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(5.0 / 8.0), -0.5);
        assert_approx_eq!(triangle(6.0 / 8.0), -1.0);
        assert_approx_eq!(triangle(7.0 / 8.0), -0.5);

        assert_approx_eq!(square(0.0 / 8.0 + eps), 1.0);
        assert_approx_eq!(square(1.0 / 8.0), 1.0);
        assert_approx_eq!(square(2.0 / 8.0), 1.0);
        assert_approx_eq!(square(3.0 / 8.0), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(square(5.0 / 8.0), -1.0);
        assert_approx_eq!(square(6.0 / 8.0), -1.0);
        assert_approx_eq!(square(7.0 / 8.0), -1.0);
        assert_approx_eq!(square(8.0 / 8.0 - eps), -1.0);

        assert_approx_eq!(sawtooth(0.0 / 8.0), 0.0);
        assert_approx_eq!(sawtooth(1.0 / 8.0), 0.25);
        assert_approx_eq!(sawtooth(2.0 / 8.0), 0.5);
        assert_approx_eq!(sawtooth(3.0 / 8.0), 0.75);
        assert_approx_eq!(sawtooth(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(sawtooth(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(sawtooth(5.0 / 8.0), -0.75);
        assert_approx_eq!(sawtooth(6.0 / 8.0), -0.5);
        assert_approx_eq!(sawtooth(7.0 / 8.0), -0.25);
    }
}
