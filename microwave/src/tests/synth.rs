use std::collections::HashMap;
use std::f64::consts::TAU;

use assert_approx_eq::assert_approx_eq;
use magnetron::Magnetron;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use tune::pitch::Pitch;

use crate::backend::Backend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::control::LiveParameterStorage;
use crate::magnetron::StageType;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::source::LfSource;
use crate::magnetron::source::LfSourceExpr;
use crate::magnetron::waveform::WaveformProperty;
use crate::magnetron::waveform::WaveformSpec;
use crate::profile::PipelineParam;
use crate::profile::WaveformParam;
use crate::synth;
use crate::synth::MagnetronEvent;
use crate::synth::MagnetronSpec;

const SAMPLE_RATE: u32 = 44100;
const NUM_SAMPLES: usize = 441;
const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;
const NUM_BUFFERS: usize = 8;
const WAVEFORM_OUT_BUFFER: usize = 5;
const OUTPUT_BUFFERS: (usize, usize) = (6, 7);

struct SynthFixture {
    backend: Box<dyn Backend<u16>>,
    stage: Stage<PipelineParam>,
    magnetron: Magnetron,
    rendered_samples: usize,
}

impl SynthFixture {
    fn new() -> Self {
        let stages: Vec<StageType<WaveformParam>> = serde_yaml::from_str(
            r"
- stage_type: Generator
  out_buffer: 5
  generator_type: Oscillator
  oscillator_type: Sin
  frequency: WaveformPitch",
        )
        .expect("waveform spec should be valid");
        let buffer_secs = NUM_SAMPLES as f64 * SAMPLE_WIDTH_SECS;

        let one_shot_envelope = EnvelopeSpec {
            in_buffer: WAVEFORM_OUT_BUFFER,
            out_buffers: OUTPUT_BUFFERS,
            out_levels: None,
            fadeout: LfSource::Value(1.0),
            attack_time: LfSource::Value(0.0),
            decay_rate: LfSource::Value(0.0),
            release_time: LfSource::Value(2.0 * buffer_secs),
        };
        let sustained_envelope = EnvelopeSpec {
            in_buffer: WAVEFORM_OUT_BUFFER,
            out_buffers: OUTPUT_BUFFERS,
            out_levels: None,
            fadeout: LfSourceExpr::Property(WaveformProperty::OffVelocitySet).wrap(),
            attack_time: LfSource::Value(0.0),
            decay_rate: LfSource::Value(0.0),
            release_time: LfSource::Value(buffer_secs),
        };
        let envelopes = HashMap::from([
            ("one-shot".to_owned(), one_shot_envelope),
            ("sustained".to_owned(), sustained_envelope),
        ]);

        let spec = MagnetronSpec {
            note_input: NoteInput::Foreground,
            num_buffers: NUM_BUFFERS,
            waveforms: vec![
                WaveformSpec {
                    name: "one-shot".to_owned(),
                    envelope: "one-shot".to_owned(),
                    stages: stages.clone(),
                },
                WaveformSpec {
                    name: "sustained".to_owned(),
                    envelope: "sustained".to_owned(),
                    stages,
                },
            ],
            default_waveform: Some(1),
        };

        let templates = HashMap::from([(
            "WaveformPitch".to_owned(),
            LfSourceExpr::Property(WaveformProperty::WaveformPitch).wrap(),
        )]);
        let (events, _) = flume::unbounded::<MagnetronEvent>();

        let mut stages = Vec::new();
        let mut backends: Vec<Box<dyn Backend<u16>>> = Vec::new();
        spec.create(
            u32::try_from(NUM_SAMPLES).unwrap(),
            SAMPLE_RATE,
            &templates,
            &envelopes,
            &mut stages,
            &mut backends,
            &events,
        );

        Self {
            backend: backends.pop().expect("backend should be created"),
            stage: stages.pop().expect("stage should be created"),
            magnetron: Magnetron::new(SAMPLE_WIDTH_SECS, NUM_BUFFERS, NUM_SAMPLES),
            rendered_samples: 0,
        }
    }

    fn press(&mut self, key: u16, pitch_hz: f64) {
        self.backend.start(key, 0, Pitch::from_hz(pitch_hz), 127);
    }

    fn release(&mut self, key: u16) {
        self.backend.stop(key, 127);
    }

    fn select_waveform(&mut self, waveform: u8) {
        self.backend
            .program_change(ProgramChange::ProgramId(waveform));
    }

    fn time(&self) -> usize {
        self.rendered_samples
    }

    fn render(&mut self) -> (usize, Vec<f64>) {
        let Self {
            stage,
            magnetron,
            rendered_samples,
            ..
        } = self;
        let start = *rendered_samples;

        let mut buffers = magnetron.prepare(NUM_SAMPLES);
        let storage = LiveParameterStorage::default();
        let globals = HashMap::new();
        buffers.process((&(), &storage, &globals), [stage]);

        let samples = buffers
            .read(BufferIndex::Internal(OUTPUT_BUFFERS.0))
            .to_vec();
        *rendered_samples += NUM_SAMPLES;

        (start, samples)
    }
}

#[derive(Clone, Copy)]
enum Envelope {
    Sustained,
    OneShot,
}

#[derive(Clone, Copy)]
struct Voice {
    pitch_hz: f64,
    start: usize,
    release: Option<usize>,
    envelope: Envelope,
}

impl Voice {
    fn sustained(pitch_hz: f64, start: usize) -> Self {
        Self {
            pitch_hz,
            start,
            release: None,
            envelope: Envelope::Sustained,
        }
    }

    fn one_shot(pitch_hz: f64, start: usize) -> Self {
        Self {
            pitch_hz,
            start,
            release: None,
            envelope: Envelope::OneShot,
        }
    }

    fn released_at(mut self, release: usize) -> Self {
        self.release = Some(release);
        self
    }

    fn sample_at(&self, sample: usize) -> f64 {
        if sample < self.start {
            return 0.0;
        }

        let age = sample - self.start;
        let amplitude = match self.envelope {
            Envelope::Sustained => {
                if age < NUM_SAMPLES {
                    age as f64 / NUM_SAMPLES as f64
                } else {
                    1.0
                }
            }
            Envelope::OneShot => {
                if age < NUM_SAMPLES {
                    0.5 * age as f64 / NUM_SAMPLES as f64
                } else if age < 2 * NUM_SAMPLES {
                    0.5 * (1.0 - (age - NUM_SAMPLES) as f64 / NUM_SAMPLES as f64)
                } else {
                    0.0
                }
            }
        };
        let signal = amplitude * (TAU * self.pitch_hz * age as f64 * SAMPLE_WIDTH_SECS).sin();

        match self.release {
            Some(release) if sample >= release => {
                let release_age = sample - release;
                if release_age < NUM_SAMPLES {
                    signal * (1.0 - release_age as f64 / NUM_SAMPLES as f64)
                } else {
                    0.0
                }
            }
            _ => signal,
        }
    }
}

fn assert_rendered(samples: &[f64], start: usize, voices: &[Voice]) {
    assert_eq!(samples.len(), NUM_SAMPLES);

    for (index, sample) in samples.iter().enumerate() {
        let expected = voices
            .iter()
            .map(|voice| voice.sample_at(start + index))
            .sum::<f64>();
        assert_approx_eq!(*sample, expected);
    }
}

#[test]
fn pressed_note_renders_expected_waveform() {
    let mut fixture = SynthFixture::new();
    let press_sample = fixture.time();
    fixture.press(0, 440.0);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[Voice::sustained(440.0, press_sample)]);
}

#[test]
fn released_note_fades_out_and_frees_its_slot() {
    let mut fixture = SynthFixture::new();
    let press_sample = fixture.time();
    fixture.press(0, 440.0);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[Voice::sustained(440.0, press_sample)]);

    let release_sample = fixture.time();
    fixture.release(0);

    let (start, samples) = fixture.render();
    let fading = Voice::sustained(440.0, press_sample).released_at(release_sample);
    assert_rendered(&samples, start, &[fading]);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[]);

    let press_sample = fixture.time();
    fixture.press(1, 550.0);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[Voice::sustained(550.0, press_sample)]);
}

#[test]
fn full_polyphony_drops_new_notes_until_a_released_slot_recovers() {
    let mut fixture = SynthFixture::new();

    let mut voices = Vec::new();
    for key in 0..synth::MAX_POLYPHONY as u16 {
        let pitch_hz = 100.0 + f64::from(key);
        fixture.press(key, pitch_hz);
        voices.push(Voice::sustained(pitch_hz, 0));
    }

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &voices);

    let extra_key = synth::MAX_POLYPHONY as u16;
    let extra_pitch_hz = 400.0;

    fixture.press(extra_key, extra_pitch_hz);
    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &voices);

    let release_sample = fixture.time();
    fixture.release(0);
    fixture.press(extra_key, extra_pitch_hz);

    let (start, samples) = fixture.render();
    voices[0] = voices[0].released_at(release_sample);
    assert_rendered(&samples, start, &voices);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &voices[1..]);

    let press_sample = fixture.time();
    fixture.press(extra_key, extra_pitch_hz);

    let (start, samples) = fixture.render();
    let mut expected = voices[1..].to_vec();
    expected.push(Voice::sustained(extra_pitch_hz, press_sample));
    assert_rendered(&samples, start, &expected);
}

#[test]
fn releasing_a_finished_key_does_not_stop_a_later_note() {
    let mut fixture = SynthFixture::new();
    fixture.select_waveform(0);

    let one_shot_start = fixture.time();
    fixture.press(0, 220.0);
    let one_shot = Voice::one_shot(220.0, one_shot_start);

    for _ in 0..3 {
        let (start, samples) = fixture.render();
        assert_rendered(&samples, start, &[one_shot]);
    }

    fixture.select_waveform(1);
    let sustained_start = fixture.time();
    fixture.press(1, 440.0);
    let sustained = Voice::sustained(440.0, sustained_start);

    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[sustained]);

    fixture.release(0);
    let (start, samples) = fixture.render();
    assert_rendered(&samples, start, &[sustained]);

    let extra_start = fixture.time();
    fixture.press(2, 660.0);
    let (start, samples) = fixture.render();
    let expected = [sustained, Voice::sustained(660.0, extra_start)];
    assert_rendered(&samples, start, &expected);
}
