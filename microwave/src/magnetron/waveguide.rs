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

impl<C: Controller> Spec for &WaveguideSpec<C> {
    type Created = Stage<C::Storage>;

    fn use_creator(self, creator: &Creator) -> Self::Created {
        let buffer_size = self.buffer_size;
        let (mut frequency, mut cutoff, mut feedback) =
            creator.create((&self.frequency, &self.cutoff, &self.feedback));
        let (input, mut output) = creator.create((&self.in_buffer, &self.out_spec));

        let (feedback_factor, length_factor) = match self.reflectance {
            Reflectance::Positive => (1.0, 1.0),
            Reflectance::Negative => (-1.0, 0.5),
        };

        let low_pass = OnePoleLowPass::default().followed_by(0.0);
        let mut comb_filter = CombFilter::new(buffer_size, low_pass, SoftClip::new(0.9));

        Box::new(move |buffers, control| {
            let frequency = frequency(control);
            let cutoff = cutoff(control);
            let feedback = feedback(control);

            let low_pass = comb_filter.response_fn();
            low_pass
                .first()
                .set_cutoff(cutoff, 1.0 / buffers.sample_width_secs);
            *low_pass.second() = feedback * feedback_factor;

            let num_samples_to_skip_back =
                length_factor / (buffers.sample_width_secs * frequency) - low_pass.delay_samples();

            let fract_offset = (num_samples_to_skip_back / buffer_size as f64).clamp(0.0, 1.0);

            buffers.read_1_and_write(&input, &mut output, control, |input| {
                comb_filter.process_sample_fract(fract_offset, input)
            })
        })
    }
}
