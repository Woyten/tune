use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use super::{
    envelope::Envelope,
    filter::{Filter, RingModulator},
    oscillator::Oscillator,
    signal::SignalSpec,
    waveform::{AutomationSpec, Creator, Spec, Stage, Waveform, WaveformState},
    waveguide::WaveguideSpec,
};

#[derive(Deserialize, Serialize)]
pub struct WaveformsSpec<C> {
    pub envelopes: Vec<EnvelopeSpec>,
    pub waveforms: Vec<WaveformSpec<C>>,
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
    type Created = Option<Waveform<A>>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        Some(Waveform {
            envelope: creator.create_envelope(self.envelope)?,
            stages: self
                .stages
                .iter()
                .map(|spec| creator.create(spec))
                .collect(),
            state: WaveformState {
                pitch: self.pitch,
                velocity: self.velocity,
                secs_since_pressed: 0.0,
                secs_since_released: 0.0,
            },
        })
    }
}

#[derive(Deserialize, Serialize)]
pub enum StageSpec<A> {
    Oscillator(Oscillator<A>),
    Signal(SignalSpec<A>),
    Waveguide(WaveguideSpec<A>),
    Filter(Filter<A>),
    RingModulator(RingModulator<A>),
}

impl<A: AutomationSpec> Spec for StageSpec<A> {
    type Created = Stage<A>;

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
