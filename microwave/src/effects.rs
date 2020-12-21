use std::{cmp::Ordering, f32::consts::TAU};

pub struct Delay {
    rot_l_l: f32,
    rot_r_l: f32,
    buffer: Vec<(f32, f32)>,
    position: usize,
}

pub struct DelayOptions {
    pub delay_time_in_s: f32,
    pub feedback_intensity: f32,
    pub feedback_rotation: f32,
}

impl Delay {
    pub fn new(options: DelayOptions, sample_rate_in_hz: f32) -> Self {
        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (options.feedback_rotation / 2.0).sin_cos();

        let num_samples_in_buffer = (options.delay_time_in_s * sample_rate_in_hz).round() as usize;

        Self {
            rot_l_l: cos * options.feedback_intensity,
            rot_r_l: sin * options.feedback_intensity,
            buffer: vec![(0.0, 0.0); num_samples_in_buffer],
            position: 0,
        }
    }

    pub fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = (0.0, 0.0))
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        // A mathematically positive rotation around the l x r axis is perceived as a clockwise rotation
        let rot_l_l = self.rot_l_l;
        let rot_r_l = self.rot_r_l;
        let rot_l_r = -self.rot_r_l;
        let rot_r_r = self.rot_l_l;

        for signal_sample in signal.chunks_mut(2) {
            let delayed_sample = self.buffer.get_mut(self.position);

            if let ([signal_l, signal_r], Some((delayed_l, delayed_r))) =
                (signal_sample, delayed_sample)
            {
                *signal_l += rot_l_l * *delayed_l + rot_l_r * *delayed_r;
                *signal_r += rot_r_l * *delayed_l + rot_r_r * *delayed_r;

                *delayed_l = *signal_l;
                *delayed_r = *signal_r;
            }

            self.position += 1;
            self.position %= self.buffer.len();
        }
    }
}

pub struct Rotary {
    options: RotaryOptions,
    buffer: Buffer,
    curr_angle: f32,
    curr_rotation_in_hz: f32,
    target_rotation_in_hz: f32,
    sample_rate_in_hz: f32,
}

pub struct RotaryOptions {
    pub rotation_radius_in_cm: f32,
    pub min_frequency_in_hz: f32,
    pub max_frequency_in_hz: f32,
    pub acceleration_time_in_s: f32,
    pub deceleration_time_in_s: f32,
}

impl Rotary {
    const SPEED_OF_SOUND_IN_CM_PER_S: f32 = 34320.0;

    pub fn new(options: RotaryOptions, sample_rate_in_hz: f32) -> Self {
        let delay_span = 2.0 * options.rotation_radius_in_cm / Self::SPEED_OF_SOUND_IN_CM_PER_S;
        let num_samples_in_buffer = (delay_span * sample_rate_in_hz) as usize + 1;

        let curr_rotation_in_hz = options.min_frequency_in_hz;
        let target_rotation_in_hz = options.min_frequency_in_hz;

        Self {
            options,
            buffer: Buffer {
                values: vec![(0.0, 0.0); num_samples_in_buffer],
                position: 0,
                dummy: (0.0, 0.0),
            },
            curr_angle: 0.0,
            curr_rotation_in_hz,
            target_rotation_in_hz,
            sample_rate_in_hz,
        }
    }

    pub fn mute(&mut self) {
        self.buffer.values.iter_mut().for_each(|e| *e = (0.0, 0.0))
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        let frequency_width = self.options.max_frequency_in_hz - self.options.min_frequency_in_hz;

        let (acceleration, lower_limit, upper_limit) = match self
            .curr_rotation_in_hz
            .partial_cmp(&self.target_rotation_in_hz)
        {
            Some(Ordering::Less) => (
                frequency_width / self.options.acceleration_time_in_s,
                self.options.min_frequency_in_hz,
                self.target_rotation_in_hz,
            ),
            Some(Ordering::Greater) => (
                -frequency_width / self.options.deceleration_time_in_s,
                self.target_rotation_in_hz,
                self.options.max_frequency_in_hz,
            ),
            _ => (
                0.0,
                self.options.min_frequency_in_hz,
                self.options.max_frequency_in_hz,
            ),
        };

        for signal_sample in signal.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                *self.buffer.get_mut() = (*signal_l, *signal_r);

                let left_offset = 0.5 + 0.5 * self.curr_angle.sin();
                let right_offset = 1.0 - left_offset;

                let l = self.buffer.get(left_offset).0;
                let r = self.buffer.get(right_offset).1;

                *signal_l = (*signal_l + l) / 2.0;
                *signal_r = (*signal_r + r) / 2.0;

                self.curr_rotation_in_hz = (self.curr_rotation_in_hz
                    + acceleration / self.sample_rate_in_hz)
                    .max(lower_limit)
                    .min(upper_limit);

                self.curr_angle = (self.curr_angle
                    + self.curr_rotation_in_hz / self.sample_rate_in_hz * TAU)
                    .rem_euclid(TAU);

                self.buffer.next_sample();
            }
        }
    }

    pub fn set_motor_voltage(&mut self, motor_voltage: f32) {
        self.target_rotation_in_hz = self.options.min_frequency_in_hz
            + motor_voltage * (self.options.max_frequency_in_hz - self.options.min_frequency_in_hz);
    }
}

struct Buffer {
    values: Vec<(f32, f32)>,
    position: usize,
    dummy: (f32, f32), // To avoid panicking
}

impl Buffer {
    fn next_sample(&mut self) {
        self.position += 1;
        self.position %= self.values.len();
    }

    fn get_mut(&mut self) -> &mut (f32, f32) {
        match self.values.get_mut(self.position) {
            Some(value) => value,
            None => {
                self.dummy = (0.0, 0.0);
                &mut self.dummy
            }
        }
    }

    fn get(&self, normalized_offset: f32) -> (f32, f32) {
        let offset = (self.values.len() - 1) as f32 * normalized_offset;
        let position = self.position + self.values.len() - offset.ceil() as usize;

        if let (Some((delayed_l_1, delayed_r_1)), Some((delayed_l_2, delayed_r_2))) = (
            self.values.get(position % self.values.len()),
            self.values.get((position + 1) % self.values.len()),
        ) {
            let interpolation = offset.ceil() - offset;
            (
                *delayed_l_1 * (1.0 - interpolation) + *delayed_l_2 * interpolation,
                *delayed_r_1 * (1.0 - interpolation) + *delayed_r_2 * interpolation,
            )
        } else {
            (0.0, 0.0)
        }
    }
}

pub struct SchroederReverb {
    allpass_filters: Vec<(AllPassDelay, AllPassDelay)>,
    comb_filters: Vec<(LowPassDelay, LowPassDelay)>,
    wetness: f32,
}

pub struct ReverbOptions {
    pub allpasses_ms: Vec<f32>,
    pub allpass_feedback: f32,
    pub combs_ms: Vec<f32>,
    pub comb_feedback: f32,
    pub stereo_ms: f32,
    pub cutoff_hz: f32,
    pub wetness: f32,
}

impl SchroederReverb {
    pub fn new(options: ReverbOptions, sample_rate_in_hz: f32) -> Self {
        let allpass_filters = options
            .allpasses_ms
            .iter()
            .map(|delay_ms| {
                let delay_samples = (delay_ms / 1000.0 * sample_rate_in_hz).round() as usize;
                (
                    AllPassDelay::new(delay_samples, options.allpass_feedback),
                    AllPassDelay::new(delay_samples, options.allpass_feedback),
                )
            })
            .collect();

        // Approximation as described in http://msp.ucsd.edu/techniques/latest/book-html/node140.html.
        let damping = (1.0 - TAU * options.cutoff_hz / sample_rate_in_hz).max(0.0);
        let stereo_offset = options.stereo_ms / 1000.0 * sample_rate_in_hz;

        let comb_filters = options
            .combs_ms
            .iter()
            .map(|delay_ms| {
                let delay_samples = delay_ms / 1000.0 * sample_rate_in_hz;
                (
                    LowPassDelay::new(
                        delay_samples.round() as usize,
                        damping,
                        options.comb_feedback,
                    ),
                    LowPassDelay::new(
                        (delay_samples + stereo_offset).round() as usize,
                        damping,
                        options.comb_feedback,
                    ),
                )
            })
            .collect();

        Self {
            allpass_filters,
            comb_filters,
            wetness: options.wetness,
        }
    }

    pub fn mute(&mut self) {
        for allpass in &mut self.allpass_filters {
            allpass.0.mute();
            allpass.1.mute();
        }
        for comb in &mut self.comb_filters {
            comb.0.mute();
            comb.1.mute();
        }
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        for signal_sample in signal.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                let mut reverbed_l = 0.0;
                let mut reverbed_r = 0.0;

                for (comb_l, comb_r) in &mut self.comb_filters {
                    reverbed_l += comb_l.process_sample(*signal_l);
                    reverbed_r += comb_r.process_sample(*signal_r);
                }

                for (allpass_l, allpass_r) in &mut self.allpass_filters {
                    reverbed_l = allpass_l.process_sample(reverbed_l);
                    reverbed_r = allpass_r.process_sample(reverbed_r);
                }

                let normalization = self.comb_filters.len() as f32;
                reverbed_l /= normalization;
                reverbed_r /= normalization;

                *signal_l = (1.0 - self.wetness) * *signal_l + self.wetness * reverbed_l;
                *signal_r = (1.0 - self.wetness) * *signal_r + self.wetness * reverbed_r;
            }
        }
    }
}

/// All pass delay as described in https://freeverb3vst.osdn.jp/tips/allpass.shtml.
struct AllPassDelay {
    feedback: f32,
    buffer: Vec<f32>,
    position: usize,
}

impl AllPassDelay {
    fn new(len: usize, feedback: f32) -> Self {
        Self {
            feedback,
            buffer: vec![0.0; len],
            position: 0,
        }
    }

    fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = 0.0);
    }

    fn process_sample(&mut self, input: f32) -> f32 {
        let buffer_len = self.buffer.len();
        if let Some(delayed_output) = self.buffer.get_mut(self.position) {
            let old_delayed_output = *delayed_output;
            *delayed_output = input + self.feedback * *delayed_output;
            self.position = (self.position + 1) % buffer_len;
            old_delayed_output - *delayed_output * self.feedback
        } else {
            0.0
        }
    }
}

struct LowPassDelay {
    feedback: f32,
    damping: f32,
    buffer: Vec<f32>,
    low_pass_state: f32,
    position: usize,
}

impl LowPassDelay {
    fn new(len: usize, damping: f32, feedback: f32) -> Self {
        Self {
            feedback,
            damping,
            buffer: vec![0.0; len],
            low_pass_state: 0.0,
            position: 0,
        }
    }

    fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = 0.0);
        self.low_pass_state = 0.0;
    }

    fn process_sample(&mut self, sample: f32) -> f32 {
        if let Some(delayed) = self.buffer.get_mut(self.position) {
            self.low_pass_state =
                (1.0 - self.damping) * *delayed + self.damping * self.low_pass_state;

            let out = self.feedback * self.low_pass_state;

            *delayed = sample + out;

            self.position = (self.position + 1) % self.buffer.len();

            out
        } else {
            0.0
        }
    }
}
