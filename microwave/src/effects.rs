use std::{cmp::Ordering, f32::consts::TAU};

pub struct Delay {
    options: DelayOptions,
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
        let num_samples_in_buffer = (options.delay_time_in_s * sample_rate_in_hz).round() as usize;
        Self {
            options,
            buffer: vec![(0.0, 0.0); num_samples_in_buffer],
            position: 0,
        }
    }

    pub fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = (0.0, 0.0))
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (self.options.feedback_rotation / 2.0).sin_cos();

        // A mathematically positive rotation around the l x r axis is perceived as a clockwise rotation
        let rot_l_l = cos * self.options.feedback_intensity;
        let rot_r_l = sin * self.options.feedback_intensity;
        let rot_l_r = -rot_r_l;
        let rot_r_r = rot_l_l;

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
