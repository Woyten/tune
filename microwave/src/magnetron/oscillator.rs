use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    functions,
    source::LfSource,
    waveform::{InBuffer, OutSpec, Stage},
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

impl<C: Controller> Oscillator<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(functions::sin),
            OscillatorKind::Sin3 => self.apply_signal_fn(functions::sin3),
            OscillatorKind::Triangle => self.apply_signal_fn(functions::triangle),
            OscillatorKind::Square => self.apply_signal_fn(functions::square),
            OscillatorKind::Sawtooth => self.apply_signal_fn(functions::sawtooth),
        }
    }

    fn apply_signal_fn(
        &self,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Stage<C::Storage> {
        match &self.modulation {
            Modulation::None => self.apply_no_modulation(oscillator_fn, 0.0),
            Modulation::ByPhase { mod_buffer } => {
                self.apply_variable_phase(oscillator_fn, mod_buffer.clone())
            }
            Modulation::ByFrequency { mod_buffer } => {
                self.apply_variable_frequency(oscillator_fn, mod_buffer.clone())
            }
        }
    }

    fn apply_no_modulation(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_0_and_write(&mut out_spec, control, || {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_phase(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: InBuffer,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_1_and_write(&in_buffer, &mut out_spec, control, |s| {
                let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: InBuffer,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_1_and_write(&in_buffer, &mut out_spec, control, |s| {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * (frequency + s)).rem_euclid(1.0);
                signal
            })
        })
    }
}
