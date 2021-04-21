use std::f64::consts::TAU;

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
            (1.0 - interpolation) * left.0 + interpolation * right.0,
            (1.0 - interpolation) * left.1 + interpolation * right.1,
        )
    }
}

pub struct DelayLine<T = f64> {
    buffer: Vec<T>,
    position: usize,
}

impl<T: Copy + Default> DelayLine<T> {
    pub fn new(num_skip_back_samples: usize) -> Self {
        Self {
            buffer: vec![Default::default(); num_skip_back_samples + 1],
            position: 0,
        }
    }

    pub fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = Default::default());
    }

    pub fn store_delayed(&mut self, delayed: T) {
        if let Some(value) = self.buffer.get_mut(self.position) {
            *value = delayed
        }
        self.position = (self.position + 1) % self.buffer.len();
    }

    pub fn get_delayed(&self) -> T {
        self.buffer
            .get((self.position + 1) % self.buffer.len())
            .copied()
            .unwrap_or_default()
    }

    pub fn get_delayed_fract(&self, fract_offset: f64) -> T
    where
        T: Interpolate,
    {
        let offset = (self.buffer.len() - 1) as f64 * fract_offset;
        let interpolation = offset.ceil() - offset;

        let position = self.position + self.buffer.len() - offset.ceil() as usize;
        let delayed_1 = self.get(position);
        let delayed_2 = self.get(position + 1);

        T::interpolate(delayed_1, delayed_2, interpolation)
    }

    fn get(&self, position: usize) -> T {
        self.buffer
            .get(position % self.buffer.len())
            .copied()
            .unwrap_or_default()
    }
}

pub trait FeedbackFn {
    fn process_sample(&mut self, input: f64) -> f64;

    fn mute(&mut self);
}

impl FeedbackFn for () {
    fn process_sample(&mut self, input: f64) -> f64 {
        input
    }

    fn mute(&mut self) {}
}

pub struct OnePoleLowPass {
    damping: f64,
    state: f64,
}

impl OnePoleLowPass {
    pub fn new(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        // Approximation as described in http://msp.ucsd.edu/techniques/latest/book-html/node140.html.
        let damping = (1.0 - TAU * cutoff_hz / sample_rate_hz).max(0.0);

        Self {
            damping,
            state: 0.0,
        }
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f64, sample_rate_hz: f64) {
        self.damping = (1.0 - TAU * cutoff_hz / sample_rate_hz).max(0.0);
    }

    /// Returns the intrinsic delay of the filter.
    pub fn intrinsic_delay_samples(&self) -> f64 {
        // Applying y(n) = (1-d)*x(n) + d*y(n-1) recursively and replacing x(n-m) with m, the total intrinsic delay becomes:
        // D = (1-d)*0 + d*(1-d)*1 + d^2*(1-d)*2 + ... = (1-d) * sum(i=0..infinity, i*d^i) = d / (1-d)
        // http://www-elsa.physik.uni-bonn.de/~dieckman/InfProd/InfProd.html#q-Series
        self.damping / (1.0 - self.damping)
    }
}

impl FeedbackFn for OnePoleLowPass {
    fn process_sample(&mut self, input: f64) -> f64 {
        self.state = (1.0 - self.damping) * input + self.damping * self.state;
        self.state
    }

    fn mute(&mut self) {
        self.state = 0.0;
    }
}

pub struct CombFilter<FB = ()> {
    feedback: f64,
    feedback_fn: FB,
    delay_line: DelayLine,
}

impl<FB: FeedbackFn> CombFilter<FB> {
    pub fn new(num_skip_back_samples: usize, feedback: f64, feedback_fn: FB) -> Self {
        Self {
            feedback,
            feedback_fn,
            delay_line: DelayLine::new(num_skip_back_samples),
        }
    }

    pub fn mute(&mut self) {
        self.delay_line.mute();
        self.feedback_fn.mute();
    }

    pub fn set_feedback(&mut self, feedback: f64) {
        self.feedback = feedback;
    }

    pub fn feedback_fn(&mut self) -> &mut FB {
        &mut self.feedback_fn
    }

    pub fn process_sample(&mut self, input: f64) -> f64 {
        let echo = self
            .feedback_fn
            .process_sample(self.delay_line.get_delayed());
        let feedback = self.feedback * echo;
        self.delay_line.store_delayed(feedback + input);
        feedback
    }

    pub fn process_sample_fract(&mut self, fract_offset: f64, input: f64) -> f64 {
        let echo = self
            .feedback_fn
            .process_sample(self.delay_line.get_delayed_fract(fract_offset));
        let feedback = self.feedback * echo;
        self.delay_line.store_delayed(feedback + input);
        feedback
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

    pub fn mute(&mut self) {
        self.delay_line.mute()
    }

    pub fn process_sample(&mut self, input: f64) -> f64 {
        let delayed = self.delay_line.get_delayed();
        let sample_to_remember = input + self.feedback * delayed;
        self.delay_line.store_delayed(sample_to_remember);
        delayed - sample_to_remember * self.feedback
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn comb_filter_process_sample() {
        // wavelength = 4, buffer delay = 5
        let mut comb = CombFilter::new(5, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),  //
            (0.1, 0.0),  //
            (0.0, 0.0),  //
            (-0.1, 0.0), //
            (0.0, 0.0),  //
            (0.1, 0.0),  //  0
            (0.0, 0.1),  //  1
            (-0.1, 0.0), //  0
            (0.0, -0.1), // -1
            (0.1, 0.0),  //  0
            (0.0, 0.1),  //  1  0
            (-0.1, 0.1), //  0  1
            (0.0, -0.1), // -1  0
            (0.1, -0.1), //  0 -1
            (0.0, 0.1),  //  1  0
            (-0.1, 0.1), //  0  1  0
            (0.0, 0.0),  // -1  0  1
            (0.1, -0.1), //  0 -1  0
            (0.0, 0.0),  //  1  0 -1
            (-0.1, 0.1), //  0  1  0
            (0.0, 0.0),  // -1  0  1  0
            (0.1, 0.0),  //  0 -1  0  1
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
        let mut comb = CombFilter::new(4, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),   //
            (0.1, 0.0),   //
            (0.0, 0.0),   //
            (-0.1, 0.0),  //
            (0.0, 0.0),   //  0
            (0.1, 0.1),   //  1
            (0.0, 0.0),   //  0
            (-0.1, -0.1), // -1
            (0.0, 0.0),   //  0  0
            (0.1, 0.2),   //  1  1
            (0.0, 0.0),   //  0  0
            (-0.1, -0.2), // -1 -1
            (0.0, 0.0),   //  0  0  0
            (0.1, 0.3),   //  1  1  1
            (0.0, 0.0),   //  0  0  0
            (-0.1, -0.3), // -1 -1 -1
            (0.0, 0.0),   //  0  0  0  0
            (0.1, 0.4),   //  1  1  1  1
            (0.0, 0.0),   //  0  0  0  0
            (-0.1, -0.4), // -1 -1 -1 -1
        ] {
            assert_approx_eq!(comb.process_sample(input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_neg_interference() {
        // wavelength = 8, buffer delay = 4
        let mut comb = CombFilter::new(4, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),  //
            (0.1, 0.0),  //
            (0.2, 0.0),  //
            (0.1, 0.0),  //
            (0.0, 0.0),  //  0
            (-0.1, 0.1), //  1
            (-0.2, 0.2), //  2
            (-0.1, 0.1), //  1
            (0.0, 0.0),  //  0  0
            (0.1, 0.0),  //  1 -1
            (0.2, 0.0),  //  2 -2
            (0.1, 0.0),  //  1 -1
            (0.0, 0.0),  //  0  0  0
            (-0.1, 0.1), //  1 -1  1
            (-0.2, 0.2), //  2 -2  2
            (-0.1, 0.1), //  1 -1  1
            (0.0, 0.0),  //  0  0  0  0
            (0.1, 0.0),  //  1 -1  1 -1
            (0.2, 0.0),  //  2 -2  2 -2
            (0.1, 0.0),  //  1 -1  1 -1
        ] {
            assert_approx_eq!(comb.process_sample(input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_fract() {
        // wavelength = 4, buffer delay = 5 = 1.0*(5-1) + 1
        let mut comb = CombFilter::new(5, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),  //
            (0.1, 0.0),  //
            (0.0, 0.0),  //
            (-0.1, 0.0), //
            (0.0, 0.0),  //
            (0.1, 0.0),  //  0
            (0.0, 0.1),  //  1
            (-0.1, 0.0), //  0
            (0.0, -0.1), // -1
            (0.1, 0.0),  //  0
            (0.0, 0.1),  //  1  0
            (-0.1, 0.1), //  0  1
            (0.0, -0.1), // -1  0
            (0.1, -0.1), //  0 -1
            (0.0, 0.1),  //  1  0
            (-0.1, 0.1), //  0  1  0
            (0.0, 0.0),  // -1  0  1
            (0.1, -0.1), //  0 -1  0
            (0.0, 0.0),  //  1  0 -1
            (-0.1, 0.1), //  0  1  0
            (0.0, 0.0),  // -1  0  1  0
            (0.1, 0.0),  //  0 -1  0  1
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
        let mut comb = CombFilter::new(8, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),   //
            (0.1, 0.0),   //
            (0.0, 0.0),   //
            (-0.1, 0.0),  //
            (0.0, 0.0),   //  0
            (0.1, 0.1),   //  1
            (0.0, 0.0),   //  0
            (-0.1, -0.1), // -1
            (0.0, 0.0),   //  0  0
            (0.1, 0.2),   //  1  1
            (0.0, 0.0),   //  0  0
            (-0.1, -0.2), // -1 -1
            (0.0, 0.0),   //  0  0  0
            (0.1, 0.3),   //  1  1  1
            (0.0, 0.0),   //  0  0  0
            (-0.1, -0.3), // -1 -1 -1
            (0.0, 0.0),   //  0  0  0  0
            (0.1, 0.4),   //  1  1  1  1
            (0.0, 0.0),   //  0  0  0  0
            (-0.1, -0.4), // -1 -1 -1 -1
        ] {
            assert_approx_eq!(comb.process_sample_fract(0.5, input), output);
        }
    }

    #[test]
    fn comb_filter_process_sample_fract_neg_interference() {
        // wavelength = 8, buffer delay = 4 = 0.5*8
        let mut comb = CombFilter::new(8, 1.0, ());
        for &(input, output) in &[
            (0.0, 0.0),  //
            (0.1, 0.0),  //
            (0.2, 0.0),  //
            (0.1, 0.0),  //
            (0.0, 0.0),  //  0
            (-0.1, 0.1), //  1
            (-0.2, 0.2), //  2
            (-0.1, 0.1), //  1
            (0.0, 0.0),  //  0  0
            (0.1, 0.0),  //  1 -1
            (0.2, 0.0),  //  2 -2
            (0.1, 0.0),  //  1 -1
            (0.0, 0.0),  //  0  0  0
            (-0.1, 0.1), //  1 -1  1
            (-0.2, 0.2), //  2 -2  2
            (-0.1, 0.1), //  1 -1  1
            (0.0, 0.0),  //  0  0  0  0
            (0.1, 0.0),  //  1 -1  1 -1
            (0.2, 0.0),  //  2 -2  2 -2
            (0.1, 0.0),  //  1 -1  1 -1
        ] {
            assert_approx_eq!(comb.process_sample_fract(0.5, input), output);
        }
    }
}
