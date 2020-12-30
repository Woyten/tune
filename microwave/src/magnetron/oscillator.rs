use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    functions,
    source::LfSource,
    waveform::{Destination, Source, Stage},
};

#[derive(Deserialize, Serialize)]
pub struct Oscillator<K> {
    pub kind: OscillatorKind,
    pub frequency: LfSource<K>,
    pub modulation: Modulation,
    pub destination: Destination<K>,
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
pub enum Modulation {
    None,
    ByPhase(Source),
    ByFrequency(Source),
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
            Modulation::ByPhase(source) => self.apply_variable_phase(oscillator_fn, source.clone()),
            Modulation::ByFrequency(source) => {
                self.apply_variable_frequency(oscillator_fn, source.clone())
            }
        }
    }

    fn apply_no_modulation(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut destination = self.destination.clone();

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_0(&mut destination, control, || {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_phase(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: Source,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut destination = self.destination.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_1(&mut destination, &source, control, |s| {
                let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: Source,
    ) -> Stage<C::Storage> {
        let mut destination = self.destination.clone();
        let mut frequency = self.frequency.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_1(&mut destination, &source, control, |s| {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * (frequency + s)).rem_euclid(1.0);
                signal
            })
        })
    }
}
