use std::collections::HashMap;
use std::iter;

use assert_approx_eq::assert_approx_eq;
use magnetron::Magnetron;
use magnetron::automation::QueryInfo;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;

use crate::backend::NoteInput;
use crate::magnetron::BufferName;
use crate::magnetron::BufferNameMap;
use crate::magnetron::Factory;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::source::LfSource;
use crate::magnetron::source::LfSourceExpr;
use crate::magnetron::waveform::WaveformProperties;
use crate::magnetron::waveform::WaveformProperty;
use crate::magnetron::waveform::WaveformSpec;
use crate::profile::WaveformParam;
use crate::synth::MagnetronSpec;

pub const NUM_SAMPLES: usize = 44100;
pub const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;
pub const MAX_BUFFER_SIZE: usize = 100_000;

pub const SINE_WAVEFORM: &str = r"
- stage_type: Generator
  out_buffer: output
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch";

pub fn test_templates() -> HashMap<String, WaveformParam> {
    HashMap::from([
        (
            "WaveformPitch".to_owned(),
            LfSourceExpr::Property(WaveformProperty::WaveformPitch).wrap(),
        ),
        (
            "Velocity".to_owned(),
            LfSourceExpr::Property(WaveformProperty::Velocity).wrap(),
        ),
    ])
}

pub fn test_envelope() -> EnvelopeSpec<WaveformParam> {
    EnvelopeSpec {
        in_buffer: "output".into(),
        out_buffers: ("left".into(), "right".into()),
        out_levels: Some((
            LfSource::template("Velocity"),
            LfSource::template("Velocity"),
        )),
        fadeout: LfSource::Value(0.0),
        attack_time: LfSource::Value(0.0),
        decay_rate: LfSource::Value(0.0),
        release_time: LfSource::Value(0.0),
    }
}

fn parse_waveforms(specs: &[&str], envelope_name: &BufferName) -> Vec<WaveformSpec<WaveformParam>> {
    specs
        .iter()
        .enumerate()
        .map(|(index, spec)| WaveformSpec {
            name: format!("waveform {index}").into(),
            envelope: envelope_name.clone(),
            stages: serde_yaml::from_str(spec).unwrap(),
        })
        .collect()
}

fn create_spec(
    waveform_specs: &[&str],
    envelopes: &HashMap<BufferName, EnvelopeSpec<WaveformParam>>,
) -> MagnetronSpec {
    let envelope_name = envelopes
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "".into());

    MagnetronSpec {
        note_input: NoteInput::Foreground,
        waveforms: parse_waveforms(waveform_specs, &envelope_name),
        default_waveform: Some(0),
    }
}

pub struct AllocationFixture {
    spec: MagnetronSpec,
    templates: HashMap<String, WaveformParam>,
    envelopes: HashMap<BufferName, EnvelopeSpec<WaveformParam>>,
    pub pipeline_buffers: BufferNameMap,
}

impl AllocationFixture {
    pub fn new(waveform_specs: &[&str]) -> Self {
        Self::new_with_envelopes(waveform_specs, HashMap::new())
    }

    pub fn new_with_envelopes(
        waveform_specs: &[&str],
        envelopes: HashMap<BufferName, EnvelopeSpec<WaveformParam>>,
    ) -> Self {
        Self {
            spec: create_spec(waveform_specs, &envelopes),
            templates: test_templates(),
            envelopes,
            pipeline_buffers: BufferNameMap::default(),
        }
    }

    pub fn create_waveform(
        &self,
        factory: &mut Factory<WaveformParam>,
        index: usize,
    ) -> Vec<Stage<WaveformParam>> {
        self.spec.waveforms[index].create(factory, &self.envelopes)
    }

    pub fn allocate(&mut self) -> (Factory<WaveformParam>, usize) {
        self.spec
            .allocate_buffers(&self.templates, &self.envelopes, &mut self.pipeline_buffers)
    }
}

pub struct MagnetronFixture {
    magnetron: Magnetron,
    stage: Stage<TestContext>,
    left_index: usize,
    right_index: usize,
    result_l: Vec<f64>,
    result_r: Vec<f64>,
    num_waveform_buffers: usize,
}

impl MagnetronFixture {
    pub fn new(waveform_specs: &[&str]) -> Self {
        Self::new_with_envelope(waveform_specs, test_envelope())
    }

    pub fn new_with_envelope(
        waveform_specs: &[&str],
        envelope_spec: EnvelopeSpec<WaveformParam>,
    ) -> Self {
        Self::new_with_pipeline_buffers(waveform_specs, envelope_spec, BufferNameMap::default())
    }

    pub fn new_with_pipeline_buffers(
        waveform_specs: &[&str],
        envelope_spec: EnvelopeSpec<WaveformParam>,
        mut pipeline_buffers: BufferNameMap,
    ) -> Self {
        let templates = test_templates();
        let envelope_name: BufferName = "test envelope".into();
        let out_buffers = envelope_spec.out_buffers.clone();
        let envelopes = HashMap::from([(envelope_name, envelope_spec)]);
        let spec = create_spec(waveform_specs, &envelopes);

        let (mut factory, num_waveform_buffers) =
            spec.allocate_buffers(&templates, &envelopes, &mut pipeline_buffers);

        let left_index = pipeline_buffers.index(&out_buffers.0);
        let right_index = pipeline_buffers.index(&out_buffers.1);
        let num_pipeline_buffers = pipeline_buffers.num_buffers();

        let mut waveforms: Vec<_> = spec
            .waveforms
            .iter()
            .map(|waveform| waveform.create(&mut factory, &envelopes))
            .collect();

        let mut magnetron =
            Magnetron::new(SAMPLE_WIDTH_SECS, num_waveform_buffers, MAX_BUFFER_SIZE);

        let stage = Stage::new(move |buffers, context: &[(f64, f64)]| {
            iter::zip(context, &mut waveforms)
                .map(|((pitch_hz, velocity), waveform)| {
                    magnetron.prepare_nested(buffers).process(
                        (
                            &WaveformProperties::initial(*pitch_hz, *velocity),
                            &Default::default(),
                            &HashMap::new(),
                        ),
                        waveform,
                    )
                })
                .max()
                .unwrap_or_default()
        });

        Self {
            magnetron: Magnetron::new(SAMPLE_WIDTH_SECS, num_pipeline_buffers, MAX_BUFFER_SIZE),
            stage,
            left_index,
            right_index,
            result_l: Vec::new(),
            result_r: Vec::new(),
            num_waveform_buffers,
        }
    }

    pub fn num_waveform_buffers(&self) -> usize {
        self.num_waveform_buffers
    }

    pub fn audio_buffer_indices(&self) -> (usize, usize) {
        (self.left_index, self.right_index)
    }

    pub fn process(&mut self, num_samples: usize, render_passes: Vec<(f64, f64)>) -> StageActivity {
        let buffers = &mut self.magnetron.prepare(num_samples);
        let activity = buffers.process(&*render_passes, [&mut self.stage]);
        self.result_l.clear();
        self.result_r.clear();
        self.result_l
            .extend(buffers.read(BufferIndex::Internal(self.left_index)));
        self.result_r
            .extend(buffers.read(BufferIndex::Internal(self.right_index)));
        activity
    }

    pub fn check_audio_out_content(&self, num_samples: usize, mut f: impl FnMut(f64) -> f64) {
        check_sampled_signal(&self.result_l, num_samples, &mut f);
        check_sampled_signal(&self.result_r, num_samples, &mut f);
    }
}

pub struct TestContext;

impl QueryInfo for TestContext {
    type Context<'a> = &'a [(f64, f64)];
}

pub fn check_sampled_signal(buffer: &[f64], num_samples: usize, mut f: impl FnMut(f64) -> f64) {
    assert_eq!(buffer.len(), num_samples);

    let mut time = 0.0;
    for sample in buffer {
        assert_approx_eq!(sample, f(time));
        time += SAMPLE_WIDTH_SECS;
    }
}
