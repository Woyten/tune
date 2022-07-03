use magnetron::{
    automation::AutomationSpec,
    buffer::BufferWriter,
    spec::{Creator, Spec},
    waveform::Stage,
};
use serde::{Deserialize, Serialize};

use super::{functions, InBufferSpec, OutSpec};

#[derive(Deserialize, Serialize)]
pub struct Oscillator<A> {
    pub kind: OscillatorKind,
    pub frequency: A,
    pub phase: Option<A>,
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
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Stage<A> {
        let out_buffer = self.out_spec.out_buffer.buffer();

        match &self.modulation {
            Modulation::None => {
                let mut phase = 0.0;
                self.apply_modulation_fn(creator, move |buffers, out_level, d_phase| {
                    buffers.read_0_and_write(out_buffer, out_level, || {
                        let signal = oscillator_fn(phase);
                        phase = (phase + d_phase).rem_euclid(1.0);
                        signal
                    });
                })
            }
            Modulation::ByPhase { mod_buffer } => {
                let mod_buffer = mod_buffer.buffer();

                let mut phase = 0.0;
                self.apply_modulation_fn(creator, move |buffers, out_level, d_phase| {
                    buffers.read_1_and_write(mod_buffer, out_buffer, out_level, |s| {
                        let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                        phase = (phase + d_phase).rem_euclid(1.0);
                        signal
                    });
                })
            }
            Modulation::ByFrequency { mod_buffer } => {
                let mod_buffer = mod_buffer.buffer();

                let mut phase = 0.0;
                self.apply_modulation_fn(creator, move |buffers, out_level, d_phase| {
                    let sample_width_secs = buffers.sample_width_secs();
                    buffers.read_1_and_write(mod_buffer, out_buffer, out_level, |s| {
                        let signal = oscillator_fn(phase);
                        phase = (phase + d_phase + s * sample_width_secs).rem_euclid(1.0);
                        signal
                    });
                })
            }
        }
    }

    fn apply_modulation_fn(
        &self,
        creator: &Creator,
        mut modulation_fn: impl FnMut(&mut BufferWriter, f64, f64) + Send + 'static,
    ) -> Stage<A> {
        let mut last_phase = 0.0;
        creator.create_stage(
            (&self.out_spec.out_level, &self.frequency, &self.phase),
            move |buffers, (out_level, frequency, phase)| {
                let phase = phase.unwrap_or_default();
                let d_phase = frequency * buffers.sample_width_secs()
                    + (phase - last_phase) / buffers.buffer_len() as f64;
                last_phase = phase;

                modulation_fn(buffers, out_level, d_phase);
            },
        )
    }
}
