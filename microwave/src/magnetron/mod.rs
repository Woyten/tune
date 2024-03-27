use magnetron::{
    automation::AutomatableValue, buffer::BufferIndex, creator::Creator, stage::Stage,
};
use serde::{Deserialize, Serialize};

use self::{
    effects::EffectSpec,
    filter::FilterSpec,
    noise::NoiseSpec,
    oscillator::{ModOscillatorSpec, OscillatorSpec},
    waveguide::WaveguideSpec,
};

mod util;

pub mod effects;
pub mod filter;
pub mod noise;
pub mod oscillator;
pub mod source;
pub mod waveform;
pub mod waveguide;

#[derive(Clone, Deserialize, Serialize)]
pub struct TemplateSpec<A> {
    pub name: String,
    pub value: A,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "stage_type")]
pub enum StageType<A> {
    Generator(GeneratorSpec<A>),
    Processor(ProcessorSpec<A>),
    MergeProcessor(MergeProcessorSpec<A>),
    StereoProcessor(StereoProcessorSpec<A>),
}

impl<A: AutomatableValue> StageType<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A> {
        match self {
            StageType::Generator(spec) => spec.use_creator(creator),
            StageType::Processor(spec) => spec.use_creator(creator),
            StageType::MergeProcessor(spec) => spec.use_creator(creator),
            StageType::StereoProcessor(spec) => spec.use_creator(creator),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GeneratorSpec<A> {
    pub out_buffer: usize,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub generator_type: GeneratorType<A>,
}

impl<A: AutomatableValue> GeneratorSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A> {
        let out_buffer = BufferIndex::Internal(self.out_buffer);
        self.generator_type
            .use_creator(creator, out_buffer, self.out_level.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProcessorSpec<A> {
    pub in_buffer: usize,
    pub in_external: Option<bool>,
    pub out_buffer: usize,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub processor_type: ProcessorType<A>,
}

impl<A: AutomatableValue> ProcessorSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A> {
        let in_buffer = to_in_buffer_index(self.in_buffer, self.in_external.unwrap_or_default());
        let out_buffer = to_out_buffer_index(self.out_buffer);
        self.processor_type
            .use_creator(creator, in_buffer, out_buffer, self.out_level.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MergeProcessorSpec<A> {
    pub in_buffers: (usize, usize),
    pub in_external: Option<(bool, bool)>,
    pub out_buffer: usize,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub processor_type: MergeProcessorType,
}

impl<A: AutomatableValue> MergeProcessorSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A> {
        let in_buffers = (
            to_in_buffer_index(self.in_buffers.0, self.in_external.unwrap_or_default().0),
            to_in_buffer_index(self.in_buffers.1, self.in_external.unwrap_or_default().1),
        );
        let out_buffer = to_out_buffer_index(self.out_buffer);
        self.processor_type
            .use_creator(creator, in_buffers, out_buffer, self.out_level.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StereoProcessorSpec<A> {
    pub in_buffers: (usize, usize),
    pub in_external: Option<(bool, bool)>,
    pub out_buffers: (usize, usize),
    pub out_levels: Option<(A, A)>,
    #[serde(flatten)]
    pub processor_type: StereoProcessorType<A>,
}

impl<A: AutomatableValue> StereoProcessorSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A> {
        let in_buffers = (
            to_in_buffer_index(self.in_buffers.0, self.in_external.unwrap_or_default().0),
            to_in_buffer_index(self.in_buffers.1, self.in_external.unwrap_or_default().1),
        );
        let out_buffers = (
            to_out_buffer_index(self.out_buffers.0),
            to_out_buffer_index(self.out_buffers.1),
        );
        self.processor_type
            .use_creator(creator, in_buffers, out_buffers, self.out_levels.as_ref())
    }
}

pub fn to_in_buffer_index(in_buffer: usize, in_external: bool) -> BufferIndex {
    if in_external {
        BufferIndex::External(in_buffer)
    } else {
        BufferIndex::Internal(in_buffer)
    }
}

pub fn to_out_buffer_index(out_buffer: usize) -> BufferIndex {
    BufferIndex::Internal(out_buffer)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "generator_type")]
pub enum GeneratorType<A> {
    Oscillator(OscillatorSpec<A>),
    Noise(NoiseSpec),
}

impl<A: AutomatableValue> GeneratorType<A> {
    fn use_creator(
        &self,
        creator: &Creator<A>,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match self {
            GeneratorType::Oscillator(spec) => spec.use_creator(creator, out_buffer, out_level),
            GeneratorType::Noise(spec) => spec.use_creator(creator, out_buffer, out_level),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum ProcessorType<A> {
    Pass,
    Pow3,
    Clip { limit: A },

    Filter(FilterSpec<A>),
    Oscillator(ModOscillatorSpec<A>),
    Waveguide(WaveguideSpec<A>),
}

impl<A: AutomatableValue> ProcessorType<A> {
    fn use_creator(
        &self,
        creator: &Creator<A>,
        in_buffer: BufferIndex,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match self {
            ProcessorType::Pass => creator.create_stage(out_level, move |buffers, out_level| {
                buffers.read_1_write_1(in_buffer, out_buffer, out_level, |s| s)
            }),
            ProcessorType::Pow3 => creator.create_stage(out_level, move |buffers, out_level| {
                buffers.read_1_write_1(in_buffer, out_buffer, out_level, |s| s * s * s)
            }),
            ProcessorType::Clip { limit } => {
                creator.create_stage((out_level, limit), move |buffers, (out_level, limit)| {
                    let limit = limit.abs();
                    buffers.read_1_write_1(in_buffer, out_buffer, out_level, |s| {
                        s.max(-limit).min(limit)
                    })
                })
            }
            ProcessorType::Filter(spec) => {
                spec.use_creator(creator, in_buffer, out_buffer, out_level)
            }
            ProcessorType::Oscillator(spec) => {
                spec.use_creator(creator, in_buffer, out_buffer, out_level)
            }
            ProcessorType::Waveguide(spec) => {
                spec.use_creator(creator, in_buffer, out_buffer, out_level)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum MergeProcessorType {
    RingModulator,
}

impl MergeProcessorType {
    fn use_creator<A: AutomatableValue>(
        &self,
        creator: &Creator<A>,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match self {
            MergeProcessorType::RingModulator => {
                creator.create_stage(out_level, move |buffers, out_level| {
                    buffers.read_2_write_1(
                        in_buffers,
                        out_buffer,
                        out_level,
                        |source_1, source_2| source_1 * source_2,
                    )
                })
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum StereoProcessorType<A> {
    Effect(EffectSpec<A>),
}

impl<A: AutomatableValue> StereoProcessorType<A> {
    fn use_creator(
        &self,
        creator: &Creator<A>,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<&(A, A)>,
    ) -> Stage<A> {
        match self {
            StereoProcessorType::Effect(spec) => {
                spec.use_creator(creator, in_buffers, out_buffers, out_levels)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, f64::consts::TAU, iter};

    use assert_approx_eq::assert_approx_eq;
    use magnetron::{
        automation::ContextInfo, envelope::EnvelopeSpec, stage::StageActivity, Magnetron,
    };

    use crate::profile::WaveformAutomatableValue;

    use super::{
        source::{LfSource, LfSourceExpr},
        waveform::{WaveformProperties, WaveformProperty, WaveformSpec},
        *,
    };

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut test = MagnetronTest::new(&[]);

        test.check_audio_out_content(0, |_| 0.0);

        test.process(128, vec![]);
        test.check_audio_out_content(128, |_| 0.0);

        test.process(256, vec![]);
        test.check_audio_out_content(256, |_| 0.0);

        test.process(64, vec![]);
        test.check_audio_out_content(64, |_| 0.0);
    }

    #[test]
    fn empty_spec() {
        let waveform = "[]";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |_| 0.0);
    }

    #[test]
    fn write_waveform_and_clear() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

        test.process(128, vec![]);
        test.check_audio_out_content(128, |_| 0.0);
    }

    #[test]
    fn mix_two_waveforms() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new(&[waveform, waveform]);

        test.process(NUM_SAMPLES, vec![(440.0, 0.7), (660.0, 0.8)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            t * (0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin())
        });
    }

    #[test]
    fn apply_optional_phase() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  phase: 1.0
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        // 441 Hz because the phase modulates from 0.0 (initial) to 1.0 within 1s (buffer size) leading to one additional oscillation
        test.check_audio_out_content(NUM_SAMPLES, move |t| t * (441.0 * t * TAU).sin());
    }

    #[test]
    fn modulate_by_frequency() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 0
  out_level: 440.0
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 330.0
- stage_type: Processor
  in_buffer: 0
  out_buffer: 5
  processor_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
  modulation: ByFrequency";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(550.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, {
            let mut mod_phase = 0.0;
            move |t| {
                let signal = ((550.0 * t + mod_phase) * TAU).sin();
                mod_phase += (330.0 * TAU * t).sin() * 440.0 * SAMPLE_WIDTH_SECS;
                t * signal
            }
        });
    }

    #[test]
    fn modulate_by_phase() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 0
  out_level: 0.44
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 330.0
- stage_type: Processor
  in_buffer: 0
  out_buffer: 5
  processor_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
  modulation: ByPhase";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(550.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            t * ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 0
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
- stage_type: Generator
  out_buffer: 1
  generator_type: Oscillator
  oscillator_type: Sin
  frequency:
    Mul: [1.5, WaveformPitch]
- stage_type: MergeProcessor
  in_buffers: [0, 1]
  out_buffer: 5
  processor_type: RingModulator";

        let mut test = MagnetronTest::new(&[waveform]);

        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            t * (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_attack_time() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new_with_envelope(
            &[waveform],
            EnvelopeSpec {
                fadeout: LfSource::Value(0.0),
                attack_time: LfSource::template("Velocity"),
                decay_rate: LfSource::Value(1.0),
                release_time: LfSource::Value(1.0),
                in_buffer: 5,
                out_buffers: (6, 7),
                out_levels: None,
            },
        );

        // attack part 1
        test.process(NUM_SAMPLES, vec![(440.0, 3.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| 1.0 / 3.0 * t * (TAU * 440.0 * t).sin());

        // attack part 2
        test.process(NUM_SAMPLES, vec![(440.0, 3.0 / 2.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (1.0 / 3.0 + 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });

        // decay part
        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_decay_time() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new_with_envelope(
            &[waveform],
            EnvelopeSpec {
                in_buffer: 5,
                out_buffers: (6, 7),
                out_levels: None,
                fadeout: LfSource::Value(0.0),
                attack_time: LfSource::Value(1.0),
                decay_rate: LfSource::template("Velocity"),
                release_time: LfSource::Value(1.0),
            },
        );

        // attack part
        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

        // decay part 1
        test.process(NUM_SAMPLES, vec![(440.0, 1.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
        });

        // decay part 2
        test.process(NUM_SAMPLES, vec![(440.0, 2.0)]);
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (1.0 / 2.0 - 3.0 / 8.0 * t) * (TAU * 440.0 * t).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_fadeout() {
        let waveform = r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

        let mut test = MagnetronTest::new_with_envelope(
            &[waveform],
            EnvelopeSpec {
                in_buffer: 5,
                out_buffers: (6, 7),
                out_levels: None,
                fadeout: LfSource::template("Velocity"),
                attack_time: LfSource::Value(1.0),
                decay_rate: LfSource::Value(0.0),
                release_time: LfSource::Value(3.0),
            },
        );

        // attack part
        assert_eq!(
            test.process(NUM_SAMPLES, vec![(440.0, 0.0)]),
            StageActivity::External
        );
        test.check_audio_out_content(NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

        // sustain part
        assert_eq!(
            test.process(NUM_SAMPLES, vec![(440.0, 0.0)]),
            StageActivity::External
        );
        test.check_audio_out_content(NUM_SAMPLES, |t| (TAU * 440.0 * t).sin());

        // release part 1
        assert_eq!(
            test.process(NUM_SAMPLES, vec![(440.0, 1.0)]),
            StageActivity::External
        );
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (1.0 - 1.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });

        // release part 1
        assert_eq!(
            test.process(NUM_SAMPLES, vec![(440.0, 2.0)]),
            StageActivity::Internal
        );
        test.check_audio_out_content(NUM_SAMPLES, |t| {
            (2.0 / 3.0 - 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });
    }

    struct MagnetronTest {
        magnetron: Magnetron,
        stage: Stage<TestContext>,
        result_l: Vec<f64>,
        result_r: Vec<f64>,
    }

    impl MagnetronTest {
        fn new(waveform_specs: &[&str]) -> Self {
            Self::new_with_envelope(
                waveform_specs,
                EnvelopeSpec {
                    in_buffer: 5,
                    out_buffers: (6, 7),
                    out_levels: Some((
                        LfSource::template("Velocity"),
                        LfSource::template("Velocity"),
                    )),
                    fadeout: LfSource::Value(0.0),
                    attack_time: LfSource::Value(0.0),
                    decay_rate: LfSource::Value(0.0),
                    release_time: LfSource::Value(0.0),
                },
            )
        }

        fn new_with_envelope(
            waveform_specs: &[&str],
            envelope_spec: EnvelopeSpec<WaveformAutomatableValue>,
        ) -> Self {
            let creator = Creator::new(HashMap::from([
                (
                    "WaveformPitch".to_owned(),
                    LfSourceExpr::Property(WaveformProperty::WaveformPitch).wrap(),
                ),
                (
                    "Velocity".to_owned(),
                    LfSourceExpr::Property(WaveformProperty::Velocity).wrap(),
                ),
            ]));

            let envelopes = HashMap::from([("test envelope".to_owned(), envelope_spec)]);
            let mut waveforms: Vec<_> = waveform_specs
                .iter()
                .map(|spec| {
                    WaveformSpec {
                        name: String::new(),
                        envelope: "test envelope".to_owned(),
                        stages: serde_yaml::from_str(spec).unwrap(),
                    }
                    .use_creator(&creator, &envelopes)
                })
                .collect();

            let mut magnetron = create_magnetron();

            let stage = Stage::new(move |buffers, _, context: &[(f64, f64)]| {
                iter::zip(context, &mut waveforms)
                    .map(|((pitch_hz, velocity), waveform)| {
                        magnetron.prepare_nested(buffers).process(
                            (
                                &WaveformProperties::initial(*pitch_hz, *velocity),
                                &Default::default(),
                            ),
                            waveform,
                        )
                    })
                    .max()
                    .unwrap_or_default()
            });

            Self {
                magnetron: create_magnetron(),
                stage,
                result_l: Vec::new(),
                result_r: Vec::new(),
            }
        }

        fn process(&mut self, num_samples: usize, render_passes: Vec<(f64, f64)>) -> StageActivity {
            let buffers = &mut self.magnetron.prepare(num_samples, false);
            let activity = buffers.process(&*render_passes, [&mut self.stage]);
            self.result_l.clear();
            self.result_r.clear();
            self.result_l.extend(buffers.read(BufferIndex::Internal(6)));
            self.result_r.extend(buffers.read(BufferIndex::Internal(7)));
            activity
        }

        fn check_audio_out_content(&self, num_samples: usize, mut f: impl FnMut(f64) -> f64) {
            check_sampled_signal(&self.result_l, num_samples, &mut f);
            check_sampled_signal(&self.result_r, num_samples, &mut f);
        }
    }

    struct TestContext;

    impl ContextInfo for TestContext {
        type Context<'a> = &'a [(f64, f64)];
    }

    fn create_magnetron() -> Magnetron {
        Magnetron::new(SAMPLE_WIDTH_SECS, 8, 100000)
    }

    fn check_sampled_signal(buffer: &[f64], num_samples: usize, mut f: impl FnMut(f64) -> f64) {
        assert_eq!(buffer.len(), num_samples);

        let mut time = 0.0;
        for sample in buffer {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_WIDTH_SECS;
        }
    }
}
