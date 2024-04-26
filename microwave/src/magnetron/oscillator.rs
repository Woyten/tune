use std::f64::consts::TAU;

use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::{BufferIndex, BufferWriter},
    stage::{Stage, StageActivity},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum OscillatorType {
    Sin,
    Sin3,
    Triangle,
    Square,
    Sawtooth,
}

impl OscillatorType {
    pub fn run_oscillator<F: OscillatorRunner>(&self, mut oscillator_runner: F) -> F::Result {
        match self {
            OscillatorType::Sin => {
                oscillator_runner.apply_oscillator_fn(|phase: f64| (phase * TAU).sin())
            }
            OscillatorType::Sin3 => oscillator_runner.apply_oscillator_fn(|phase: f64| {
                let sin = (phase * TAU).sin();
                sin * sin * sin
            }),
            OscillatorType::Triangle => oscillator_runner.apply_oscillator_fn(|phase: f64| {
                (((0.75 + phase).fract() - 0.5).abs() - 0.25) * 4.0
            }),
            OscillatorType::Square => {
                oscillator_runner
                    .apply_oscillator_fn(|phase: f64| if phase < 0.5 { 1.0 } else { -1.0 })
            }
            OscillatorType::Sawtooth => oscillator_runner
                .apply_oscillator_fn(|phase: f64| ((0.5 + phase).fract() - 0.5) * 2.0),
        }
    }
}

pub trait OscillatorRunner {
    type Result;

    fn apply_oscillator_fn(
        &mut self,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OscillatorSpec<A> {
    pub oscillator_type: OscillatorType,
    pub frequency: A,
    pub phase: Option<A>,
}

impl<A: AutomatableParam> OscillatorSpec<A> {
    pub fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        self.oscillator_type.run_oscillator(StageOscillatorRunner {
            factory,
            modulation: None,
            out_buffer,
            out_level,
            spec: self,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModOscillatorSpec<A> {
    #[serde(flatten)]
    pub spec: OscillatorSpec<A>,
    pub modulation: Modulation,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum Modulation {
    ByPhase,
    ByFrequency,
}

impl<A: AutomatableParam> ModOscillatorSpec<A> {
    pub fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffer: BufferIndex,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        self.spec
            .oscillator_type
            .run_oscillator(StageOscillatorRunner {
                factory,
                modulation: Some((in_buffer, self.modulation)),
                out_buffer,
                out_level,
                spec: &self.spec,
            })
    }
}

struct StageOscillatorRunner<'a, A: AutomatableParam> {
    factory: &'a mut AutomationFactory<A>,
    modulation: Option<(BufferIndex, Modulation)>,
    out_buffer: BufferIndex,
    out_level: Option<&'a A>,
    spec: &'a OscillatorSpec<A>,
}

impl<A: AutomatableParam> OscillatorRunner for StageOscillatorRunner<'_, A> {
    type Result = Stage<A>;

    fn apply_oscillator_fn(
        &mut self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Self::Result {
        let out_buffer = self.out_buffer;

        match &self.modulation {
            None => {
                let mut phase = 0.0;
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
                    buffers.read_0_write_1(out_buffer, out_level, || {
                        let signal = oscillator_fn(phase);
                        phase = (phase + d_phase).rem_euclid(1.0);
                        signal
                    })
                })
            }
            &Some((mod_buffer, Modulation::ByPhase)) => {
                let mut phase = 0.0;
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
                    buffers.read_1_write_1(mod_buffer, out_buffer, out_level, |s| {
                        let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                        phase = (phase + d_phase).rem_euclid(1.0);
                        signal
                    })
                })
            }
            &Some((mod_buffer, Modulation::ByFrequency)) => {
                let mut phase = 0.0;
                self.apply_modulation_fn(move |buffers, out_level, d_phase| {
                    let sample_width_secs = buffers.sample_width_secs();
                    buffers.read_1_write_1(mod_buffer, out_buffer, out_level, |s| {
                        let signal = oscillator_fn(phase);
                        phase = (phase + d_phase + s * sample_width_secs).rem_euclid(1.0);
                        signal
                    })
                })
            }
        }
    }
}

impl<A: AutomatableParam> StageOscillatorRunner<'_, A> {
    fn apply_modulation_fn(
        &mut self,
        mut modulation_fn: impl FnMut(&mut BufferWriter, Option<f64>, f64) -> StageActivity
            + Send
            + 'static,
    ) -> Stage<A> {
        let mut saved_phase = 0.0;
        self.factory
            .automate((&self.out_level, &self.spec.frequency, &self.spec.phase))
            .into_stage(move |buffers, (out_level, frequency, phase)| {
                let to_phase = phase.unwrap_or_default();

                let d_phase = frequency * buffers.sample_width_secs()
                    + (to_phase - saved_phase) / buffers.buffer_len() as f64;

                saved_phase = to_phase;

                modulation_fn(buffers, out_level, d_phase)
            })
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
            &mut self,
            oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        ) -> Self::Result {
            Box::new(oscillator_fn)
        }
    }

    #[test]
    fn oscillator_correctness() {
        let eps = 1e-10;

        let mut sin = OscillatorType::Sin.run_oscillator(TestOscillatorRunner);
        let mut sin3 = OscillatorType::Sin3.run_oscillator(TestOscillatorRunner);
        let mut triangle = OscillatorType::Triangle.run_oscillator(TestOscillatorRunner);
        let mut square = OscillatorType::Square.run_oscillator(TestOscillatorRunner);
        let mut sawtooth = OscillatorType::Sawtooth.run_oscillator(TestOscillatorRunner);

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

        assert_approx_eq!(square(0.0 / 8.0), 1.0);
        assert_approx_eq!(square(1.0 / 8.0), 1.0);
        assert_approx_eq!(square(2.0 / 8.0), 1.0);
        assert_approx_eq!(square(3.0 / 8.0), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(square(4.0 / 8.0), -1.0);
        assert_approx_eq!(square(5.0 / 8.0), -1.0);
        assert_approx_eq!(square(6.0 / 8.0), -1.0);
        assert_approx_eq!(square(7.0 / 8.0), -1.0);
        assert_approx_eq!(square(8.0 / 8.0 - eps), -1.0);

        assert_approx_eq!(sawtooth(0.0 / 8.0), 0.0);
        assert_approx_eq!(sawtooth(1.0 / 8.0), 0.25);
        assert_approx_eq!(sawtooth(2.0 / 8.0), 0.5);
        assert_approx_eq!(sawtooth(3.0 / 8.0), 0.75);
        assert_approx_eq!(sawtooth(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(sawtooth(4.0 / 8.0), -1.0);
        assert_approx_eq!(sawtooth(5.0 / 8.0), -0.75);
        assert_approx_eq!(sawtooth(6.0 / 8.0), -0.5);
        assert_approx_eq!(sawtooth(7.0 / 8.0), -0.25);
    }
}
