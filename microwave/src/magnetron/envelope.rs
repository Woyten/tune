use serde::{Deserialize, Serialize};

use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::{Stage, StageActivity},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnvelopeSpec<A> {
    pub in_buffer: usize,
    pub out_buffers: (usize, usize),
    pub out_levels: Option<(A, A)>,
    pub fadeout: A,
    pub attack_time: A,
    pub decay_rate: A,
    pub release_time: A,
}

impl<A: AutomatableParam> EnvelopeSpec<A> {
    pub fn create(&self, factory: &mut AutomationFactory<A>) -> Stage<A> {
        let mut attack_progress = 0.0;
        let mut decay_progress = 0.0f64;
        let mut release_progress = 0.0;
        let mut saved_amplitude = 0.0;

        let (in_buffer, out_buffers) = (
            BufferIndex::Internal(self.in_buffer),
            (
                BufferIndex::External(self.out_buffers.0),
                BufferIndex::External(self.out_buffers.1),
            ),
        );

        factory
            .automate((
                (&self.out_levels, &self.fadeout),
                (&self.attack_time, &self.release_time, &self.decay_rate),
            ))
            .into_stage(
                move |buffers, ((out_levels, fadeout), (attack_time, release_time, decay_rate))| {
                    attack_progress += (buffers.render_window_secs() / attack_time).max(0.0);
                    let amplitude_without_release = if attack_progress <= 1.0 {
                        attack_progress
                    } else {
                        decay_progress -= (buffers.render_window_secs() * decay_rate).max(0.0);
                        decay_progress.exp2()
                    };

                    release_progress +=
                        (buffers.render_window_secs() * fadeout / release_time).max(0.0);
                    let to_amplitude =
                        amplitude_without_release * (1.0 - release_progress.min(1.0));

                    let amplitude_increment =
                        (to_amplitude - saved_amplitude) / (buffers.buffer_len() as f64);

                    let out_levels = out_levels.unwrap_or((1.0, 1.0));
                    buffers.read_1_write_2(in_buffer, out_buffers, None, |src| {
                        let signal = src * saved_amplitude;
                        saved_amplitude += amplitude_increment;
                        (signal * out_levels.0, signal * out_levels.1)
                    });

                    match release_progress < 1.0 {
                        true => StageActivity::External,
                        false => StageActivity::Exhausted,
                    }
                },
            )
    }
}
