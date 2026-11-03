use std::collections::HashMap;
use std::f64::consts::TAU;

use magnetron::Magnetron;
use magnetron::automation::AutomationFactory;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use tune::pitch::Pitch;

use crate::backend::Backend;
use crate::backend::NoteInput;
use crate::control::LiveParameterStorage;
use crate::magnetron::BufferName;
use crate::magnetron::BufferNameMap;
use crate::magnetron::Factory;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::waveform::WaveformSpec;
use crate::pipeline::PipelineEvent;
use crate::profile::PipelineParam;
use crate::synth::MagnetronSpec;
use crate::tests::magnetron::fixture;
use crate::tests::magnetron::fixture::AllocationFixture;
use crate::tests::magnetron::fixture::MagnetronFixture;

#[test]
fn buffer_names_are_local_to_waveforms() {
    let first = r"
- stage_type: Generator
  out_buffer: foo
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 440.0
- stage_type: Processor
  in_buffer: foo
  out_buffer: bar
  processor_type: Pass";
    let second = r"
- stage_type: Generator
  out_buffer: baz
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 440.0
- stage_type: Processor
  in_buffer: baz
  out_buffer: qux
  processor_type: Pass";

    let mut fixture = AllocationFixture::new(&[first, second]);
    let (_, num_buffers) = fixture.allocate();

    assert_eq!(num_buffers, 2);
}

#[test]
fn buffer_names_are_reused_within_waveform() {
    let waveform = r"
- stage_type: Generator
  out_buffer: oscillator
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
- stage_type: Processor
  in_buffer: oscillator
  out_buffer: output
  processor_type: Pass";

    let mut test = MagnetronFixture::new(&[waveform]);

    assert_eq!(test.num_waveform_buffers(), 2);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());
}

#[test]
fn waveforms_with_different_buffer_counts_use_the_maximum() {
    let short = fixture::SINE_WAVEFORM;
    let long = r"
- stage_type: Generator
  out_buffer: oscillator
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
- stage_type: Processor
  in_buffer: oscillator
  out_buffer: modulator
  processor_type: Pass
- stage_type: Processor
  in_buffer: modulator
  out_buffer: output
  processor_type: Pass";

    let mut test = MagnetronFixture::new(&[short, long]);

    assert_eq!(test.num_waveform_buffers(), 3);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 0.7), (660.0, 0.8)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        t * (0.7 * (TAU * 440.0 * t).sin() + 0.8 * (TAU * 660.0 * t).sin())
    });
}

#[test]
fn envelope_output_uses_main_pipeline_buffer_indices() {
    let voice_l: BufferName = "voice_l".into();
    let voice_r: BufferName = "voice_r".into();

    let mut pipeline_buffers = BufferNameMap::default();
    pipeline_buffers.index(&voice_r);
    pipeline_buffers.index(&voice_l);

    let envelope = EnvelopeSpec {
        out_buffers: (voice_l, voice_r),
        out_levels: None,
        ..fixture::test_envelope()
    };

    let mut test = MagnetronFixture::new_with_pipeline_buffers(
        &[fixture::SINE_WAVEFORM],
        envelope,
        pipeline_buffers,
    );

    assert_eq!(test.audio_buffer_indices(), (1, 0));

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());
}

#[test]
fn envelope_external_buffers_are_registered_in_main_pipeline() {
    let envelope = fixture::test_envelope();
    let mut fixture = AllocationFixture::new_with_envelopes(
        &[fixture::SINE_WAVEFORM],
        HashMap::from([("test envelope".into(), envelope)]),
    );

    let (factory, _) = fixture.allocate();

    let left = fixture.pipeline_buffers.index_of("left").unwrap();
    let right = fixture.pipeline_buffers.index_of("right").unwrap();

    assert_eq!(factory.external_buffers().index_of("left"), Some(left));
    assert_eq!(factory.external_buffers().index_of("right"), Some(right));
}

#[test]
fn repeated_waveform_creation_resets_internal_names() {
    let mut fixture = AllocationFixture::new(&[fixture::SINE_WAVEFORM]);
    let (mut factory, _) = fixture.allocate();

    fixture.create_waveform(&mut factory, 0);
    let num_buffers = factory.num_internal_buffers();
    let output_index = factory.internal_buffers().index_of("output");

    fixture.create_waveform(&mut factory, 0);

    assert_eq!(factory.num_internal_buffers(), num_buffers);
    assert_eq!(factory.internal_buffers().index_of("output"), output_index);
}

#[test]
fn production_spec_renders_with_named_buffers() {
    let spec = MagnetronSpec {
        note_input: NoteInput::Foreground,
        waveforms: vec![WaveformSpec {
            name: "test waveform".into(),
            envelope: "test envelope".into(),
            stages: serde_yaml::from_str(fixture::SINE_WAVEFORM).unwrap(),
        }],
        default_waveform: Some(0),
    };
    let templates = fixture::test_templates();
    let envelopes = HashMap::from([("test envelope".into(), fixture::test_envelope())]);

    let mut factory = Factory::new(AutomationFactory::new(HashMap::new()));
    factory.internal_buffer_index(&"audio_in_l".into());

    let mut stages: Vec<Stage<PipelineParam>> = Vec::new();
    let mut backends: Vec<Box<dyn Backend<u32>>> = Vec::new();
    let (events, _events) = flume::unbounded::<PipelineEvent>();

    spec.create(
        44100,
        44100,
        &templates,
        &envelopes,
        &mut stages,
        &mut backends,
        &events,
        &mut factory,
    );

    let left = factory.internal_buffers().index_of("left").unwrap();
    let right = factory.internal_buffers().index_of("right").unwrap();
    assert_eq!(left, 1);
    assert_eq!(right, 2);

    let mut magnetron = Magnetron::new(
        fixture::SAMPLE_WIDTH_SECS,
        factory.num_internal_buffers(),
        fixture::MAX_BUFFER_SIZE,
    );

    backends[0].start(0, 0, Pitch::from_hz(440.0), 127);

    let buffers = &mut magnetron.prepare(fixture::NUM_SAMPLES);
    buffers.process(
        (&(), &LiveParameterStorage::default(), &HashMap::new()),
        stages.iter_mut(),
    );

    let expected = |t: f64| t * (TAU * 440.0 * t).sin();
    fixture::check_sampled_signal(
        buffers.read(BufferIndex::Internal(left)),
        fixture::NUM_SAMPLES,
        expected,
    );
    fixture::check_sampled_signal(
        buffers.read(BufferIndex::Internal(right)),
        fixture::NUM_SAMPLES,
        expected,
    );
}
