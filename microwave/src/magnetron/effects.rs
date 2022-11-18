use std::{cmp::Ordering, collections::HashMap, f64::consts::TAU};

use magnetron::{
    automation::{Automation, AutomationContext, AutomationSpec},
    spec::{Creator, Spec},
};
use serde::{Deserialize, Serialize};

use crate::audio::AudioStage;

use super::util::{
    AllPassDelay, CombFilter, DelayLine, Interaction, OnePoleLowPass, SuccessiveInteractions,
};

#[derive(Deserialize, Serialize)]
pub enum EffectSpec<A> {
    Echo(EchoSpec<A>),
    SchroederReverb(SchroederReverbSpec<A>),
    RotarySpeaker(RotarySpeakerSpec<A>),
}

impl<A: AutomationSpec> EffectSpec<A> {
    pub fn create(&self) -> Box<dyn AudioStage<A::Context>> {
        let creator = Creator::new(HashMap::new());
        match self {
            EffectSpec::Echo(spec) => Box::new(creator.create(spec)),
            EffectSpec::SchroederReverb(spec) => Box::new(creator.create(spec)),
            EffectSpec::RotarySpeaker(spec) => Box::new(creator.create(spec)),
        }
    }
}

/// A recursive delay with channel-rotated feedback
#[derive(Deserialize, Serialize)]
pub struct EchoSpec<A> {
    pub buffer_size: usize,

    pub gain: A,

    /// Delay time (s)
    pub delay_time: A,

    /// Delay feedback
    pub feedback: A,

    /// Delay feedback rotation angle (degrees clock-wise)
    pub feedback_rotation: A,
}

impl<A: AutomationSpec> Spec<A> for EchoSpec<A> {
    type Created = Echo<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        Echo {
            delay_line: DelayLine::new(self.buffer_size),
            gain: creator.create(&self.gain),
            delay_time_secs: creator.create(&self.delay_time),
            feedback: creator.create(&self.feedback),
            feedback_rotation: creator.create(&self.feedback_rotation),
        }
    }
}

pub struct Echo<T> {
    delay_line: DelayLine<(f64, f64)>,
    gain: Automation<T>,
    delay_time_secs: Automation<T>,
    feedback: Automation<T>,
    feedback_rotation: Automation<T>,
}

impl<T> AudioStage<T> for Echo<T> {
    fn render(&mut self, buffer: &mut [f64], context: &AutomationContext<T>) {
        let gain = context.read(&mut self.gain);
        let (delay_time_secs, feedback, feedback_rotation) = context.read(&mut (
            &mut self.delay_time_secs,
            &mut self.feedback,
            &mut self.feedback_rotation,
        ));

        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (feedback_rotation / 2.0).sin_cos();
        let rot_l_l = cos * feedback;
        let rot_r_l = sin * feedback;
        let rot_l_r = -rot_r_l;
        let rot_r_r = rot_l_l;

        let sample_width_secs = context.render_window_secs / buffer.len() as f64;
        let delay_line_secs = sample_width_secs * self.delay_line.buffer_len() as f64;
        let fract_offset = delay_time_secs / delay_line_secs;

        for signal_sample in buffer.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                self.delay_line.advance();

                let delayed = self.delay_line.get_delayed_fract(fract_offset);

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
pub struct SchroederReverbSpec<A> {
    pub buffer_size: usize,

    pub gain: A,

    /// Short-response diffusing delay lines (ms)
    pub allpasses: Vec<A>,

    /// Short-response diffuse feedback
    pub allpass_feedback: A,

    /// Long-response resonating delay lines (ms)
    pub combs: Vec<(A, A)>,

    /// Long-response resonant feedback
    pub comb_feedback: A,

    /// Long-response damping cutoff (Hz)
    pub cutoff: A,
}

impl<A: AutomationSpec> Spec<A> for SchroederReverbSpec<A> {
    type Created = SchroederReverb<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        let allpasses = self
            .allpasses
            .iter()
            .map(|delay_ms| {
                (
                    AllPassDelay::new(self.buffer_size, 0.0),
                    AllPassDelay::new(self.buffer_size, 0.0),
                    creator.create(delay_ms),
                    0.0,
                )
            })
            .collect();

        let combs = self
            .combs
            .iter()
            .map(|(delay_ms_l, delay_ms_r)| {
                (
                    CombFilter::new(
                        self.buffer_size,
                        OnePoleLowPass::new(0.0, 0.0).followed_by(0.0),
                        1.0,
                    ),
                    CombFilter::new(
                        self.buffer_size,
                        OnePoleLowPass::new(0.0, 0.0).followed_by(0.0),
                        1.0,
                    ),
                    creator.create(delay_ms_l),
                    creator.create(delay_ms_r),
                    0.0,
                    0.0,
                )
            })
            .collect();

        SchroederReverb {
            buffer_size: self.buffer_size,
            gain: creator.create(&self.gain),
            allpasses,
            allpass_feedback: creator.create(&self.allpass_feedback),
            combs,
            comb_feedback: creator.create(&self.comb_feedback),
            cutoff_hz: creator.create(&self.cutoff),
        }
    }
}

type LowPassCombFilter = CombFilter<SuccessiveInteractions<OnePoleLowPass, f64>>;
type Allpass<T> = (AllPassDelay, AllPassDelay, Automation<T>, f64);
type Comb<T> = (
    LowPassCombFilter,
    LowPassCombFilter,
    Automation<T>,
    Automation<T>,
    f64,
    f64,
);

pub struct SchroederReverb<T> {
    buffer_size: usize,
    gain: Automation<T>,
    allpasses: Vec<Allpass<T>>,
    allpass_feedback: Automation<T>,
    combs: Vec<Comb<T>>,
    comb_feedback: Automation<T>,
    cutoff_hz: Automation<T>,
}

impl<T> AudioStage<T> for SchroederReverb<T> {
    fn render(&mut self, buffer: &mut [f64], context: &AutomationContext<T>) {
        let gain = context.read(&mut self.gain);
        let (allpass_feedback, comb_feedback, cutoff_hz) = context.read(&mut (
            &mut self.allpass_feedback,
            &mut self.comb_feedback,
            &mut self.cutoff_hz,
        ));

        let sample_width_secs = context.render_window_secs / buffer.len() as f64;
        let sample_rate_hz = sample_width_secs.recip();
        let delay_line_ms = sample_width_secs * self.buffer_size as f64 * 1000.0;

        for (allpass_l, allpass_r, delay_ms, delay) in &mut self.allpasses {
            allpass_l.set_feedback(allpass_feedback);
            allpass_r.set_feedback(allpass_feedback);

            *delay = context.read(delay_ms) / delay_line_ms;
        }

        for (comb_l, comb_r, delay_ms_l, delay_ms_r, delay_l, delay_r) in &mut self.combs {
            let response_fn_l = &mut comb_l.response_fn();
            response_fn_l.first().set_cutoff(cutoff_hz, sample_rate_hz);
            *response_fn_l.second() = comb_feedback;

            let response_fn_r = &mut comb_r.response_fn();
            response_fn_r.first().set_cutoff(cutoff_hz, sample_rate_hz);
            *response_fn_r.second() = comb_feedback;

            *delay_l = context.read(delay_ms_l) / delay_line_ms;
            *delay_r = context.read(delay_ms_r) / delay_line_ms;
        }

        for signal_sample in buffer.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                let mut diffused_l = gain * *signal_l;
                let mut diffused_r = gain * *signal_r;

                for (allpass_l, allpass_r, .., delay) in &mut self.allpasses {
                    diffused_l = allpass_l.process_sample_fract(*delay, diffused_l);
                    diffused_r = allpass_r.process_sample_fract(*delay, diffused_r);
                }

                let mut reverbed_l = 0.0;
                let mut reverbed_r = 0.0;

                for (comb_l, comb_r, .., delay_l, delay_r) in &mut self.combs {
                    reverbed_l += comb_l.process_sample_fract(*delay_l, diffused_l);
                    reverbed_r += comb_r.process_sample_fract(*delay_r, diffused_r);
                }

                let normalization = self.combs.len() as f64;

                *signal_l += reverbed_l / normalization;
                *signal_r += reverbed_r / normalization;
            }
        }
    }

    fn mute(&mut self) {
        for allpass in &mut self.allpasses {
            allpass.0.mute();
            allpass.1.mute();
        }
        for comb in &mut self.combs {
            comb.0.mute();
            comb.1.mute();
        }
    }
}

/// A simulation of two (stereo) speakers oscillating radially wrt. the listener
#[derive(Clone, Deserialize, Serialize)]
pub struct RotarySpeakerSpec<A> {
    pub buffer_size: usize,

    pub gain: A,

    /// Rotary speaker radius (cm)
    pub rotation_radius: A,

    /// Rotary speaker target speed (revolutions per s)
    pub speed: A,

    /// Rotary speaker acceleration time (s)
    pub acceleration: A,

    /// Rotary speaker deceleration time (s)
    pub deceleration: A,
}

impl<A: AutomationSpec> Spec<A> for RotarySpeakerSpec<A> {
    type Created = RotarySpeaker<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        RotarySpeaker {
            buffer_size: self.buffer_size,
            delay_line_l: DelayLine::new(self.buffer_size),
            delay_line_r: DelayLine::new(self.buffer_size),
            gain: creator.create(&self.gain),
            rotation_radius_cm: creator.create(&self.rotation_radius),
            target_speed_hz: creator.create(&self.speed),
            acceleration_hz_per_s: creator.create(&self.acceleration),
            deceleration_hz_per_s: creator.create(&self.deceleration),
            curr_angle: 0.0,
            curr_speed_hz: 0.0,
        }
    }
}

pub struct RotarySpeaker<T> {
    buffer_size: usize,
    delay_line_l: DelayLine<f64>,
    delay_line_r: DelayLine<f64>,
    gain: Automation<T>,
    rotation_radius_cm: Automation<T>,
    target_speed_hz: Automation<T>,
    acceleration_hz_per_s: Automation<T>,
    deceleration_hz_per_s: Automation<T>,
    curr_angle: f64,
    curr_speed_hz: f64,
}

impl<T> AudioStage<T> for RotarySpeaker<T> {
    fn render(&mut self, buffer: &mut [f64], context: &AutomationContext<T>) {
        const SPEED_OF_SOUND_CM_PER_S: f64 = 34320.0;

        let gain = context.read(&mut self.gain);
        let rotation_radius_cm = context.read(&mut self.rotation_radius_cm);
        let (target_speed_hz, acceleration_hz_per_s, deceleration_hz_per_s) = context.read(&mut (
            &mut self.target_speed_hz,
            &mut self.acceleration_hz_per_s,
            &mut self.deceleration_hz_per_s,
        ));

        let (required_acceleration_hz_per_s, from_speed_hz, to_speed_hz) = match target_speed_hz
            .partial_cmp(&self.curr_speed_hz)
        {
            Some(Ordering::Less) => (-deceleration_hz_per_s, target_speed_hz, self.curr_speed_hz),
            Some(Ordering::Greater) => (acceleration_hz_per_s, self.curr_speed_hz, target_speed_hz),
            Some(Ordering::Equal) | None => (0.0, self.curr_speed_hz, self.curr_speed_hz),
        };

        let sample_width_secs = context.render_window_secs / buffer.len() as f64;
        let delay_line_secs = sample_width_secs * self.buffer_size as f64;
        let speed_increment_hz = required_acceleration_hz_per_s * sample_width_secs;
        let max_fract_delay = rotation_radius_cm / SPEED_OF_SOUND_CM_PER_S / delay_line_secs;

        for signal_sample in buffer.chunks_mut(2) {
            if let [signal_l, signal_r] = signal_sample {
                self.delay_line_l.advance();
                self.delay_line_r.advance();

                self.delay_line_l.write(gain * *signal_l);
                self.delay_line_r.write(gain * *signal_r);

                let offset_l = 0.5 + 0.5 * self.curr_angle.sin();
                let offset_r = 1.0 - offset_l;

                let fract_offset_l = max_fract_delay * offset_l;
                let fract_offset_r = max_fract_delay * offset_r;

                let delayed_l = self.delay_line_l.get_delayed_fract(fract_offset_l);
                let delayed_r = self.delay_line_r.get_delayed_fract(fract_offset_r);

                *signal_l += delayed_l;
                *signal_r += delayed_r;

                self.curr_speed_hz = (self.curr_speed_hz + speed_increment_hz)
                    .max(from_speed_hz)
                    .min(to_speed_hz);

                let angle_increment = self.curr_speed_hz * sample_width_secs;
                self.curr_angle = (self.curr_angle + angle_increment * TAU).rem_euclid(TAU);
            }
        }
    }

    fn mute(&mut self) {
        self.delay_line_l.mute();
        self.delay_line_r.mute();
    }
}
