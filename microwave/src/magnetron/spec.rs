use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    envelope::Envelope,
    filter::{Filter, RingModulator},
    oscillator::Oscillator,
    signal::SignalSpec,
    waveform::{Creator, Spec, Stage},
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

#[derive(Deserialize, Serialize)]
pub enum StageSpec<C> {
    Oscillator(Oscillator<C>),
    Signal(SignalSpec<C>),
    Waveguide(WaveguideSpec<C>),
    Filter(Filter<C>),
    RingModulator(RingModulator<C>),
}

impl<C: Controller> Spec for &StageSpec<C> {
    type Created = Stage<C>;

    fn use_creator(self, creator: &Creator) -> Self::Created {
        match self {
            StageSpec::Oscillator(oscillation) => creator.create(oscillation),
            StageSpec::Signal(spec) => creator.create(spec),
            StageSpec::Waveguide(spec) => creator.create(spec),
            StageSpec::Filter(filter) => creator.create(filter),
            StageSpec::RingModulator(ring_modulator) => creator.create(ring_modulator),
        }
    }
}
