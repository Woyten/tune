use std::{f64::consts::TAU, iter};

use magnetron::{
    automation::{AutomatableParam, AutomatableSlice, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::Stage,
};
use serde::{Deserialize, Serialize};

use super::util::{AllPassDelay, CombFilter, DelayLine, Interaction, OnePoleLowPass};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "effect_type")]
pub enum EffectSpec<A> {
    /// A recursive delay with channel-rotated feedback
    Echo {
        buffer_size: usize,

        gain: A,

        /// Delay time (s)
        delay_time: A,

        /// Delay feedback
        feedback: A,

        /// Delay feedback rotation angle (degrees clock-wise)
        feedback_rotation: A,
    },
    /// A combination of multiple allpass filters in series, followed by multiple comb filters in parallel
    SchroederReverb {
        buffer_size: usize,

        gain: A,

        /// Short-response diffusing delay lines (ms)
        allpasses: Vec<A>,

        /// Short-response diffuse feedback
        allpass_feedback: A,

        /// Long-response resonating delay lines (ms)
        combs: Vec<(A, A)>,

        /// Long-response resonant feedback
        comb_feedback: A,

        /// Long-response damping cutoff (Hz)
        cutoff: A,
    },
    /// A simulation of two (stereo) speakers oscillating radially wrt. the listener
    RotarySpeaker {
        buffer_size: usize,

        gain: A,

        /// Rotary speaker radius (cm)
        rotation_radius: A,

        /// Rotary speaker speed (revolutions per s)
        speed: A,
    },
}

impl<A: AutomatableParam> EffectSpec<A> {
    pub fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<&(A, A)>,
    ) -> Stage<A> {
        match self {
            EffectSpec::Echo {
                buffer_size,
                gain,
                delay_time,
                feedback,
                feedback_rotation,
            } => {
                let mut delay_line = DelayLine::<(f64, f64)>::new(*buffer_size);

                factory
                    .automate((gain, (delay_time, feedback, feedback_rotation), out_levels))
                    .into_stage(
                        move |buffers,
                              (
                            gain,
                            (delay_time_secs, feedback, feedback_rotation),
                            out_levels,
                        )| {
                            if buffers.reset() {
                                delay_line.mute();
                            }

                            // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
                            let (sin, cos) = (feedback_rotation / 2.0).to_radians().sin_cos();
                            let rot_l_l = cos * feedback;
                            let rot_r_l = sin * feedback;
                            let rot_l_r = -rot_r_l;
                            let rot_r_r = rot_l_l;

                            let delay_line_secs =
                                buffers.sample_width_secs() * delay_line.buffer_len() as f64;
                            let fract_offset = delay_time_secs / delay_line_secs;

                            buffers.read_2_write_2(
                                in_buffers,
                                out_buffers,
                                out_levels,
                                |signal_l, signal_r| {
                                    delay_line.advance();

                                    let delayed = delay_line.get_delayed_fract(fract_offset);

                                    let feedback_l = rot_l_l * delayed.0 + rot_l_r * delayed.1;
                                    let feedback_r = rot_r_l * delayed.0 + rot_r_r * delayed.1;

                                    delay_line.write((
                                        feedback_l + gain * signal_l,
                                        feedback_r + gain * signal_r,
                                    ));

                                    (signal_l + feedback_l, signal_r + feedback_r)
                                },
                            )
                        },
                    )
            }
            EffectSpec::SchroederReverb {
                buffer_size,
                gain,
                allpasses,
                allpass_feedback,
                combs,
                comb_feedback,
                cutoff,
            } => {
                let buffer_size = *buffer_size;

                let mut allpass_processors: Vec<_> = iter::repeat_with(|| {
                    (
                        AllPassDelay::new(buffer_size, 0.0),
                        AllPassDelay::new(buffer_size, 0.0),
                    )
                })
                .take(allpasses.len())
                .collect();

                let mut comb_processors: Vec<_> = iter::repeat_with(|| {
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
                    )
                })
                .take(combs.len())
                .collect();

                factory
                    .automate((
                        gain,
                        (AutomatableSlice::new(allpasses), allpass_feedback),
                        (AutomatableSlice::new(combs), comb_feedback),
                        cutoff,
                        out_levels,
                    ))
                    .into_stage(
                        move |buffers,
                              (
                            gain,
                            (allpass_delays_ms, allpass_feedback),
                            (comb_delays_ms, comb_feedback),
                            cutoff_hz,
                            out_levels,
                        )| {
                            if buffers.reset() {
                                for allpass in &mut allpass_processors {
                                    allpass.0.mute();
                                    allpass.1.mute();
                                }
                                for comb in &mut comb_processors {
                                    comb.0.mute();
                                    comb.1.mute();
                                }
                            }

                            let sample_rate_hz = buffers.sample_width_secs().recip();
                            let delay_line_ms =
                                buffers.sample_width_secs() * buffer_size as f64 * 1000.0;

                            for (allpass_l, allpass_r) in &mut allpass_processors {
                                allpass_l.set_feedback(allpass_feedback);
                                allpass_r.set_feedback(allpass_feedback);
                            }

                            for (comb_l, comb_r) in &mut comb_processors {
                                let response_fn_l = &mut comb_l.response_fn();
                                response_fn_l.first().set_cutoff(cutoff_hz, sample_rate_hz);
                                *response_fn_l.second() = comb_feedback;

                                let response_fn_r = &mut comb_r.response_fn();
                                response_fn_r.first().set_cutoff(cutoff_hz, sample_rate_hz);
                                *response_fn_r.second() = comb_feedback;
                            }

                            buffers.read_2_write_2(
                                in_buffers,
                                out_buffers,
                                out_levels,
                                |signal_l, signal_r| {
                                    let mut diffused_l = gain * signal_l;
                                    let mut diffused_r = gain * signal_r;

                                    for ((allpass_l, allpass_r), delay_ms) in
                                        allpass_processors.iter_mut().zip(allpass_delays_ms)
                                    {
                                        diffused_l = allpass_l.process_sample_fract(
                                            *delay_ms / delay_line_ms,
                                            diffused_l,
                                        );
                                        diffused_r = allpass_r.process_sample_fract(
                                            *delay_ms / delay_line_ms,
                                            diffused_r,
                                        );
                                    }

                                    let mut reverbed_l = 0.0;
                                    let mut reverbed_r = 0.0;

                                    for ((comb_l, comb_r), (delay_l_ms, delay_r)) in
                                        comb_processors.iter_mut().zip(comb_delays_ms)
                                    {
                                        reverbed_l += comb_l.process_sample_fract(
                                            *delay_l_ms / delay_line_ms,
                                            diffused_l,
                                        );
                                        reverbed_r += comb_r.process_sample_fract(
                                            *delay_r / delay_line_ms,
                                            diffused_r,
                                        );
                                    }

                                    let normalization = comb_delays_ms.len() as f64;

                                    (
                                        signal_l + reverbed_l / normalization,
                                        signal_r + reverbed_r / normalization,
                                    )
                                },
                            )
                        },
                    )
            }
            EffectSpec::RotarySpeaker {
                buffer_size,
                gain,
                rotation_radius,
                speed,
            } => {
                const SPEED_OF_SOUND_CM_PER_S: f64 = 34320.0;

                let buffer_size = *buffer_size;

                let mut delay_line_l = DelayLine::new(buffer_size);
                let mut delay_line_r = DelayLine::new(buffer_size);

                let mut curr_angle = 0.0f64;

                factory
                    .automate((gain, (rotation_radius, speed), out_levels))
                    .into_stage(
                        move |buffers, (gain, (rotation_radius_cm, speed_hz), out_levels)| {
                            if buffers.reset() {
                                delay_line_l.mute();
                                delay_line_r.mute();
                            }

                            let sample_width_secs = buffers.sample_width_secs();
                            let max_fract_delay = rotation_radius_cm
                                / (SPEED_OF_SOUND_CM_PER_S
                                    * sample_width_secs
                                    * buffer_size as f64);

                            buffers.read_2_write_2(
                                in_buffers,
                                out_buffers,
                                out_levels,
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
                                    curr_angle =
                                        (curr_angle + angle_increment * TAU).rem_euclid(TAU);

                                    (signal_l + delayed_l, signal_r + delayed_r)
                                },
                            )
                        },
                    )
            }
        }
    }
}
