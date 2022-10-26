use std::{cmp::Ordering, f64::consts::TAU};

use serde::{Deserialize, Serialize};

use crate::{
    audio::AudioStage,
    control::{LiveParameter, LiveParameterStorage},
};

use super::util::{
    AllPassDelay, CombFilter, DelayLine, Interaction, OnePoleLowPass, SuccessiveInteractions,
};

#[derive(Deserialize, Serialize)]
pub enum EffectSpec {
    Echo(EchoSpec),
    SchroederReverb(SchroederReverbSpec),
    RotarySpeaker(RotarySpeakerSpec),
}

impl EffectSpec {
    pub fn create(&self, sample_rate_hz: f64) -> Box<dyn AudioStage> {
        match self {
            EffectSpec::Echo(spec) => Box::new(spec.create(sample_rate_hz)),
            EffectSpec::SchroederReverb(spec) => Box::new(spec.create(sample_rate_hz)),
            EffectSpec::RotarySpeaker(spec) => Box::new(spec.create(sample_rate_hz)),
        }
    }
}

/// A recursive delay with channel-rotated feedback
#[derive(Deserialize, Serialize)]
pub struct EchoSpec {
    pub gain_controller: LiveParameter,

    /// Delay time (s)
    pub delay_time: f64,

    /// Delay feedback
    pub feedback: f64,

    /// Delay feedback rotation angle (degrees clock-wise)
    pub feedback_rotation: f64,
}

impl EchoSpec {
    fn create(&self, sample_rate_hz: f64) -> Echo {
        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (self.feedback_rotation / 2.0).sin_cos();

        let num_samples_in_buffer = (self.delay_time * sample_rate_hz).round() as usize;

        Echo {
            gain_controller: self.gain_controller,
            rot_l_l: cos * self.feedback,
            rot_r_l: sin * self.feedback,
            delay_line: DelayLine::new(num_samples_in_buffer),
        }
    }
}

struct Echo {
    gain_controller: LiveParameter,
    rot_l_l: f64,
    rot_r_l: f64,
    delay_line: DelayLine<(f64, f64)>,
}

impl AudioStage for Echo {
    fn render(&mut self, signal: &mut [f64], storage: &LiveParameterStorage) {
        let gain = storage.read_parameter(self.gain_controller);

        // A mathematically positive rotation around the l x r axis is perceived as a clockwise rotation
        let rot_l_l = self.rot_l_l;
        let rot_r_l = self.rot_r_l;
        let rot_l_r = -self.rot_r_l;
        let rot_r_r = self.rot_l_l;

        for signal_sample in signal.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                self.delay_line.advance();

                let delayed = self.delay_line.get_delayed();

                let feedback_l = rot_l_l * delayed.0 + rot_l_r * delayed.1;
                let feedback_r = rot_r_l * delayed.0 + rot_r_r * delayed.1;

                self.delay_line
                    .write((feedback_l + gain * *signal_l, feedback_r + gain * *signal_r));

                *signal_l += feedback_l;
                *signal_r += feedback_r;
            }
        }
    }

    fn mute(&mut self) {
        self.delay_line.mute()
    }
}

/// A combination of multiple allpass filters in series, followed by multiple comb filters in parallel
#[derive(Deserialize, Serialize)]
pub struct SchroederReverbSpec {
    pub gain_controller: LiveParameter,
    pub max_gain: f64,

    /// Short-response diffusing delay lines (ms)
    pub allpasses: Vec<f64>,

    /// Short-response diffuse feedback
    pub allpass_feedback: f64,

    /// Long-response resonating delay lines (ms)
    pub combs: Vec<f64>,

    /// Long-response resonant feedback
    pub comb_feedback: f64,

    /// Long-response damping cutoff (Hz)
    pub cutoff: f64,

    /// Additional delay (ms) on right channel for an enhanced stereo effect
    pub stereo: f64,
}

impl SchroederReverbSpec {
    pub fn create(&self, sample_rate_hz: f64) -> SchroederReverb {
        let allpass_filters = self
            .allpasses
            .iter()
            .map(|delay_ms| {
                let delay_samples = (delay_ms / 1000.0 * sample_rate_hz).round() as usize;
                (
                    AllPassDelay::new(delay_samples, self.allpass_feedback),
                    AllPassDelay::new(delay_samples, self.allpass_feedback),
                )
            })
            .collect();

        let stereo_offset = self.stereo / 1000.0 * sample_rate_hz;

        let comb_filters = self
            .combs
            .iter()
            .map(|delay_ms| {
                let delay_samples = delay_ms / 1000.0 * sample_rate_hz;
                (
                    CombFilter::new(
                        delay_samples.round() as usize,
                        OnePoleLowPass::new(self.cutoff, sample_rate_hz)
                            .followed_by(self.comb_feedback),
                        1.0,
                    ),
                    CombFilter::new(
                        (delay_samples + stereo_offset).round() as usize,
                        OnePoleLowPass::new(self.cutoff, sample_rate_hz)
                            .followed_by(self.comb_feedback),
                        1.0,
                    ),
                )
            })
            .collect();

        SchroederReverb {
            gain_controller: self.gain_controller,
            max_gain: self.max_gain,
            allpass_filters,
            comb_filters,
        }
    }
}

type LowPassCombFilter = CombFilter<SuccessiveInteractions<OnePoleLowPass, f64>>;

pub struct SchroederReverb {
    gain_controller: LiveParameter,
    max_gain: f64,
    allpass_filters: Vec<(AllPassDelay, AllPassDelay)>,
    comb_filters: Vec<(LowPassCombFilter, LowPassCombFilter)>,
}

impl AudioStage for SchroederReverb {
    fn render(&mut self, buffer: &mut [f64], storage: &LiveParameterStorage) {
        let gain = self.max_gain * storage.read_parameter(self.gain_controller);

        for signal_sample in buffer.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                let mut reverbed_l = 0.0;
                let mut reverbed_r = 0.0;

                for (comb_l, comb_r) in &mut self.comb_filters {
                    reverbed_l += comb_l.process_sample(gain * *signal_l);
                    reverbed_r += comb_r.process_sample(gain * *signal_r);
                }

                for (allpass_l, allpass_r) in &mut self.allpass_filters {
                    reverbed_l = allpass_l.process_sample(reverbed_l);
                    reverbed_r = allpass_r.process_sample(reverbed_r);
                }

                let normalization = self.comb_filters.len() as f64;

                *signal_l += reverbed_l / normalization;
                *signal_r += reverbed_r / normalization;
            }
        }
    }

    fn mute(&mut self) {
        for allpass in &mut self.allpass_filters {
            allpass.0.mute();
            allpass.1.mute();
        }
        for comb in &mut self.comb_filters {
            comb.0.mute();
            comb.1.mute();
        }
    }
}

/// A simulation of two (stereo) speakers oscillating radially wrt. the listener
#[derive(Clone, Deserialize, Serialize)]
pub struct RotarySpeakerSpec {
    pub gain_controller: LiveParameter,
    pub motor_controller: LiveParameter,

    /// Rotary speaker radius (cm)
    pub rotation_radius: f64,

    /// Rotary speaker minimum speed (revolutions per s)
    pub min_speed: f64,

    /// Rotary speaker maximum speed (revolutions per s)
    pub max_speed: f64,

    /// Rotary speaker acceleration time (s)
    pub acceleration: f64,

    /// Rotary speaker deceleration time (s)
    pub deceleration: f64,
}

impl RotarySpeakerSpec {
    const SPEED_OF_SOUND_IN_CM_PER_S: f64 = 34320.0;

    fn create(&self, sample_rate_hz: f64) -> RotarySpeaker {
        let delay_span = 2.0 * self.rotation_radius / Self::SPEED_OF_SOUND_IN_CM_PER_S;
        let num_samples_in_buffer = (delay_span * sample_rate_hz) as usize + 1;

        RotarySpeaker {
            spec: self.clone(),
            delay_line: DelayLine::new(num_samples_in_buffer),
            curr_angle: 0.0,
            curr_rotation_in_hz: self.min_speed,
            sample_rate_hz,
        }
    }
}

pub struct RotarySpeaker {
    spec: RotarySpeakerSpec,
    delay_line: DelayLine<(f64, f64)>,
    curr_angle: f64,
    curr_rotation_in_hz: f64,
    sample_rate_hz: f64,
}

impl AudioStage for RotarySpeaker {
    fn render(&mut self, signal: &mut [f64], storage: &LiveParameterStorage) {
        let gain = 0.5 * storage.read_parameter(self.spec.gain_controller);
        let target_rotation = self.spec.min_speed
            + storage.read_parameter(self.spec.motor_controller)
                * (self.spec.max_speed - self.spec.min_speed);

        let frequency_width = self.spec.max_speed - self.spec.min_speed;

        let (acceleration, lower_limit, upper_limit) =
            match self.curr_rotation_in_hz.partial_cmp(&target_rotation) {
                Some(Ordering::Less) => (
                    frequency_width / self.spec.acceleration,
                    self.spec.min_speed,
                    target_rotation,
                ),
                Some(Ordering::Greater) => (
                    -frequency_width / self.spec.deceleration,
                    target_rotation,
                    self.spec.max_speed,
                ),
                _ => (0.0, self.spec.min_speed, self.spec.max_speed),
            };

        for signal_sample in signal.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                self.delay_line.advance();
                self.delay_line.write((gain * *signal_l, gain * *signal_r));

                let left_offset = 0.5 + 0.5 * self.curr_angle.sin();
                let right_offset = 1.0 - left_offset;

                let delayed_l = self.delay_line.get_delayed_fract(left_offset).0;
                let delayed_r = self.delay_line.get_delayed_fract(right_offset).1;

                *signal_l += delayed_l;
                *signal_r += delayed_r;

                self.curr_rotation_in_hz = (self.curr_rotation_in_hz
                    + acceleration / self.sample_rate_hz)
                    .max(lower_limit)
                    .min(upper_limit);

                self.curr_angle = (self.curr_angle
                    + self.curr_rotation_in_hz / self.sample_rate_hz * TAU)
                    .rem_euclid(TAU);
            }
        }
    }

    fn mute(&mut self) {
        self.delay_line.mute();
    }
}
