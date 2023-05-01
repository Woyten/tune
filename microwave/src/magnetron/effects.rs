use std::f64::consts::TAU;

use magnetron::{automation::AutomationSpec, buffer::BufferIndex, creator::Creator, stage::Stage};
use serde::{Deserialize, Serialize};

use super::util::{AllPassDelay, CombFilter, DelayLine, Interaction, OnePoleLowPass};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum EffectSpec<A> {
    Echo(EchoSpec<A>),
    SchroederReverb(SchroederReverbSpec<A>),
    RotarySpeaker(RotarySpeakerSpec<A>),
}

impl<A: AutomationSpec> EffectSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        match self {
            EffectSpec::Echo(spec) => spec.use_creator(creator),
            EffectSpec::SchroederReverb(spec) => spec.use_creator(creator),
            EffectSpec::RotarySpeaker(spec) => spec.use_creator(creator),
        }
    }
}

/// A recursive delay with channel-rotated feedback
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EchoSpec<A> {
    pub buffer_size: usize,

    pub gain: A,

    /// Delay time (s)
    pub delay_time: A,

    /// Delay feedback
    pub feedback: A,

    /// Delay feedback rotation angle (degrees clock-wise)
    pub feedback_rotation: A,

    pub in_buffers: (usize, usize),
    pub out_buffers: (usize, usize),
}

impl<A: AutomationSpec> EchoSpec<A> {
    fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        let in_buffers = self.in_buffers;
        let out_buffers = self.out_buffers;

        let mut delay_line = DelayLine::<(f64, f64)>::new(self.buffer_size);

        creator.create_stage(
            (
                &self.gain,
                (&self.delay_time, &self.feedback, &self.feedback_rotation),
            ),
            move |buffers, (gain, (delay_time_secs, feedback, feedback_rotation))| {
                if buffers.reset() {
                    delay_line.mute();
                }

                // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
                let (sin, cos) = (feedback_rotation / 2.0).to_radians().sin_cos();
                let rot_l_l = cos * feedback;
                let rot_r_l = sin * feedback;
                let rot_l_r = -rot_r_l;
                let rot_r_r = rot_l_l;

                let delay_line_secs = buffers.sample_width_secs() * delay_line.buffer_len() as f64;
                let fract_offset = delay_time_secs / delay_line_secs;

                buffers.read_2_write_2(
                    (
                        BufferIndex::Internal(in_buffers.0),
                        BufferIndex::Internal(in_buffers.1),
                    ),
                    (
                        BufferIndex::Internal(out_buffers.0),
                        BufferIndex::Internal(out_buffers.1),
                    ),
                    |signal_l, signal_r| {
                        delay_line.advance();

                        let delayed = delay_line.get_delayed_fract(fract_offset);

                        let feedback_l = rot_l_l * delayed.0 + rot_l_r * delayed.1;
                        let feedback_r = rot_r_l * delayed.0 + rot_r_r * delayed.1;

                        delay_line
                            .write((feedback_l + gain * signal_l, feedback_r + gain * signal_r));

                        (signal_l + feedback_l, signal_r + feedback_r)
                    },
                )
            },
        )
    }
}

/// A combination of multiple allpass filters in series, followed by multiple comb filters in parallel
#[derive(Clone, Debug, Deserialize, Serialize)]
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

    pub in_buffers: (usize, usize),
    pub out_buffers: (usize, usize),
}

impl<A: AutomationSpec> SchroederReverbSpec<A> {
    fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        let in_buffers = self.in_buffers;
        let out_buffers = self.out_buffers;

        let buffer_size = self.buffer_size;
        let mut allpasses: Vec<_> = self
            .allpasses
            .iter()
            .map(|delay_ms| {
                (
                    AllPassDelay::new(buffer_size, 0.0),
                    AllPassDelay::new(buffer_size, 0.0),
                    creator.create_value(delay_ms),
                    0.0,
                )
            })
            .collect();
        let mut combs: Vec<_> = self
            .combs
            .iter()
            .map(|(delay_ms_l, delay_ms_r)| {
                (
                    CombFilter::new(
                        buffer_size,
                        OnePoleLowPass::new(0.0, 0.0).followed_by(0.0),
                        1.0,
                    ),
                    CombFilter::new(
                        buffer_size,
                        OnePoleLowPass::new(0.0, 0.0).followed_by(0.0),
                        1.0,
                    ),
                    creator.create_value(delay_ms_l),
                    creator.create_value(delay_ms_r),
                    0.0,
                    0.0,
                )
            })
            .collect();

        let mut gain = creator.create_value(&self.gain);
        let (mut allpass_feedback, mut comb_feedback, mut cutoff_hz) =
            creator.create_value((&self.allpass_feedback, &self.comb_feedback, &self.cutoff));

        Stage::new(move |buffers, context| {
            if buffers.reset() {
                for allpass in &mut allpasses {
                    allpass.0.mute();
                    allpass.1.mute();
                }
                for comb in &mut combs {
                    comb.0.mute();
                    comb.1.mute();
                }
            }

            let gain = context.read(&mut gain);
            let (allpass_feedback, comb_feedback, cutoff_hz) =
                context.read(&mut (&mut allpass_feedback, &mut comb_feedback, &mut cutoff_hz));

            let sample_rate_hz = buffers.sample_width_secs().recip();
            let delay_line_ms = buffers.sample_width_secs() * buffer_size as f64 * 1000.0;

            for (allpass_l, allpass_r, delay_ms, delay) in &mut allpasses {
                allpass_l.set_feedback(allpass_feedback);
                allpass_r.set_feedback(allpass_feedback);

                *delay = context.read(delay_ms) / delay_line_ms;
            }

            for (comb_l, comb_r, delay_ms_l, delay_ms_r, delay_l, delay_r) in &mut combs {
                let response_fn_l = &mut comb_l.response_fn();
                response_fn_l.first().set_cutoff(cutoff_hz, sample_rate_hz);
                *response_fn_l.second() = comb_feedback;

                let response_fn_r = &mut comb_r.response_fn();
                response_fn_r.first().set_cutoff(cutoff_hz, sample_rate_hz);
                *response_fn_r.second() = comb_feedback;

                *delay_l = context.read(delay_ms_l) / delay_line_ms;
                *delay_r = context.read(delay_ms_r) / delay_line_ms;
            }

            buffers.read_2_write_2(
                (
                    BufferIndex::Internal(in_buffers.0),
                    BufferIndex::Internal(in_buffers.1),
                ),
                (
                    BufferIndex::Internal(out_buffers.0),
                    BufferIndex::Internal(out_buffers.1),
                ),
                |signal_l, signal_r| {
                    let mut diffused_l = gain * signal_l;
                    let mut diffused_r = gain * signal_r;

                    for (allpass_l, allpass_r, .., delay) in &mut allpasses {
                        diffused_l = allpass_l.process_sample_fract(*delay, diffused_l);
                        diffused_r = allpass_r.process_sample_fract(*delay, diffused_r);
                    }

                    let mut reverbed_l = 0.0;
                    let mut reverbed_r = 0.0;

                    for (comb_l, comb_r, .., delay_l, delay_r) in &mut combs {
                        reverbed_l += comb_l.process_sample_fract(*delay_l, diffused_l);
                        reverbed_r += comb_r.process_sample_fract(*delay_r, diffused_r);
                    }

                    let normalization = combs.len() as f64;

                    (
                        signal_l + reverbed_l / normalization,
                        signal_r + reverbed_r / normalization,
                    )
                },
            )
        })
    }
}

/// A simulation of two (stereo) speakers oscillating radially wrt. the listener
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RotarySpeakerSpec<A> {
    pub buffer_size: usize,

    pub gain: A,

    /// Rotary speaker radius (cm)
    pub rotation_radius: A,

    /// Rotary speaker speed (revolutions per s)
    pub speed: A,

    pub in_buffers: (usize, usize),
    pub out_buffers: (usize, usize),
}

impl<A: AutomationSpec> RotarySpeakerSpec<A> {
    fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        const SPEED_OF_SOUND_CM_PER_S: f64 = 34320.0;

        let in_buffers = self.in_buffers;
        let out_buffers = self.out_buffers;

        let buffer_size = self.buffer_size;
        let mut delay_line_l = DelayLine::new(buffer_size);
        let mut delay_line_r = DelayLine::new(buffer_size);

        let mut curr_angle = 0.0f64;

        creator.create_stage(
            (&self.gain, &self.rotation_radius, &self.speed),
            move |buffers, (gain, rotation_radius_cm, speed_hz)| {
                if buffers.reset() {
                    delay_line_l.mute();
                    delay_line_r.mute();
                }

                let sample_width_secs = buffers.sample_width_secs();
                let max_fract_delay = rotation_radius_cm
                    / (SPEED_OF_SOUND_CM_PER_S * sample_width_secs * buffer_size as f64);

                buffers.read_2_write_2(
                    (
                        BufferIndex::Internal(in_buffers.0),
                        BufferIndex::Internal(in_buffers.1),
                    ),
                    (
                        BufferIndex::Internal(out_buffers.0),
                        BufferIndex::Internal(out_buffers.1),
                    ),
                    |signal_l, signal_r| {
                        delay_line_l.advance();
                        delay_line_r.advance();

                        delay_line_l.write(gain * signal_l);
                        delay_line_r.write(gain * signal_r);

                        let offset_l = 0.5 + 0.5 * curr_angle.sin();
                        let offset_r = 1.0 - offset_l;

                        let fract_offset_l = max_fract_delay * offset_l;
                        let fract_offset_r = max_fract_delay * offset_r;

                        let delayed_l = delay_line_l.get_delayed_fract(fract_offset_l);
                        let delayed_r = delay_line_r.get_delayed_fract(fract_offset_r);

                        let angle_increment = speed_hz * sample_width_secs;
                        curr_angle = (curr_angle + angle_increment * TAU).rem_euclid(TAU);

                        (signal_l + delayed_l, signal_r + delayed_r)
                    },
                )
            },
        )
    }
}
