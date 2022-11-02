use std::f64::consts::TAU;

use magnetron::{
    buffer::BufferWriter,
    spec::{Creator, Spec},
    waveform::Stage,
};
use serde::{Deserialize, Serialize};

use super::{AutomationSpec, InBufferSpec, OutSpec};

#[derive(Clone, Deserialize, Serialize)]
pub enum OscillatorKind {
    Sin,
    Sin3,
    Triangle,
    Square,
    Sawtooth,
}

impl OscillatorKind {
    pub fn run_oscillator<F: OscillatorRunner>(&self, oscillator_runner: F) -> F::Result {
        match self {
            OscillatorKind::Sin => {
                oscillator_runner.apply_oscillator_fn(|phase: f64| (phase * TAU).sin())
            }
            OscillatorKind::Sin3 => oscillator_runner.apply_oscillator_fn(|phase: f64| {
                let sin = (phase * TAU).sin();
                sin * sin * sin
            }),
            OscillatorKind::Triangle => oscillator_runner.apply_oscillator_fn(|phase: f64| {
                (((0.75 + phase).fract() - 0.5).abs() - 0.25) * 4.0
            }),
            OscillatorKind::Square => {
                oscillator_runner.apply_oscillator_fn(|phase: f64| (0.5 - phase).signum())
            }
            OscillatorKind::Sawtooth => oscillator_runner
                .apply_oscillator_fn(|phase: f64| ((0.5 + phase).fract() - 0.5) * 2.0),
        }
    }
}

pub trait OscillatorRunner {
    type Result;

    fn apply_oscillator_fn(
        &self,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result;
}

#[derive(Deserialize, Serialize)]
pub struct OscillatorSpec<A> {
    pub kind: OscillatorKind,
    pub frequency: A,
    pub phase: Option<A>,
    #[serde(flatten)]
    pub modulation: Modulation,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "modulation")]
pub enum Modulation {
    None,
    ByPhase { mod_buffer: InBufferSpec },
    ByFrequency { mod_buffer: InBufferSpec },
}

impl<A: AutomationSpec> Spec for OscillatorSpec<A> {
    type Created = Stage<A::Context>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        self.kind.run_oscillator(StageOscillatorRunner {
            spec: self,
            creator,
        })
    }
}

struct StageOscillatorRunner<'a, A> {
    spec: &'a OscillatorSpec<A>,
    creator: &'a Creator,
}

impl<A: AutomationSpec> OscillatorRunner for StageOscillatorRunner<'_, A> {
    type Result = Stage<A::Context>;

    fn apply_oscillator_fn(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result {
        let out_buffer = self.spec.out_spec.out_buffer.buffer();

        match &self.spec.modulation {
            Modulation::None => {
                let mut phase = 0.0;
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
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
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
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
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
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
}

impl<A: AutomationSpec> StageOscillatorRunner<'_, A> {
    fn apply_modulation_fn(
        &self,
        mut modulation_fn: impl FnMut(&mut BufferWriter, f64, f64) + Send + 'static,
    ) -> Stage<A::Context> {
        let mut last_phase = 0.0;
        self.creator.create_stage(
            (
                &self.spec.out_spec.out_level,
                &self.spec.frequency,
                &self.spec.phase,
            ),
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

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    struct TestOscillatorRunner;

    impl OscillatorRunner for TestOscillatorRunner {
        type Result = Box<dyn FnMut(f64) -> f64 + Send + 'static>;

        fn apply_oscillator_fn(
            &self,
            oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        ) -> Self::Result {
            Box::new(oscillator_fn)
        }
    }

    #[test]
    fn oscillator_correctness() {
        let eps = 1e-10;

        let mut sin = OscillatorKind::Sin.run_oscillator(TestOscillatorRunner);
        let mut sin3 = OscillatorKind::Sin3.run_oscillator(TestOscillatorRunner);
        let mut triangle = OscillatorKind::Triangle.run_oscillator(TestOscillatorRunner);
        let mut square = OscillatorKind::Square.run_oscillator(TestOscillatorRunner);
        let mut sawtooth = OscillatorKind::Sawtooth.run_oscillator(TestOscillatorRunner);

        assert_approx_eq!(sin(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin(1.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin(3.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin(5.0 / 8.0), -(1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin(7.0 / 8.0), -(1.0f64 / 2.0).sqrt());

        assert_approx_eq!(sin3(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(1.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin3(3.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(5.0 / 8.0), -(1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin3(7.0 / 8.0), -(1.0f64 / 8.0).sqrt());

        assert_approx_eq!(triangle(0.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(1.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(2.0 / 8.0), 1.0);
        assert_approx_eq!(triangle(3.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(4.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(5.0 / 8.0), -0.5);
        assert_approx_eq!(triangle(6.0 / 8.0), -1.0);
        assert_approx_eq!(triangle(7.0 / 8.0), -0.5);

        assert_approx_eq!(square(0.0 / 8.0 + eps), 1.0);
        assert_approx_eq!(square(1.0 / 8.0), 1.0);
        assert_approx_eq!(square(2.0 / 8.0), 1.0);
        assert_approx_eq!(square(3.0 / 8.0), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(square(5.0 / 8.0), -1.0);
        assert_approx_eq!(square(6.0 / 8.0), -1.0);
        assert_approx_eq!(square(7.0 / 8.0), -1.0);
        assert_approx_eq!(square(8.0 / 8.0 - eps), -1.0);

        assert_approx_eq!(sawtooth(0.0 / 8.0), 0.0);
        assert_approx_eq!(sawtooth(1.0 / 8.0), 0.25);
        assert_approx_eq!(sawtooth(2.0 / 8.0), 0.5);
        assert_approx_eq!(sawtooth(3.0 / 8.0), 0.75);
        assert_approx_eq!(sawtooth(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(sawtooth(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(sawtooth(5.0 / 8.0), -0.75);
        assert_approx_eq!(sawtooth(6.0 / 8.0), -0.5);
        assert_approx_eq!(sawtooth(7.0 / 8.0), -0.25);
    }
}
