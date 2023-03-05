use serde::{Deserialize, Serialize};

use crate::{automation::AutomationSpec, creator::Creator, Stage, StageState};

#[derive(Clone, Deserialize, Serialize)]
pub struct EnvelopeSpec<A> {
    pub amplitude: A,
    pub fadeout: A,
    pub attack_time: A,
    pub decay_rate: A,
    pub release_time: A,
}

impl<A: AutomationSpec> EnvelopeSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        let mut attack_progress = 0.0;
        let mut decay_progress = 0.0f64;
        let mut release_progress = 0.0;
        let mut saved_amplitude = 0.0;

        creator.create_stage(
            (
                (&self.amplitude, &self.fadeout),
                (&self.attack_time, &self.release_time, &self.decay_rate),
            ),
            move |buffers, ((amplitude, fadeout), (attack_time, release_time, decay_rate))| {
                let buffer_len_f64 = buffers.buffer_len() as f64;
                let render_window_secs = buffers.sample_width_secs() * buffer_len_f64;

                attack_progress += (render_window_secs / attack_time).max(0.0);
                let signal_without_release = if attack_progress <= 1.0 {
                    attack_progress
                } else {
                    decay_progress -= (render_window_secs * decay_rate).max(0.0);
                    decay_progress.exp2()
                };

                release_progress += (render_window_secs * fadeout / release_time).max(0.0);
                let to_amplitude =
                    (signal_without_release * (1.0 - release_progress.min(1.0))) * amplitude;

                let amplitude_increment = (to_amplitude - saved_amplitude) / buffer_len_f64;

                let out_buffer = buffers.readable.audio_out.read();
                buffers.readable.mix.write(out_buffer.iter().map(|src| {
                    let result = src * saved_amplitude;
                    saved_amplitude += amplitude_increment;
                    result
                }));

                match release_progress < 1.0 {
                    true => StageState::Active,
                    false => StageState::Exhausted,
                }
            },
        )
    }
}
