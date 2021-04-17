use serde::{Deserialize, Serialize};
use tune::pitch::{Pitch, Ratio};

use super::{
    control::Controller,
    envelope::Envelope,
    filter::{Filter, RingModulator},
    oscillator::Oscillator,
    signal::SignalSpec,
    waveform::{Stage, Waveform, WaveformProperties},
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
pub struct WaveformSpec<C> {
    pub name: String,
    pub envelope: String,
    pub stages: Vec<StageSpec<C>>,
}

impl<C: Controller> WaveformSpec<C> {
    pub fn create_waveform(
        &self,
        pitch: Pitch,
        velocity: f64,
        envelope: Envelope,
    ) -> Waveform<C::Storage> {
        Waveform {
            envelope,
            stages: self.stages.iter().map(StageSpec::create_stage).collect(),
            properties: WaveformProperties {
                pitch,
                pitch_bend: Ratio::default(),
                velocity,
                pressure: 0.0,
                secs_since_pressed: 0.0,
                secs_since_released: 0.0,
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Deserialize, Serialize)]
pub enum StageSpec<C> {
    Oscillator(Oscillator<C>),
    Signal(SignalSpec<C>),
    Waveguide(WaveguideSpec<C>),
    Filter(Filter<C>),
    RingModulator(RingModulator<C>),
}

impl<C: Controller> StageSpec<C> {
    fn create_stage(&self) -> Stage<C::Storage> {
        match self {
            StageSpec::Oscillator(oscillation) => oscillation.create_stage(),
            StageSpec::Signal(spec) => spec.create_stage(),
            StageSpec::Waveguide(spec) => spec.create_stage(),
            StageSpec::Filter(filter) => filter.create_stage(),
            StageSpec::RingModulator(ring_modulator) => ring_modulator.create_stage(),
        }
    }
}
