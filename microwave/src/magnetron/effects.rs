use std::{cmp::Ordering, f64::consts::TAU};

use super::util::{
    AllPassDelay, CombFilter, DelayLine, Interaction, OnePoleLowPass, SuccessiveInteractions,
};

pub struct Delay {
    rot_l_l: f64,
    rot_r_l: f64,
    delay_line: DelayLine<(f64, f64)>,
}

pub struct DelayOptions {
    pub delay_time_in_s: f64,
    pub feedback_intensity: f64,
    pub feedback_rotation: f64,
}

impl Delay {
    pub fn new(options: DelayOptions, sample_rate_in_hz: f64) -> Self {
        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (options.feedback_rotation / 2.0).sin_cos();

        let num_samples_in_buffer = (options.delay_time_in_s * sample_rate_in_hz).round() as usize;

        Self {
            rot_l_l: cos * options.feedback_intensity,
            rot_r_l: sin * options.feedback_intensity,
            delay_line: DelayLine::new(num_samples_in_buffer),
        }
    }

    pub fn mute(&mut self) {
        self.delay_line.mute()
    }

    pub fn process(&mut self, signal: &mut [f64]) {
        // A mathematically positive rotation around the l x r axis is perceived as a clockwise rotation
        let rot_l_l = self.rot_l_l;
        let rot_r_l = self.rot_r_l;
        let rot_l_r = -self.rot_r_l;
        let rot_r_r = self.rot_l_l;

        for signal_sample in signal.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                let delayed = self.delay_line.get_delayed();

                *signal_l += rot_l_l * delayed.0 + rot_l_r * delayed.1;
                *signal_r += rot_r_l * delayed.0 + rot_r_r * delayed.1;

                self.delay_line.store_delayed((*signal_l, *signal_r));
            }
        }
    }
}

pub struct Rotary {
    options: RotaryOptions,
    delay_line: DelayLine<(f64, f64)>,
    curr_angle: f64,
    curr_rotation_in_hz: f64,
    target_rotation_in_hz: f64,
    sample_rate_in_hz: f64,
}

pub struct RotaryOptions {
    pub rotation_radius_in_cm: f64,
    pub min_frequency_in_hz: f64,
    pub max_frequency_in_hz: f64,
    pub acceleration_time_in_s: f64,
    pub deceleration_time_in_s: f64,
}

impl Rotary {
    const SPEED_OF_SOUND_IN_CM_PER_S: f64 = 34320.0;

    pub fn new(options: RotaryOptions, sample_rate_in_hz: f64) -> Self {
        let delay_span = 2.0 * options.rotation_radius_in_cm / Self::SPEED_OF_SOUND_IN_CM_PER_S;
        let num_samples_in_buffer = (delay_span * sample_rate_in_hz) as usize + 1;

        let curr_rotation_in_hz = options.min_frequency_in_hz;
        let target_rotation_in_hz = options.min_frequency_in_hz;

        Self {
            options,
            delay_line: DelayLine::new(num_samples_in_buffer),
            curr_angle: 0.0,
            curr_rotation_in_hz,
            target_rotation_in_hz,
            sample_rate_in_hz,
        }
    }

    pub fn mute(&mut self) {
        self.delay_line.mute();
    }

    pub fn process(&mut self, signal: &mut [f64]) {
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
                let left_offset = 0.5 + 0.5 * self.curr_angle.sin();
                let right_offset = 1.0 - left_offset;

                self.delay_line.store_delayed((*signal_l, *signal_r));

                let delayed_l = self.delay_line.get_delayed_fract(left_offset).0;
                let delayed_r = self.delay_line.get_delayed_fract(right_offset).1;

                *signal_l = (*signal_l + delayed_l) / 2.0;
                *signal_r = (*signal_r + delayed_r) / 2.0;

                self.curr_rotation_in_hz = (self.curr_rotation_in_hz
                    + acceleration / self.sample_rate_in_hz)
                    .max(lower_limit)
                    .min(upper_limit);

                self.curr_angle = (self.curr_angle
                    + self.curr_rotation_in_hz / self.sample_rate_in_hz * TAU)
                    .rem_euclid(TAU);
            }
        }
    }

    pub fn set_motor_voltage(&mut self, motor_voltage: f64) {
        self.target_rotation_in_hz = self.options.min_frequency_in_hz
            + motor_voltage * (self.options.max_frequency_in_hz - self.options.min_frequency_in_hz);
    }
}

type LowPassCombFilter = CombFilter<SuccessiveInteractions<OnePoleLowPass, f64>>;

pub struct SchroederReverb {
    allpass_filters: Vec<(AllPassDelay, AllPassDelay)>,
    comb_filters: Vec<(LowPassCombFilter, LowPassCombFilter)>,
    wetness: f64,
}

pub struct ReverbOptions {
    pub allpasses_ms: Vec<f64>,
    pub allpass_feedback: f64,
    pub combs_ms: Vec<f64>,
    pub comb_feedback: f64,
    pub stereo_ms: f64,
    pub cutoff_hz: f64,
    pub wetness: f64,
}

impl SchroederReverb {
    pub fn new(options: ReverbOptions, sample_rate_hz: f64) -> Self {
        let allpass_filters = options
            .allpasses_ms
            .iter()
            .map(|delay_ms| {
                let delay_samples = (delay_ms / 1000.0 * sample_rate_hz).round() as usize;
                (
                    AllPassDelay::new(delay_samples, options.allpass_feedback),
                    AllPassDelay::new(delay_samples, options.allpass_feedback),
                )
            })
            .collect();

        let stereo_offset = options.stereo_ms / 1000.0 * sample_rate_hz;

        let comb_filters = options
            .combs_ms
            .iter()
            .map(|delay_ms| {
                let delay_samples = delay_ms / 1000.0 * sample_rate_hz;
                (
                    CombFilter::new(
                        delay_samples.round() as usize,
                        OnePoleLowPass::new(options.cutoff_hz, sample_rate_hz)
                            .followed_by(options.comb_feedback),
                        1.0,
                    ),
                    CombFilter::new(
                        (delay_samples + stereo_offset).round() as usize,
                        OnePoleLowPass::new(options.cutoff_hz, sample_rate_hz)
                            .followed_by(options.comb_feedback),
                        1.0,
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

    pub fn process(&mut self, signal: &mut [f64]) {
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

                let normalization = self.comb_filters.len() as f64;
                reverbed_l /= normalization;
                reverbed_r /= normalization;

                *signal_l = (1.0 - self.wetness) * *signal_l + self.wetness * reverbed_l;
                *signal_r = (1.0 - self.wetness) * *signal_r + self.wetness * reverbed_r;
            }
        }
    }
}
