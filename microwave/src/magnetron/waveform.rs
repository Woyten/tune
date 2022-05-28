use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use super::{
    control::Controller,
    envelope::Envelope,
    source::{Automation, LfSource},
    spec::{EnvelopeSpec, WaveformSpec},
    Magnetron, WaveformControl,
};

pub struct Waveform<S> {
    pub envelope: Envelope,
    pub stages: Vec<Stage<S>>,
    pub properties: WaveformProperties,
}

pub struct WaveformProperties {
    pub pitch: Pitch,
    pub velocity: f64,
    pub pressure: f64,
    pub secs_since_pressed: f64,
    pub secs_since_released: f64,
}

pub type Stage<S> = Box<dyn FnMut(&mut Magnetron, &WaveformControl<S>) + Send>;

// TODO: Move to spec
pub struct Creator {
    envelope_map: HashMap<String, EnvelopeSpec>,
}

impl Creator {
    pub fn new(envelope_map: HashMap<String, EnvelopeSpec>) -> Self {
        Self { envelope_map }
    }

    pub fn create<S: Spec>(&self, spec: S) -> S::Created {
        spec.use_creator(self)
    }

    pub fn create_waveform<C: Controller>(
        &self,
        spec: &WaveformSpec<C>,
        pitch: Pitch,
        velocity: f64,
        envelope_name: &str,
    ) -> Option<Waveform<C::Storage>> {
        let envelope = self.envelope_map.get(envelope_name)?.create_envelope();

        Some(Waveform {
            envelope,
            stages: spec.stages.iter().map(|spec| self.create(spec)).collect(),
            properties: WaveformProperties {
                pitch,
                velocity,
                pressure: 0.0,
                secs_since_pressed: 0.0,
                secs_since_released: 0.0,
            },
        })
    }

    pub fn create_envelope(&self, envelop_name: &str) -> Option<Envelope> {
        self.envelope_map
            .get(envelop_name)
            .map(EnvelopeSpec::create_envelope)
    }
}

pub trait Spec {
    type Created;

    fn use_creator(self, creator: &Creator) -> Self::Created;
}

impl<S1: Spec, S2: Spec> Spec for (S1, S2) {
    type Created = (S1::Created, S2::Created);

    fn use_creator(self, creator: &Creator) -> Self::Created {
        (creator.create(self.0), creator.create(self.1))
    }
}

impl<S1: Spec, S2: Spec, S3: Spec> Spec for (S1, S2, S3) {
    type Created = (S1::Created, S2::Created, S3::Created);

    fn use_creator(self, creator: &Creator) -> Self::Created {
        (
            creator.create(self.0),
            creator.create(self.1),
            creator.create(self.2),
        )
    }
}

pub struct Input {
    pub buffer: InBuffer,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InBuffer {
    Buffer(usize),
    AudioIn(AudioIn),
}

impl InBuffer {
    pub fn audio_in() -> Self {
        Self::AudioIn(AudioIn::AudioIn)
    }
}

impl Spec for &InBuffer {
    type Created = Input;

    fn use_creator(self, _creator: &Creator) -> Self::Created {
        Input {
            buffer: self.clone(),
        }
    }
}

// Single variant enum for nice serialization
#[derive(Clone, Deserialize, Serialize)]
pub enum AudioIn {
    AudioIn,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OutSpec<C> {
    pub out_buffer: OutBuffer,
    pub out_level: LfSource<C>,
}

impl<C: Controller> Spec for &OutSpec<C> {
    type Created = Output<C::Storage>;

    fn use_creator(self, creator: &Creator) -> Self::Created {
        Output {
            buffer: self.out_buffer.clone(),
            level: creator.create(&self.out_level),
        }
    }
}

pub struct Output<S> {
    // Making those fields private results in a performance loss
    pub buffer: OutBuffer,
    pub level: Automation<S>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutBuffer {
    Buffer(usize),
    AudioOut(AudioOut),
}

impl OutBuffer {
    pub fn audio_out() -> Self {
        Self::AudioOut(AudioOut::AudioOut)
    }
}

// Single variant enum for nice serialization
#[derive(Clone, Deserialize, Serialize)]
pub enum AudioOut {
    AudioOut,
}
