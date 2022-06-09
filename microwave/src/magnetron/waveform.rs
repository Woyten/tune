use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use super::{
    control::Controller,
    envelope::Envelope,
    source::LfSource,
    spec::{EnvelopeSpec, WaveformSpec},
    AutomatedValue, AutomationContext, Magnetron,
};

pub struct Waveform<C: Controller> {
    pub envelope: Envelope,
    pub stages: Vec<Stage<C>>,
    pub properties: WaveformProperties,
}

pub struct WaveformProperties {
    pub pitch: Pitch,
    pub velocity: f64,
    pub pressure: f64,
    pub secs_since_pressed: f64,
    pub secs_since_released: f64,
}

pub struct Stage<C: Controller> {
    stage_fn: Box<dyn FnMut(&mut Magnetron, &AutomationContext<C::Storage>) + Send>,
}

impl<C: Controller> Stage<C> {
    pub fn render(&mut self, buffers: &mut Magnetron, context: &AutomationContext<C::Storage>) {
        (self.stage_fn)(buffers, context);
    }
}

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
    ) -> Option<Waveform<C>> {
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

    pub fn create_stage<C: Controller, S: Spec>(
        &self,
        input: S,
        mut stage_fn: impl FnMut(&mut Magnetron, <S::Created as AutomatedValue>::Value) + Send + 'static,
    ) -> Stage<C>
    where
        S::Created: AutomatedValue<Storage = C::Storage> + Send + 'static,
    {
        let mut input = self.create(input);
        Stage {
            stage_fn: Box::new(move |magnetron, context| {
                stage_fn(magnetron, context.read(&mut input))
            }),
        }
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
