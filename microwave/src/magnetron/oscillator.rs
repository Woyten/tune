use serde::{Deserialize, Serialize};

use super::{
    functions,
    waveform::{AutomationSpec, Creator, InBufferSpec, OutSpec, Spec, Stage},
};

#[derive(Deserialize, Serialize)]
pub struct Oscillator<A> {
    pub kind: OscillatorKind,
    pub frequency: A,
    #[serde(flatten)]
    pub modulation: Modulation,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
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
    ByPhase { mod_buffer: InBufferSpec },
    ByFrequency { mod_buffer: InBufferSpec },
}

impl<A: AutomationSpec> Spec for Oscillator<A> {
    type Created = Stage<A>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(creator, functions::sin),
            OscillatorKind::Sin3 => self.apply_signal_fn(creator, functions::sin3),
            OscillatorKind::Triangle => self.apply_signal_fn(creator, functions::triangle),
            OscillatorKind::Square => self.apply_signal_fn(creator, functions::square),
            OscillatorKind::Sawtooth => self.apply_signal_fn(creator, functions::sawtooth),
        }
    }
}

impl<A: AutomationSpec> Oscillator<A> {
    fn apply_signal_fn(
        &self,
        creator: &Creator,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Stage<A> {
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
    ) -> Stage<A> {
        let out_buffer = self.out_spec.out_buffer.buffer();

        creator.create_stage(
            (&self.out_spec.out_level, &self.frequency),
            move |buffers, (out_level, frequency)| {
                let d_phase = frequency * buffers.sample_width_secs;

                buffers.read_0_and_write(out_buffer, out_level, || {
                    let signal = oscillator_fn(phase);
                    phase = (phase + d_phase).rem_euclid(1.0);
                    signal
                })
            },
        )
    }

    fn apply_variable_phase(
        &self,
        creator: &Creator,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: &InBufferSpec,
    ) -> Stage<A> {
        let in_buffer = in_buffer.buffer();
        let out_buffer = self.out_spec.out_buffer.buffer();

        let mut phase = 0.0;
        creator.create_stage(
            (&self.out_spec.out_level, &self.frequency),
            move |buffers, (out_level, frequency)| {
                let d_phase = frequency * buffers.sample_width_secs;

                buffers.read_1_and_write(in_buffer, out_buffer, out_level, |s| {
                    let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                    phase = (phase + d_phase).rem_euclid(1.0);
                    signal
                })
            },
        )
    }

    fn apply_variable_frequency(
        &self,
        creator: &Creator,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: &InBufferSpec,
    ) -> Stage<A> {
        let in_buffer = in_buffer.buffer();
        let out_buffer = self.out_spec.out_buffer.buffer();

        let mut phase = 0.0;
        creator.create_stage(
            (&self.out_spec.out_level, &self.frequency),
            move |buffers, (out_level, frequency)| {
                let sample_width_secs = buffers.sample_width_secs;
                buffers.read_1_and_write(in_buffer, out_buffer, out_level, |s| {
                    let signal = oscillator_fn(phase);
                    phase = (phase + sample_width_secs * (frequency + s)).rem_euclid(1.0);
                    signal
                })
            },
        )
    }
}
