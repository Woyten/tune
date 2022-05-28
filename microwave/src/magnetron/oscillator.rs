use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    functions,
    source::LfSource,
    waveform::{Creator, InBuffer, OutSpec, Spec, Stage},
};

#[derive(Deserialize, Serialize)]
pub struct Oscillator<C> {
    pub kind: OscillatorKind,
    pub frequency: LfSource<C>,
    #[serde(flatten)]
    pub modulation: Modulation,
    #[serde(flatten)]
    pub out_spec: OutSpec<C>,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum OscillatorKind {
    Sin,
    Sin3,
    Triangle,
    Square,
    Sawtooth,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "modulation")]
pub enum Modulation {
    None,
    ByPhase { mod_buffer: InBuffer },
    ByFrequency { mod_buffer: InBuffer },
}

impl<C: Controller> Spec for &Oscillator<C> {
    type Created = Stage<C::Storage>;

    fn use_creator(self, creator: &Creator) -> Self::Created {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(creator, functions::sin),
            OscillatorKind::Sin3 => self.apply_signal_fn(creator, functions::sin3),
            OscillatorKind::Triangle => self.apply_signal_fn(creator, functions::triangle),
            OscillatorKind::Square => self.apply_signal_fn(creator, functions::square),
            OscillatorKind::Sawtooth => self.apply_signal_fn(creator, functions::sawtooth),
        }
    }
}

impl<C: Controller> Oscillator<C> {
    fn apply_signal_fn(
        &self,
        creator: &Creator,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Stage<C::Storage> {
        match &self.modulation {
            Modulation::None => self.apply_no_modulation(creator, oscillator_fn, 0.0),
            Modulation::ByPhase { mod_buffer } => {
                self.apply_variable_phase(creator, oscillator_fn, mod_buffer)
            }
            Modulation::ByFrequency { mod_buffer } => {
                self.apply_variable_frequency(creator, oscillator_fn, mod_buffer)
            }
        }
    }

    fn apply_no_modulation(
        &self,
        creator: &Creator,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage<C::Storage> {
        let (mut frequency, mut output) = creator.create((&self.frequency, &self.out_spec));

        Box::new(move |buffers, control| {
            let d_phase = frequency(control) * buffers.sample_width_secs;
            buffers.read_0_and_write(&mut output, control, || {
                let signal = oscillator_fn(phase);
                phase = (phase + d_phase).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_phase(
        &self,
        creator: &Creator,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: &InBuffer,
    ) -> Stage<C::Storage> {
        let (mut frequency, mut output, input) =
            creator.create((&self.frequency, &self.out_spec, in_buffer));

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let d_phase = frequency(control) * buffers.sample_width_secs;
            buffers.read_1_and_write(&input, &mut output, control, |s| {
                let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                phase = (phase + d_phase).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        creator: &Creator,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: &InBuffer,
    ) -> Stage<C::Storage> {
        let (mut frequency, mut output, input) =
            creator.create((&self.frequency, &self.out_spec, in_buffer));

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let sample_width_secs = buffers.sample_width_secs;
            let frequency = frequency(control);
            buffers.read_1_and_write(&input, &mut output, control, |s| {
                let signal = oscillator_fn(phase);
                phase = (phase + sample_width_secs * (frequency + s)).rem_euclid(1.0);
                signal
            })
        })
    }
}
