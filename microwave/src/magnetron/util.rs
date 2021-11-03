use std::f64::consts::TAU;

use magnetron::delay::DelayLine;

pub trait Interaction {
    fn process_sample(&mut self, input: f64) -> f64;

    /// Returns the inherent temporal delay of this interaction. This is information is needed for precise waveguide tuning.
    fn delay_samples(&self) -> f64;

    fn mute(&mut self);

    fn followed_by<T: Interaction>(self, successor: T) -> SuccessiveInteractions<Self, T>
    where
        Self: Sized,
    {
        SuccessiveInteractions {
            first: self,
            second: successor,
        }
    }
}

pub struct SuccessiveInteractions<I1, T2> {
    first: I1,
    second: T2,
}

impl<I1, I2> SuccessiveInteractions<I1, I2> {
    pub fn first(&mut self) -> &mut I1 {
        &mut self.first
    }

    pub fn second(&mut self) -> &mut I2 {
        &mut self.second
    }
}

impl<I1: Interaction, I2: Interaction> Interaction for SuccessiveInteractions<I1, I2> {
    fn process_sample(&mut self, input: f64) -> f64 {
        let output_of_first_interaction = self.first.process_sample(input);
        self.second.process_sample(output_of_first_interaction)
    }

    fn delay_samples(&self) -> f64 {
        self.first.delay_samples() + self.second.delay_samples()
    }

    fn mute(&mut self) {
        self.first.mute();
        self.second.mute();
    }
}

impl Interaction for f64 {
    fn process_sample(&mut self, input: f64) -> f64 {
        *self * input
    }

    fn delay_samples(&self) -> f64 {
        0.0
    }

    fn mute(&mut self) {}
}

#[derive(Default)]
pub struct SoftClip {
    linear_until: f64,
}

impl SoftClip {
    pub fn new(linear_until: f64) -> Self {
        let mut result = Self::default();
        result.set_linear_until(linear_until);
        result
    }

    pub fn set_linear_until(&mut self, linear_until: f64) {
        self.linear_until = linear_until;
    }
}

impl Interaction for SoftClip {
    fn process_sample(&mut self, input: f64) -> f64 {
        let abs_input = input.abs();
        if abs_input <= self.linear_until {
            input
        } else {
            let overshoot = abs_input - self.linear_until;
            (self.linear_until + overshoot / (overshoot / (1.0 - self.linear_until) + 1.0))
                .copysign(input)
        }
    }

    fn delay_samples(&self) -> f64 {
        0.0
    }

    fn mute(&mut self) {}
}

#[derive(Default)]
pub struct OnePoleLowPass {
    damping: f64,
    state: f64,
}

impl OnePoleLowPass {
    pub fn new(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        let mut result = Self::default();
        result.set_cutoff(cutoff_hz, sample_rate_hz);
        result
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f64, sample_rate_hz: f64) {
        // Approximation as described in http://msp.ucsd.edu/techniques/latest/book-html/node140.html.
        self.damping = (1.0 - TAU * cutoff_hz / sample_rate_hz).max(0.0);
    }
}

impl Interaction for OnePoleLowPass {
    fn process_sample(&mut self, input: f64) -> f64 {
        self.state = (1.0 - self.damping) * input + self.damping * self.state;
        self.state
    }

    fn delay_samples(&self) -> f64 {
        // Applying y(n) = (1-d)*x(n) + d*y(n-1) recursively and replacing x(n-m) with m, the total intrinsic delay becomes:
        // D = (1-d)*0 + d*(1-d)*1 + d^2*(1-d)*2 + ... = (1-d) * sum(i=0..infinity, i*d^i) = d / (1-d)
        // http://www-elsa.physik.uni-bonn.de/~dieckman/InfProd/InfProd.html#q-Series
        self.damping / (1.0 - self.damping)
    }

    fn mute(&mut self) {
        self.state = 0.0;
    }
}

pub struct CombFilter<R = f64, L = f64> {
    delay_line: DelayLine,
    response_fn: R,
    limit_fn: L,
}

impl<R: Interaction, L: Interaction> CombFilter<R, L> {
    pub fn new(num_skip_back_samples: usize, response_fn: R, limit_fn: L) -> Self {
        Self {
            delay_line: DelayLine::new(num_skip_back_samples),
            response_fn,
            limit_fn,
        }
    }

    pub fn response_fn(&mut self) -> &mut R {
        &mut self.response_fn
    }

    #[allow(dead_code)] // Keep for future use
    pub fn process_sample(&mut self, input: f64) -> f64 {
        self.delay_line.advance();

        let feedback = self
            .response_fn
            .process_sample(self.delay_line.get_delayed());

        self.delay_line
            .write(self.limit_fn.process_sample(feedback + input));

        feedback
    }

    pub fn process_sample_fract(&mut self, fract_offset: f64, input: f64) -> f64 {
        self.delay_line.advance();

        let feedback = self
            .response_fn
            .process_sample(self.delay_line.get_delayed_fract(fract_offset));

        self.delay_line
            .write(self.limit_fn.process_sample(feedback + input));

        feedback
    }

    pub fn mute(&mut self) {
        self.delay_line.mute();
        self.response_fn.mute();
        self.limit_fn.mute();
    }
}

/// All pass delay as described in https://freeverb3vst.osdn.jp/tips/allpass.shtml.
pub struct AllPassDelay {
    feedback: f64,
    delay_line: DelayLine,
}

impl AllPassDelay {
    pub fn new(num_skip_back_samples: usize, feedback: f64) -> Self {
        Self {
            feedback,
            delay_line: DelayLine::new(num_skip_back_samples),
        }
    }

    pub fn set_feedback(&mut self, feedback: f64) {
        self.feedback = feedback;
    }

    pub fn mute(&mut self) {
        self.delay_line.mute()
    }

    #[allow(dead_code)] // Keep for future use
    pub fn process_sample(&mut self, input: f64) -> f64 {
        self.delay_line.advance();

        let delayed = self.delay_line.get_delayed();
        let sample_to_remember = input + self.feedback * delayed;

        self.delay_line.write(sample_to_remember);

        delayed - sample_to_remember * self.feedback
    }

    pub fn process_sample_fract(&mut self, fract_offset: f64, input: f64) -> f64 {
        self.delay_line.advance();

        let delayed = self.delay_line.get_delayed_fract(fract_offset);
        let sample_to_remember = input + self.feedback * delayed;

        self.delay_line.write(sample_to_remember);

        delayed - sample_to_remember * self.feedback
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn soft_clip_correctness() {
        use assert_approx_eq::assert_approx_eq;

        let mut soft_clip = SoftClip::new(0.9);

        assert_approx_eq!(soft_clip.process_sample(-16.0), -0.999342);
        assert_approx_eq!(soft_clip.process_sample(-8.0), -0.998611);
        assert_approx_eq!(soft_clip.process_sample(-4.0), -0.996875);
        assert_approx_eq!(soft_clip.process_sample(-2.0), -0.991667);
        assert_approx_eq!(soft_clip.process_sample(-1.0), -0.95);
        assert_approx_eq!(soft_clip.process_sample(-0.95), -0.933333);
        assert_approx_eq!(soft_clip.process_sample(-0.901), -0.900990);
        assert_approx_eq!(soft_clip.process_sample(-0.9001), -0.9001);
        assert_approx_eq!(soft_clip.process_sample(-0.9), -0.9);
        assert_approx_eq!(soft_clip.process_sample(-0.45), -0.45);
        assert_approx_eq!(soft_clip.process_sample(0.0), 0.0);
        assert_approx_eq!(soft_clip.process_sample(0.45), 0.45);
        assert_approx_eq!(soft_clip.process_sample(0.9), 0.9);
        assert_approx_eq!(soft_clip.process_sample(0.9001), 0.9001);
        assert_approx_eq!(soft_clip.process_sample(0.901), 0.900990);
        assert_approx_eq!(soft_clip.process_sample(0.95), 0.933333);
        assert_approx_eq!(soft_clip.process_sample(1.0), 0.95);
        assert_approx_eq!(soft_clip.process_sample(2.0), 0.991667);
        assert_approx_eq!(soft_clip.process_sample(4.0), 0.996875);
        assert_approx_eq!(soft_clip.process_sample(8.0), 0.998611);
        assert_approx_eq!(soft_clip.process_sample(16.0), 0.999342);
    }

    #[test]
    fn comb_filter_process_sample() {
        // wavelength = 4, buffer delay = 5
        let mut comb = CombFilter::new(5, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),  //
            (1.0, 0.0),  //
            (0.0, 0.0),  //
            (-1.0, 0.0), //
            (0.0, 0.0),  //
            (1.0, 0.0),  //  0
            (0.0, 1.0),  //  1
            (-1.0, 0.0), //  0
            (0.0, -1.0), // -1
            (1.0, 0.0),  //  0
            (0.0, 1.0),  //  1  0
            (-1.0, 1.0), //  0  1
            (0.0, -1.0), // -1  0
            (1.0, -1.0), //  0 -1
            (0.0, 1.0),  //  1  0
            (-1.0, 1.0), //  0  1  0
            (0.0, 0.0),  // -1  0  1
            (1.0, -1.0), //  0 -1  0
            (0.0, 0.0),  //  1  0 -1
            (-1.0, 1.0), //  0  1  0
            (0.0, 0.0),  // -1  0  1  0
            (1.0, 0.0),  //  0 -1  0  1
            (0.0, 0.0),  //  1  0 -1  0
            (0.0, 0.0),  //  0  1  0 -1
            (0.0, 0.0),  // -1  0  1  0
        ] {
            assert_approx_eq!(comb.process_sample(input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_pos_interference() {
        // wavelength = 4, buffer delay = 4
        let mut comb = CombFilter::new(4, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),   //
            (1.0, 0.0),   //
            (0.0, 0.0),   //
            (-1.0, 0.0),  //
            (0.0, 0.0),   //  0
            (1.0, 1.0),   //  1
            (0.0, 0.0),   //  0
            (-1.0, -1.0), // -1
            (0.0, 0.0),   //  0  0
            (1.0, 2.0),   //  1  1
            (0.0, 0.0),   //  0  0
            (-1.0, -2.0), // -1 -1
            (0.0, 0.0),   //  0  0  0
            (1.0, 3.0),   //  1  1  1
            (0.0, 0.0),   //  0  0  0
            (-1.0, -3.0), // -1 -1 -1
            (0.0, 0.0),   //  0  0  0  0
            (1.0, 4.0),   //  1  1  1  1
            (0.0, 0.0),   //  0  0  0  0
            (-1.0, -4.0), // -1 -1 -1 -1
        ] {
            assert_approx_eq!(comb.process_sample(input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_neg_interference() {
        // wavelength = 8, buffer delay = 4
        let mut comb = CombFilter::new(4, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),  //
            (1.0, 0.0),  //
            (2.0, 0.0),  //
            (1.0, 0.0),  //
            (0.0, 0.0),  //  0
            (-1.0, 1.0), //  1
            (-2.0, 2.0), //  2
            (-1.0, 1.0), //  1
            (0.0, 0.0),  //  0  0
            (1.0, 0.0),  //  1 -1
            (2.0, 0.0),  //  2 -2
            (1.0, 0.0),  //  1 -1
            (0.0, 0.0),  //  0  0  0
            (-1.0, 1.0), //  1 -1  1
            (-2.0, 2.0), //  2 -2  2
            (-1.0, 1.0), //  1 -1  1
            (0.0, 0.0),  //  0  0  0  0
            (1.0, 0.0),  //  1 -1  1 -1
            (2.0, 0.0),  //  2 -2  2 -2
            (1.0, 0.0),  //  1 -1  1 -1
        ] {
            assert_approx_eq!(comb.process_sample(input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_fract() {
        // wavelength = 4, buffer delay = 5 = 1.0*(5-1) + 1
        let mut comb = CombFilter::new(5, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),  //
            (1.0, 0.0),  //
            (0.0, 0.0),  //
            (-1.0, 0.0), //
            (0.0, 0.0),  //
            (1.0, 0.0),  //  0
            (0.0, 1.0),  //  1
            (-1.0, 0.0), //  0
            (0.0, -1.0), // -1
            (1.0, 0.0),  //  0
            (0.0, 1.0),  //  1  0
            (-1.0, 1.0), //  0  1
            (0.0, -1.0), // -1  0
            (1.0, -1.0), //  0 -1
            (0.0, 1.0),  //  1  0
            (-1.0, 1.0), //  0  1  0
            (0.0, 0.0),  // -1  0  1
            (1.0, -1.0), //  0 -1  0
            (0.0, 0.0),  //  1  0 -1
            (-1.0, 1.0), //  0  1  0
            (0.0, 0.0),  // -1  0  1  0
            (1.0, 0.0),  //  0 -1  0  1
            (0.0, 0.0),  //  1  0 -1  0
            (0.0, 0.0),  //  0  1  0 -1
            (0.0, 0.0),  // -1  0  1  0
        ] {
            assert_approx_eq!(comb.process_sample_fract(1.0, input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_fract_pos_interference() {
        // wavelength = 4, buffer delay = 4 = 0.5*8
        let mut comb = CombFilter::new(8, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),   //
            (1.0, 0.0),   //
            (0.0, 0.0),   //
            (-1.0, 0.0),  //
            (0.0, 0.0),   //  0
            (1.0, 1.0),   //  1
            (0.0, 0.0),   //  0
            (-1.0, -1.0), // -1
            (0.0, 0.0),   //  0  0
            (1.0, 2.0),   //  1  1
            (0.0, 0.0),   //  0  0
            (-1.0, -2.0), // -1 -1
            (0.0, 0.0),   //  0  0  0
            (1.0, 3.0),   //  1  1  1
            (0.0, 0.0),   //  0  0  0
            (-1.0, -3.0), // -1 -1 -1
            (0.0, 0.0),   //  0  0  0  0
            (1.0, 4.0),   //  1  1  1  1
            (0.0, 0.0),   //  0  0  0  0
            (-1.0, -4.0), // -1 -1 -1 -1
        ] {
            assert_approx_eq!(comb.process_sample_fract(0.5, input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_fract_neg_interference() {
        // wavelength = 8, buffer delay = 4 = 0.5*8
        let mut comb = CombFilter::new(8, 1.0, 1.0);
        for &(input, output) in &[
            (0.0, 0.0),  //
            (1.0, 0.0),  //
            (2.0, 0.0),  //
            (1.0, 0.0),  //
            (0.0, 0.0),  //  0
            (-1.0, 1.0), //  1
            (-2.0, 2.0), //  2
            (-1.0, 1.0), //  1
            (0.0, 0.0),  //  0  0
            (1.0, 0.0),  //  1 -1
            (2.0, 0.0),  //  2 -2
            (1.0, 0.0),  //  1 -1
            (0.0, 0.0),  //  0  0  0
            (-1.0, 1.0), //  1 -1  1
            (-2.0, 2.0), //  2 -2  2
            (-1.0, 1.0), //  1 -1  1
            (0.0, 0.0),  //  0  0  0  0
            (1.0, 0.0),  //  1 -1  1 -1
            (2.0, 0.0),  //  2 -2  2 -2
            (1.0, 0.0),  //  1 -1  1 -1
        ] {
            assert_approx_eq!(comb.process_sample_fract(0.5, input), output);
        }
    }
}
