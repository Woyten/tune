use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    source::LfSource,
    util::{CombFilter, Interaction, OnePoleLowPass, SoftClip},
    waveform::{Creator, InBuffer, OutSpec, Spec, Stage},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct WaveguideSpec<C> {
    pub buffer_size: usize,
    pub frequency: LfSource<C>,
    pub cutoff: LfSource<C>,
    pub feedback: LfSource<C>,
    pub reflectance: Reflectance,
    pub in_buffer: InBuffer,
    #[serde(flatten)]
    pub out_spec: OutSpec<C>,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Reflectance {
    Positive,
    Negative,
}

impl<C: Controller> Spec for WaveguideSpec<C> {
    type Created = Stage<C>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let in_buffer = self.in_buffer.clone();
        let out_buffer = self.out_spec.out_buffer.clone();

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

                buffers.read_1_and_write(&in_buffer, &out_buffer, out_level, |input| {
                    comb_filter.process_sample_fract(fract_offset, input)
                })
            },
        )
    }
}
