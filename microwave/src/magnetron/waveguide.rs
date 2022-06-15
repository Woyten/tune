use serde::{Deserialize, Serialize};

use super::{
    util::{CombFilter, Interaction, OnePoleLowPass, SoftClip},
    waveform::{AutomationSpec, Creator, InBufferSpec, OutSpec, Spec, Stage},
};

#[derive(Deserialize, Serialize)]
pub struct WaveguideSpec<A> {
    pub buffer_size: usize,
    pub frequency: A,
    pub cutoff: A,
    pub feedback: A,
    pub reflectance: Reflectance,
    pub in_buffer: InBufferSpec,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Reflectance {
    Positive,
    Negative,
}

impl<A: AutomationSpec> Spec for WaveguideSpec<A> {
    type Created = Stage<A>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let in_buffer = self.in_buffer.buffer();
        let out_buffer = self.out_spec.out_buffer.buffer();

        let buffer_size = self.buffer_size;
        let (feedback_factor, length_factor) = match self.reflectance {
            Reflectance::Positive => (1.0, 1.0),
            Reflectance::Negative => (-1.0, 0.5),
        };

        let low_pass = OnePoleLowPass::default().followed_by(0.0);
        let mut comb_filter = CombFilter::new(buffer_size, low_pass, SoftClip::new(0.9));

        creator.create_stage(
            (
                &self.out_spec.out_level,
                (&self.frequency, &self.cutoff, &self.feedback),
            ),
            move |buffers, (out_level, (frequency, cutoff, feedback))| {
                let low_pass = comb_filter.response_fn();
                low_pass
                    .first()
                    .set_cutoff(cutoff, 1.0 / buffers.sample_width_secs);
                *low_pass.second() = feedback * feedback_factor;

                let num_samples_to_skip_back = length_factor
                    / (buffers.sample_width_secs * frequency)
                    - low_pass.delay_samples();

                let fract_offset = (num_samples_to_skip_back / buffer_size as f64).clamp(0.0, 1.0);

                buffers.read_1_and_write(in_buffer, out_buffer, out_level, |input| {
                    comb_filter.process_sample_fract(fract_offset, input)
                })
            },
        )
    }
}
