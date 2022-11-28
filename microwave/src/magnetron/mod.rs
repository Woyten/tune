use magnetron::{
    automation::AutomationSpec,
    buffer::{InBuffer, OutBuffer},
    envelope::EnvelopeSpec,
    spec::{Creator, Spec},
    waveform::{Waveform, WaveformProperties},
    Stage, StageState,
};
use serde::{Deserialize, Serialize};

use crate::control::LiveParameter;

use self::{
    effects::EffectSpec,
    filter::{Filter, RingModulator},
    oscillator::OscillatorSpec,
    signal::SignalSpec,
    source::{LfSource, NoAccess, StorageAccess},
    waveguide::WaveguideSpec,
};

mod util;

pub mod effects;
pub mod filter;
pub mod oscillator;
pub mod signal;
pub mod source;
pub mod waveguide;

#[derive(Deserialize, Serialize)]
pub struct AudioSpec {
    pub templates: Vec<TemplateSpec<LfSource<WaveformProperty, LiveParameter>>>,
    pub envelopes: Vec<NamedEnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>>,
    pub waveforms: Vec<WaveformSpec<LfSource<WaveformProperty, LiveParameter>>>,
    pub effects: Vec<EffectSpec<LfSource<NoAccess, LiveParameter>>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TemplateSpec<A> {
    pub name: String,
    pub spec: A,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NamedEnvelopeSpec<A> {
    pub name: String,
    #[serde(flatten)]
    pub spec: EnvelopeSpec<A>,
}

#[derive(Deserialize, Serialize)]
pub struct WaveformSpec<A> {
    pub name: String,
    pub envelope: String,
    pub stages: Vec<StageSpec<A>>,
}

impl<T, A: AutomationSpec<Context = (WaveformProperties, T)>> Spec<A> for WaveformSpec<A> {
    type Created = Waveform<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        let envelope_name = &self.envelope;

        Self::Created {
            stages: self
                .stages
                .iter()
                .map(|spec| creator.create(spec))
                .collect(),
            envelope: creator.create_envelope(envelope_name).unwrap_or_else(|| {
                println!("[WARNING] Unknown envelope {envelope_name}");
                creator.create_stage((), |_, _| StageState::Exhausted)
            }),
            is_active: true,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub enum WaveformProperty {
    WaveformPitch,
    WaveformPeriod,
    Velocity,
    KeyPressureSet,
    KeyPressure,
    OffVelocitySet,
    OffVelocity,
}

impl StorageAccess for WaveformProperty {
    type Storage = WaveformProperties;

    fn access(&mut self, storage: &Self::Storage) -> f64 {
        match self {
            WaveformProperty::WaveformPitch => storage.pitch_hz,
            WaveformProperty::WaveformPeriod => storage.pitch_hz.recip(),
            WaveformProperty::Velocity => storage.velocity,
            WaveformProperty::KeyPressureSet => f64::from(u8::from(storage.key_pressure.is_some())),
            WaveformProperty::KeyPressure => storage.key_pressure.unwrap_or_default(),
            WaveformProperty::OffVelocitySet => f64::from(u8::from(storage.off_velocity.is_some())),
            WaveformProperty::OffVelocity => storage.off_velocity.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub enum StageSpec<A> {
    Oscillator(OscillatorSpec<A>),
    Signal(SignalSpec<A>),
    Waveguide(WaveguideSpec<A>),
    Filter(Filter<A>),
    RingModulator(RingModulator<A>),
}

impl<A: AutomationSpec> Spec<A> for StageSpec<A> {
    type Created = Stage<A::Context>;

    fn use_creator(&self, creator: &Creator<A>) -> Self::Created {
        match self {
            StageSpec::Oscillator(spec) => creator.create(spec),
            StageSpec::Signal(spec) => creator.create(spec),
            StageSpec::Waveguide(spec) => creator.create(spec),
            StageSpec::Filter(spec) => creator.create(spec),
            StageSpec::RingModulator(spec) => creator.create(spec),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum InBufferSpec {
    Buffer(usize),
    AudioIn(AudioIn),
}

// Single variant enum for nice serialization
#[derive(Deserialize, Serialize)]
pub enum AudioIn {
    AudioIn,
}

impl InBufferSpec {
    pub fn audio_in() -> Self {
        Self::AudioIn(AudioIn::AudioIn)
    }

    pub fn buffer(&self) -> InBuffer {
        match self {
            InBufferSpec::Buffer(buffer) => InBuffer::Buffer(*buffer),
            InBufferSpec::AudioIn(AudioIn::AudioIn) => InBuffer::AudioIn,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct OutSpec<A> {
    pub out_buffer: OutBufferSpec,
    pub out_level: A,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutBufferSpec {
    Buffer(usize),
    AudioOut(AudioOut),
}

// Single variant enum for nice serialization
#[derive(Deserialize, Serialize)]
pub enum AudioOut {
    AudioOut,
}

impl OutBufferSpec {
    pub fn audio_out() -> Self {
        Self::AudioOut(AudioOut::AudioOut)
    }

    pub fn buffer(&self) -> OutBuffer {
        match self {
            OutBufferSpec::Buffer(buffer) => OutBuffer::Buffer(*buffer),
            OutBufferSpec::AudioOut(AudioOut::AudioOut) => OutBuffer::AudioOut,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, f64::consts::TAU};

    use assert_approx_eq::assert_approx_eq;
    use magnetron::{spec::Creator, Magnetron};

    use crate::{assets::get_builtin_waveforms, control::LiveParameterStorage};

    use super::*;

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut buffers = magnetron();

        assert_eq!(buffers.mix(), &[0f64; 0]);

        buffers.clear(128);
        assert_eq!(buffers.mix(), &[0f64; 128]);

        buffers.clear(256);
        assert_eq!(buffers.mix(), &[0f64; 256]);

        buffers.clear(64);
        assert_eq!(buffers.mix(), &[0f64; 64]);
    }

    #[test]
    fn empty_spec() {
        let spec = parse_stages_spec("[]");
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_eq!(buffers.mix(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| t * (TAU * 440.0 * t).sin());

        buffers.clear(128);
        assert_eq!(buffers.mix(), &[0f64; 128]);
    }

    #[test]
    fn mix_two_waveforms() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform1 = creator().create(&spec);
        let mut waveform2 = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform1, &payload(440.0, 0.7));
        assert_buffer_mix_is(&buffers, |t| t * 0.7 * (440.0 * TAU * t).sin());

        buffers.write(&mut waveform2, &payload(660.0, 0.8));
        assert_buffer_mix_is(&buffers, |t| {
            t * (0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin())
        });
    }

    #[test]
    fn apply_optional_phase() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    phase: 1.0
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(440.0, 1.0));
        // 441 Hz because the phase modulates from 0.0 (initial) to 1.0 within 1s (buffer size) leading to one additional oscillation
        assert_buffer_mix_is(&buffers, move |t| t * (441.0 * t * TAU).sin());
    }

    #[test]
    fn modulate_by_frequency() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: 330.0
    modulation: None
    out_buffer: 0
    out_level: 440.0
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: ByFrequency
    mod_buffer: 0
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(550.0, 1.0));
        assert_buffer_mix_is(&buffers, {
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
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: 330.0
    modulation: None
    out_buffer: 0
    out_level: 0.44
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: ByPhase
    mod_buffer: 0
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(550.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| {
            t * ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: 0
    out_level: 1.0
- Oscillator:
    kind: Sin
    frequency:
      Mul: [1.5, WaveformPitch]
    modulation: None
    out_buffer: 1
    out_level: 1.0
- RingModulator:
    in_buffers: [0, 1]
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator().create(&spec);

        let mut buffers = magnetron();

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| {
            t * (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_attack_time() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator_with_envelope(EnvelopeSpec {
            amplitude: LfSource::Value(1.0),
            fadeout: LfSource::Value(0.0),
            attack_time: LfSource::template("Velocity"),
            decay_rate: LfSource::Value(1.0),
            release_time: LfSource::Value(1.0),
        })
        .create(&spec);

        let mut buffers = magnetron();

        // attack part 1
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 3.0));
        assert_buffer_mix_is(&buffers, |t| 1.0 / 3.0 * t * (TAU * 440.0 * t).sin());

        // attack part 2
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 3.0 / 2.0));
        assert_buffer_mix_is(&buffers, |t| {
            (1.0 / 3.0 + 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });

        // decay part
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| {
            (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_decay_time() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator_with_envelope(EnvelopeSpec {
            amplitude: LfSource::Value(1.0),
            fadeout: LfSource::Value(0.0),
            attack_time: LfSource::Value(1.0),
            decay_rate: LfSource::template("Velocity"),
            release_time: LfSource::Value(1.0),
        })
        .create(&spec);

        let mut buffers = magnetron();

        // attack part
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| t * (TAU * 440.0 * t).sin());

        // decay part 1
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| {
            (1.0 - 1.0 / 2.0 * t) * (TAU * 440.0 * t).sin()
        });

        // decay part 2
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 2.0));
        assert_buffer_mix_is(&buffers, |t| {
            (1.0 / 2.0 - 3.0 / 8.0 * t) * (TAU * 440.0 * t).sin()
        });
    }

    #[test]
    fn evaluate_envelope_varying_fadeout() {
        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );
        let mut waveform = creator_with_envelope(EnvelopeSpec {
            amplitude: LfSource::Value(1.0),
            fadeout: LfSource::template("Velocity"),
            attack_time: LfSource::Value(1.0),
            decay_rate: LfSource::Value(0.0),
            release_time: LfSource::Value(3.0),
        })
        .create(&spec);

        let mut buffers = magnetron();

        // attack part
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 0.0));
        assert_buffer_mix_is(&buffers, |t| t * (TAU * 440.0 * t).sin());
        assert!(waveform.is_active);

        // sustain part
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 0.0));
        assert_buffer_mix_is(&buffers, |t| (TAU * 440.0 * t).sin());
        assert!(waveform.is_active);

        // release part 1
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 1.0));
        assert_buffer_mix_is(&buffers, |t| {
            (1.0 - 1.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });
        assert!(waveform.is_active);

        // release part 1
        buffers.clear(NUM_SAMPLES);
        buffers.write(&mut waveform, &payload(440.0, 2.0));
        assert_buffer_mix_is(&buffers, |t| {
            (2.0 / 3.0 - 2.0 / 3.0 * t) * (TAU * 440.0 * t).sin()
        });
        assert!(!waveform.is_active);
    }

    fn parse_stages_spec(
        stages_spec: &str,
    ) -> WaveformSpec<LfSource<WaveformProperty, LiveParameter>> {
        WaveformSpec {
            name: String::new(),
            envelope: "test envelope".to_owned(),
            stages: serde_yaml::from_str(stages_spec).unwrap(),
        }
    }

    fn creator() -> Creator<LfSource<WaveformProperty, LiveParameter>> {
        creator_with_envelope(EnvelopeSpec {
            amplitude: LfSource::template("Velocity"),
            fadeout: LfSource::Value(0.0),
            attack_time: LfSource::Value(0.0),
            decay_rate: LfSource::Value(0.0),
            release_time: LfSource::Value(0.0),
        })
    }

    fn creator_with_envelope(
        spec: EnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>,
    ) -> Creator<LfSource<WaveformProperty, LiveParameter>> {
        Creator::new(
            get_builtin_waveforms()
                .templates
                .into_iter()
                .map(|spec| (spec.name, spec.spec))
                .collect(),
            HashMap::from([("test envelope".to_owned(), spec)]),
        )
    }

    fn magnetron() -> Magnetron {
        Magnetron::new(SAMPLE_WIDTH_SECS, 2, 100000)
    }

    fn payload(pitch_hz: f64, velocity: f64) -> (WaveformProperties, LiveParameterStorage) {
        (
            WaveformProperties::initial(pitch_hz, velocity),
            Default::default(),
        )
    }

    fn assert_buffer_mix_is(buffers: &Magnetron, mut f: impl FnMut(f64) -> f64) {
        let mut time = 0.0;
        for sample in buffers.mix() {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_WIDTH_SECS;
        }
    }
}
