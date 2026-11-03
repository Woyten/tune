use std::f64::consts::TAU;

use magnetron::stage::StageActivity;

use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::source::LfSource;
use crate::tests::magnetron::fixture;
use crate::tests::magnetron::fixture::MagnetronFixture;

#[test]
fn clear_and_resize_buffers() {
    let mut test = MagnetronFixture::new(&[]);

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

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |_| 0.0);
}

#[test]
fn write_waveform_and_clear() {
    let waveform = fixture::SINE_WAVEFORM;

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

    test.process(128, vec![]);
    test.check_audio_out_content(128, |_| 0.0);
}

#[test]
fn mix_two_waveforms() {
    let waveform = fixture::SINE_WAVEFORM;

    let mut test = MagnetronFixture::new(&[waveform, waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 0.7), (660.0, 0.8)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        t * (0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin())
    });
}

#[test]
fn apply_optional_phase() {
    let waveform = r"
- stage_type: Generator
  out_buffer: output
  generator_type: Oscillator
  oscillator_type: Sin
  phase: 1.0
  frequency: WaveformPitch";

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    // 441 Hz because the phase modulates from 0.0 (initial) to 1.0 within 1s (buffer size) leading to one additional oscillation
    test.check_audio_out_content(fixture::NUM_SAMPLES, move |t| t * (441.0 * t * TAU).sin());
}

#[test]
fn modulate_by_frequency() {
    let waveform = r"
- stage_type: Generator
  out_buffer: oscillator
  out_level: 440.0
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 330.0
- stage_type: Processor
  in_buffer: oscillator
  out_buffer: output
  processor_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
  modulation: ByFrequency";

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(550.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, {
        let mut mod_phase = 0.0;
        move |t| {
            let signal = ((550.0 * t + mod_phase) * TAU).sin();
            mod_phase += (330.0 * TAU * t).sin() * 440.0 * fixture::SAMPLE_WIDTH_SECS;
            t * signal
        }
    });
}

#[test]
fn modulate_by_phase() {
    let waveform = r"
- stage_type: Generator
  out_buffer: oscillator
  out_level: 0.44
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: 330.0
- stage_type: Processor
  in_buffer: oscillator
  out_buffer: output
  processor_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
  modulation: ByPhase";

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(550.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        t * ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
    });
}

#[test]
fn ring_modulation() {
    let waveform = r"
- stage_type: Generator
  out_buffer: oscillator
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch
- stage_type: Generator
  out_buffer: modulator
  generator_type: Oscillator
  oscillator_type: Sin
  frequency:
    Mul: [1.5, WaveformPitch]
- stage_type: MergeProcessor
  in_buffers: [oscillator, modulator]
  out_buffer: output
  processor_type: RingModulator";

    let mut test = MagnetronFixture::new(&[waveform]);

    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        t * (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
    });
}

#[test]
fn evaluate_envelope_varying_attack_time() {
    let waveform = fixture::SINE_WAVEFORM;

    let mut test = MagnetronFixture::new_with_envelope(
        &[waveform],
        EnvelopeSpec {
            out_levels: None,
            attack_time: LfSource::template("Velocity"),
            decay_rate: LfSource::Value(1.0),
            release_time: LfSource::Value(1.0),
            ..fixture::test_envelope()
        },
    );

    // attack part 1
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 3.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        1.0 / 3.0 * t * (TAU * 440.0 * t).sin()
    });

    // attack part 2
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 3.0 / 2.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (1.0 / 3.0 + 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
    });

    // decay part
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
    });
}

#[test]
fn evaluate_envelope_varying_decay_time() {
    let waveform = fixture::SINE_WAVEFORM;

    let mut test = MagnetronFixture::new_with_envelope(
        &[waveform],
        EnvelopeSpec {
            out_levels: None,
            attack_time: LfSource::Value(1.0),
            decay_rate: LfSource::template("Velocity"),
            release_time: LfSource::Value(1.0),
            ..fixture::test_envelope()
        },
    );

    // attack part
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

    // decay part 1
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
    });

    // decay part 2
    test.process(fixture::NUM_SAMPLES, vec![(440.0, 2.0)]);
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (1.0 / 2.0 - 3.0 / 8.0 * t) * (TAU * 440.0 * t).sin()
    });
}

#[test]
fn evaluate_envelope_varying_fadeout() {
    let waveform = fixture::SINE_WAVEFORM;

    let mut test = MagnetronFixture::new_with_envelope(
        &[waveform],
        EnvelopeSpec {
            out_levels: None,
            fadeout: LfSource::template("Velocity"),
            attack_time: LfSource::Value(1.0),
            decay_rate: LfSource::Value(0.0),
            release_time: LfSource::Value(3.0),
            ..fixture::test_envelope()
        },
    );

    // attack part
    assert_eq!(
        test.process(fixture::NUM_SAMPLES, vec![(440.0, 0.0)]),
        StageActivity::External
    );
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| t * (TAU * 440.0 * t).sin());

    // sustain part
    assert_eq!(
        test.process(fixture::NUM_SAMPLES, vec![(440.0, 0.0)]),
        StageActivity::External
    );
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| (TAU * 440.0 * t).sin());

    // release part 1
    assert_eq!(
        test.process(fixture::NUM_SAMPLES, vec![(440.0, 1.0)]),
        StageActivity::External
    );
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (1.0 - 1.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
    });

    // release part 2
    assert_eq!(
        test.process(fixture::NUM_SAMPLES, vec![(440.0, 2.0)]),
        StageActivity::Internal
    );
    test.check_audio_out_content(fixture::NUM_SAMPLES, |t| {
        (2.0 / 3.0 - 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
    });
}
