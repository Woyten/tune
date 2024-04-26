use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::Stage,
};
use serde::{Deserialize, Serialize};

use super::util::{CombFilter, Interaction, OnePoleLowPass, SoftClip};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WaveguideSpec<A> {
    pub buffer_size: usize,
    pub frequency: A,
    pub cutoff: A,
    pub feedback: A,
    pub reflectance: Reflectance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Reflectance {
    Positive,
    Negative,
}

impl<A: AutomatableParam> WaveguideSpec<A> {
    pub fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffer: BufferIndex,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        let buffer_size = self.buffer_size;
        let (feedback_factor, length_factor) = match self.reflectance {
            Reflectance::Positive => (1.0, 1.0),
            Reflectance::Negative => (-1.0, 0.5),
        };

        let low_pass = OnePoleLowPass::default().followed_by(0.0);
        let mut comb_filter = CombFilter::new(buffer_size, low_pass, SoftClip::new(0.9));

        factory
            .automate((out_level, &self.frequency, &self.cutoff, &self.feedback))
            .into_stage(move |buffers, (out_level, frequency, cutoff, feedback)| {
                let low_pass = comb_filter.response_fn();
                low_pass
                    .first()
                    .set_cutoff(cutoff, 1.0 / buffers.sample_width_secs());
                *low_pass.second() = feedback * feedback_factor;

                let num_samples_to_skip_back = length_factor
                    / (buffers.sample_width_secs() * frequency)
                    - low_pass.delay_samples();

                let fract_offset = (num_samples_to_skip_back / buffer_size as f64)
                    .max(0.0)
                    .min(1.0);

                buffers.read_1_write_1(in_buffer, out_buffer, out_level, |input| {
                    comb_filter.process_sample_fract(fract_offset, input)
                })
            })
    }
}
