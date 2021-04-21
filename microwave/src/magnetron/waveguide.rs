use serde::{Deserialize, Serialize};

use crate::audio::DEFAULT_SAMPLE_RATE;

use super::{
    control::Controller,
    source::LfSource,
    util::{CombFilter, OnePoleLowPass},
    waveform::{OutSpec, Stage},
    WaveformControl,
};

#[derive(Clone, Deserialize, Serialize)]
pub struct WaveguideSpec<C> {
    pub buffer_size_secs: f64,
    pub frequency: LfSource<C>,
    pub cutoff: LfSource<C>,
    pub feedback: LfSource<C>,
    pub pluck_location: LfSource<C>,
    #[serde(flatten)]
    pub out_spec: OutSpec<C>,
}

impl<C: Controller> WaveguideSpec<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let mut samples_processed = 0;

        let mut pluck_location = self.pluck_location.clone();

        self.create_stage_internal(move |frequency, control| {
            let pluck_location = pluck_location.next(control);
            let counter_wave_at =
                (pluck_location.max(0.0).min(1.0) * DEFAULT_SAMPLE_RATE / 2.0 / frequency).round()
                    as usize;

            if samples_processed > counter_wave_at {
                samples_processed += 1;
                0.0
            } else if samples_processed == 0 || samples_processed == counter_wave_at {
                samples_processed += 1;
                1.0
            } else {
                samples_processed += 1;
                0.0
            }
        })
    }

    fn create_stage_internal(
        &self,
        mut exciter: impl FnMut(f64, &WaveformControl<C::Storage>) -> f64 + Send + 'static,
    ) -> Stage<C::Storage> {
        let mut out_spec = self.out_spec.clone();
        let num_samples_in_buffer = (self.buffer_size_secs * DEFAULT_SAMPLE_RATE).ceil() as usize;

        let low_pass = OnePoleLowPass::new(0.0, DEFAULT_SAMPLE_RATE);
        let mut comb_filter = CombFilter::new(num_samples_in_buffer, 0.0, low_pass);

        let mut frequency = self.frequency.clone();
        let mut cutoff = self.cutoff.clone();
        let mut feedback = self.feedback.clone();

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            let cutoff = cutoff.next(control);
            let feedback = feedback.next(control);

            comb_filter.set_feedback(-feedback);
            let low_pass = comb_filter.feedback_fn();
            low_pass.set_cutoff(cutoff, DEFAULT_SAMPLE_RATE);
            let intrinsic_delay = low_pass.intrinsic_delay_samples();

            let num_samples_to_skip_back = DEFAULT_SAMPLE_RATE / 2.0 / frequency - intrinsic_delay;

            let offset = num_samples_to_skip_back / num_samples_in_buffer as f64;

            buffers.read_0_and_write(&mut out_spec, control, || {
                comb_filter
                    .process_sample_fract(offset.max(0.0).min(1.0), exciter(frequency, control))
            })
        })
    }
}
