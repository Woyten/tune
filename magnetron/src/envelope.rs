use serde::{Deserialize, Serialize};

use crate::{
    automation::AutomationSpec,
    spec::{Creator, Spec},
    Stage, StageState,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct EnvelopeSpec<A> {
    pub amplitude: A,
    pub fadeout: A,
    pub attack_time: A,
    pub release_time: A,
    pub decay_rate: A,
}

impl<A: AutomationSpec> Spec<A> for EnvelopeSpec<A> {
    type Created = Stage<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        let mut secs_since_pressed = 0.0;
        let mut secs_since_released = 0.0;
        let mut saved_amplitude = 0.0;

        creator.create_stage(
            (
                (&self.amplitude, &self.fadeout),
                (&self.attack_time, &self.release_time, &self.decay_rate),
            ),
            move |buffers, ((amplitude, fadeout), (attack_time, release_time, decay_rate))| {
                let buffer_len_f64 = buffers.buffer_len() as f64;
                let render_window_secs = buffers.sample_width_secs() * buffer_len_f64;

                secs_since_pressed += render_window_secs;
                secs_since_released += render_window_secs * fadeout;

                let (envelope_value, state) = (Envelope {
                    attack_time,
                    release_time,
                    decay_rate,
                })
                .evaluate(secs_since_pressed, secs_since_released);

                let to_amplitude = envelope_value * amplitude;
                let amplitude_increment = (to_amplitude - saved_amplitude) / buffer_len_f64;

                let out_buffer = buffers.readable.audio_out.read();
                buffers.readable.mix.write(out_buffer.iter().map(|src| {
                    let result = src * saved_amplitude;
                    saved_amplitude += amplitude_increment;
                    result
                }));

                state
            },
        )
    }
}

struct Envelope {
    attack_time: f64,
    release_time: f64,
    decay_rate: f64,
}

impl Envelope {
    fn evaluate(&self, secs_since_pressed: f64, secs_since_released: f64) -> (f64, StageState) {
        let signal_without_release = if secs_since_pressed < self.attack_time {
            secs_since_pressed / self.attack_time
        } else {
            ((self.attack_time - secs_since_pressed) * self.decay_rate).exp2()
        };

        if secs_since_released < self.release_time {
            (
                signal_without_release * (1.0 - secs_since_released / self.release_time),
                StageState::Active,
            )
        } else {
            (0.0, StageState::Exhausted)
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn normal_envelope() {
        let envelope = Envelope {
            attack_time: 1.0,
            release_time: 2.0,
            decay_rate: 0.0,
        };

        assert_approx_eq!(envelope.evaluate(0.0, 0.0).0, 0.00);
        assert_approx_eq!(envelope.evaluate(0.5, 0.0).0, 0.50);
        assert_approx_eq!(envelope.evaluate(1.0, 0.0).0, 1.00);
        assert_approx_eq!(envelope.evaluate(1.5, 0.0).0, 1.00);
        assert_approx_eq!(envelope.evaluate(2.0, 0.0).0, 1.00);
        assert_approx_eq!(envelope.evaluate(2.0, 0.5).0, 0.75);
        assert_approx_eq!(envelope.evaluate(2.0, 1.0).0, 0.50);
        assert_approx_eq!(envelope.evaluate(2.0, 1.5).0, 0.25);
        assert_approx_eq!(envelope.evaluate(2.0, 2.0).0, 0.00);
        assert_eq!(envelope.evaluate(0.0, 0.000).1, StageState::Active);
        assert_eq!(envelope.evaluate(0.0, 1.000).1, StageState::Active);
        assert_eq!(envelope.evaluate(0.0, 1.999).1, StageState::Active);
        assert_eq!(envelope.evaluate(0.0, 2.001).1, StageState::Exhausted);
    }

    #[test]
    fn trivial_envelope() {
        let envelope = Envelope {
            attack_time: 1e-10,
            release_time: 1e-10,
            decay_rate: 0.0,
        };

        assert_approx_eq!(envelope.evaluate(0.000, 0.000).0, 0.0);
        assert_approx_eq!(envelope.evaluate(0.001, 0.000).0, 1.0);
        assert_approx_eq!(envelope.evaluate(0.001, 0.001).0, 0.0);
        assert_eq!(envelope.evaluate(0.0, 0.000).1, StageState::Active);
        assert_eq!(envelope.evaluate(0.0, 0.001).1, StageState::Exhausted);
    }
}
