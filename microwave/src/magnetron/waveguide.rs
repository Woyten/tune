use serde::{Deserialize, Serialize};

use crate::audio::DEFAULT_SAMPLE_RATE;

use super::{
    control::Controller,
    source::LfSource,
    util::{CombFilter, OnePoleLowPass},
    waveform::{InBuffer, OutSpec, Stage},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct WaveguideSpec<C> {
    pub buffer_size_secs: f64,
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

impl<C: Controller> WaveguideSpec<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut cutoff = self.cutoff.clone();
        let mut feedback = self.feedback.clone();
        let in_buffer = self.in_buffer.clone();
        let mut out_spec = self.out_spec.clone();

        let (feedback_factor, length_factor) = match self.reflectance {
            Reflectance::Positive => (1.0, 1.0),
            Reflectance::Negative => (-1.0, 0.5),
        };

        let num_skip_back_samples =
            (DEFAULT_SAMPLE_RATE * length_factor * self.buffer_size_secs).ceil() as usize;

        let mut comb_filter = CombFilter::new(num_skip_back_samples, OnePoleLowPass::default());

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            let cutoff = cutoff.next(control);
            let feedback = feedback.next(control);

            let low_pass = comb_filter.feedback_fn();
            low_pass.set_cutoff(cutoff, DEFAULT_SAMPLE_RATE);
            low_pass.set_feedback(feedback * feedback_factor);

            let num_samples_to_skip_back = DEFAULT_SAMPLE_RATE * length_factor / frequency
                - low_pass.intrinsic_delay_samples();

            let fract_offset =
                (num_samples_to_skip_back / num_skip_back_samples as f64).clamp(0.0, 1.0);

            buffers.read_1_and_write(&in_buffer, &mut out_spec, control, |input| {
                comb_filter.process_sample_fract_with_limit(fract_offset, input)
            })
        })
    }
}
