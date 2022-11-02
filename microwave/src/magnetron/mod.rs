use magnetron::{
    automation::Automation,
    buffer::{InBuffer, OutBuffer},
    spec::{Creator, Spec},
    waveform::{Envelope, Stage, Waveform, WaveformState},
};
use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use self::{
    effects::EffectSpec,
    filter::{Filter, RingModulator},
    oscillator::OscillatorSpec,
    signal::SignalSpec,
    waveguide::WaveguideSpec,
};

mod util;

pub mod effects;
pub mod filter;
pub mod oscillator;
pub mod signal;
pub mod source;
pub mod waveguide;

pub trait AutomationSpec: Spec<Created = Automation<Self::Context>> {
    type Context: 'static;
}

#[derive(Deserialize, Serialize)]
pub struct WaveformsSpec<A> {
    pub envelopes: Vec<EnvelopeSpec>,
    pub waveforms: Vec<WaveformSpec<A>>,
    pub effects: Vec<EffectSpec>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct EnvelopeSpec {
    pub name: String,
    pub attack_time: f64,
    pub release_time: f64,
    pub decay_rate: f64,
}

impl EnvelopeSpec {
    pub fn create_envelope(&self) -> Envelope {
        Envelope {
            attack_time: self.attack_time,
            release_time: self.release_time,
            decay_rate: self.decay_rate,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct WaveformSpec<A> {
    pub name: String,
    pub envelope: String,
    pub stages: Vec<StageSpec<A>>,
}

impl<A> WaveformSpec<A> {
    pub fn with_pitch_and_velocity(&self, pitch: Pitch, velocity: f64) -> CreateWaveformSpec<A> {
        CreateWaveformSpec {
            envelope: &self.envelope,
            stages: &self.stages,
            pitch,
            velocity,
        }
    }
}

pub struct CreateWaveformSpec<'a, A> {
    pub envelope: &'a str,
    pub stages: &'a [StageSpec<A>],
    pub pitch: Pitch,
    pub velocity: f64,
}

impl<'a, A: AutomationSpec> Spec for CreateWaveformSpec<'a, A> {
    type Created = Option<Waveform<A::Context>>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        Some(Waveform {
            envelope: creator.create_envelope(self.envelope)?,
            stages: self
                .stages
                .iter()
                .map(|spec| creator.create(spec))
                .collect(),
            state: WaveformState {
                pitch_hz: self.pitch.as_hz(),
                velocity: self.velocity,
                key_pressure: 0.0,
                secs_since_pressed: 0.0,
                secs_since_released: 0.0,
            },
        })
    }
}

pub struct WaveformStateAndStorage<S> {
    pub state: WaveformState,
    pub storage: S,
}

#[derive(Deserialize, Serialize)]
pub enum StageSpec<A> {
    Oscillator(OscillatorSpec<A>),
    Signal(SignalSpec<A>),
    Waveguide(WaveguideSpec<A>),
    Filter(Filter<A>),
    RingModulator(RingModulator<A>),
}

impl<A: AutomationSpec> Spec for StageSpec<A> {
    type Created = Stage<A::Context>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
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
    use magnetron::{
        spec::Creator,
        waveform::{Envelope, Waveform},
        Magnetron,
    };
    use tune::pitch::Pitch;

    use super::{
        source::{LfSource, NoControl},
        *,
    };

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
        let mut buffers = magnetron();
        let (mut waveform, payload) =
            create_waveform(&parse_stages_spec("[]"), Pitch::from_hz(440.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        assert_eq!(buffers.mix(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let mut buffers = magnetron();
        let (mut waveform, payload) = create_waveform(
            &parse_stages_spec(
                r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
            ),
            Pitch::from_hz(440.0),
            1.0,
        );

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        assert_buffer_mix_is(&buffers, |t| (TAU * 440.0 * t).sin());

        buffers.clear(128);
        assert_eq!(buffers.mix(), &[0f64; 128]);
    }

    #[test]
    fn mix_two_waveforms() {
        let mut buffers = magnetron();

        let spec = parse_stages_spec(
            r"
- Oscillator:
    kind: Sin
    frequency: WaveformPitch
    modulation: None
    out_buffer: AudioOut
    out_level: 1.0",
        );

        let (mut waveform1, payload1) = create_waveform(&spec, Pitch::from_hz(440.0), 0.7);
        let (mut waveform2, payload2) = create_waveform(&spec, Pitch::from_hz(660.0), 0.8);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform1, &payload1, 1.0);
        assert_buffer_mix_is(&buffers, |t| 0.7 * (440.0 * TAU * t).sin());

        buffers.write(&mut waveform2, &payload2, 1.0);
        assert_buffer_mix_is(&buffers, |t| {
            0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin()
        });
    }

    #[test]
    fn apply_optional_phase() {
        let mut buffers = magnetron();

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

        let (mut waveform, payload) = create_waveform(&spec, Pitch::from_hz(440.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        // 441 Hz because the phase modulates from 0.0 (initial) to 1.0 within 1s (buffer size) leading to one additional oscillation
        assert_buffer_mix_is(&buffers, move |t| (441.0 * t * TAU).sin());
    }

    #[test]
    fn modulate_by_frequency() {
        let mut buffers = magnetron();

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

        let (mut waveform, payload) = create_waveform(&spec, Pitch::from_hz(550.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        assert_buffer_mix_is(&buffers, {
            let mut mod_phase = 0.0;
            move |t| {
                let signal = ((550.0 * t + mod_phase) * TAU).sin();
                mod_phase += (330.0 * TAU * t).sin() * 440.0 * SAMPLE_WIDTH_SECS;
                signal
            }
        });
    }

    #[test]
    fn modulate_by_phase() {
        let mut buffers = magnetron();

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

        let (mut waveform, payload) = create_waveform(&spec, Pitch::from_hz(550.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        assert_buffer_mix_is(&buffers, |t| {
            ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let mut buffers = magnetron();

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

        let (mut waveform, payload) = create_waveform(&spec, Pitch::from_hz(440.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.mix(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &payload, 1.0);
        assert_buffer_mix_is(&buffers, |t| {
            (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    fn magnetron() -> Magnetron {
        Magnetron::new(SAMPLE_WIDTH_SECS, 2, 100000)
    }

    fn parse_stages_spec(stages_spec: &str) -> WaveformSpec<LfSource<NoControl>> {
        WaveformSpec {
            name: String::new(),
            envelope: "Organ".to_owned(),
            stages: serde_yaml::from_str(stages_spec).unwrap(),
        }
    }

    fn create_waveform(
        spec: &WaveformSpec<LfSource<NoControl>>,
        pitch: Pitch,
        velocity: f64,
    ) -> (
        Waveform<WaveformStateAndStorage<()>>,
        WaveformStateAndStorage<()>,
    ) {
        let envelope_map = HashMap::from([(
            spec.envelope.to_owned(),
            Envelope {
                attack_time: -1e-10,
                release_time: 1e-10,
                decay_rate: 0.0,
            },
        )]);
        let waveform = Creator::new(envelope_map)
            .create(spec.with_pitch_and_velocity(pitch, velocity))
            .unwrap();
        let payload = WaveformStateAndStorage {
            state: waveform.state,
            storage: (),
        };
        (waveform, payload)
    }

    fn assert_buffer_mix_is(buffers: &Magnetron, mut f: impl FnMut(f64) -> f64) {
        let mut time = 0.0;
        for sample in buffers.mix() {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_WIDTH_SECS;
        }
    }
}
