use std::f32::consts::TAU;

pub trait Interpolate {
    fn interpolate(left: Self, right: Self, interpolation: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(left: Self, right: Self, interpolation: f32) -> Self {
        (1.0 - interpolation) * left + interpolation * right
    }
}

impl Interpolate for (f32, f32) {
    fn interpolate(left: Self, right: Self, interpolation: f32) -> Self {
        (
            (1.0 - interpolation) * left.0 + interpolation * right.0,
            (1.0 - interpolation) * left.1 + interpolation * right.1,
        )
    }
}

pub struct DelayLine<T = f32> {
    buffer: Vec<T>,
    position: usize,
}

impl<T: Copy + Default> DelayLine<T> {
    pub fn new(num_samples_in_buffer: usize) -> Self {
        Self {
            buffer: vec![Default::default(); num_samples_in_buffer],
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
        self.buffer.get(self.position).copied().unwrap_or_default()
    }

    pub fn get_delayed_fract(&self, fract_offset: f32) -> T
    where
        T: Interpolate,
    {
        let offset = (self.buffer.len() - 1) as f32 * fract_offset;
        let interpolation = offset.ceil() - offset;

        let position = self.position + self.buffer.len() - offset.ceil() as usize - 1;
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
    fn process_sample(&mut self, input: f32) -> f32;

    fn mute(&mut self);
}

impl FeedbackFn for () {
    fn process_sample(&mut self, input: f32) -> f32 {
        input
    }

    fn mute(&mut self) {}
}

pub struct OnePoleLowPass {
    damping: f32,
    state: f32,
}

impl OnePoleLowPass {
    pub fn new(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        // Approximation as described in http://msp.ucsd.edu/techniques/latest/book-html/node140.html.
        let damping = (1.0 - TAU * cutoff_hz / sample_rate_hz).max(0.0);

        Self {
            damping,
            state: 0.0,
        }
    }
}

impl FeedbackFn for OnePoleLowPass {
    fn process_sample(&mut self, input: f32) -> f32 {
        self.state = (1.0 - self.damping) * input + self.damping * self.state;
        self.state
    }

    fn mute(&mut self) {
        self.state = 0.0;
    }
}

pub struct CombFilter<FB = ()> {
    feedback: f32,
    feedback_fn: FB,
    delay_line: DelayLine,
}

impl<FB: FeedbackFn> CombFilter<FB> {
    pub fn new(num_samples_in_buffer: usize, feedback: f32, feedback_fn: FB) -> Self {
        Self {
            feedback,
            feedback_fn,
            delay_line: DelayLine::new(num_samples_in_buffer),
        }
    }

    pub fn mute(&mut self) {
        self.delay_line.mute();
        self.feedback_fn.mute();
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let echo = self
            .feedback_fn
            .process_sample(self.delay_line.get_delayed());
        let feedback = self.feedback * echo;
        self.delay_line.store_delayed(feedback + input);
        feedback
    }
}

/// All pass delay as described in https://freeverb3vst.osdn.jp/tips/allpass.shtml.
pub struct AllPassDelay {
    feedback: f32,
    delay_line: DelayLine,
}

impl AllPassDelay {
    pub fn new(num_samples_in_buffer: usize, feedback: f32) -> Self {
        Self {
            feedback,
            delay_line: DelayLine::new(num_samples_in_buffer),
        }
    }

    pub fn mute(&mut self) {
        self.delay_line.mute()
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delayed = self.delay_line.get_delayed();
        let sample_to_remember = input + self.feedback * delayed;
        self.delay_line.store_delayed(sample_to_remember);
        delayed - sample_to_remember * self.feedback
    }
}
