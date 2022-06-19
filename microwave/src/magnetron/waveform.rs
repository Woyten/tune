use std::{collections::HashMap, marker::PhantomData};

use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

use super::{
    envelope::Envelope, source::Automation, spec::EnvelopeSpec, AutomatedValue, AutomationContext,
    InBuffer, Magnetron, OutBuffer,
};

pub struct Waveform<A: AutomationSpec> {
    pub envelope: Envelope,
    pub stages: Vec<Stage<A>>,
    pub state: WaveformState,
}

pub struct WaveformState {
    pub pitch: Pitch,
    pub velocity: f64,
    pub secs_since_pressed: f64,
    pub secs_since_released: f64,
}

pub struct Stage<A: AutomationSpec> {
    stage_fn: Box<dyn FnMut(&mut Magnetron, &AutomationContext<A::Storage>) + Send>,
}

impl<A: AutomationSpec> Stage<A> {
    pub fn render(&mut self, buffers: &mut Magnetron, context: &AutomationContext<A::Storage>) {
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

    pub fn create_envelope(&self, envelop_name: &str) -> Option<Envelope> {
        self.envelope_map
            .get(envelop_name)
            .map(EnvelopeSpec::create_envelope)
    }

    pub fn create_stage<A: AutomationSpec, S: Spec>(
        &self,
        input: S,
        mut stage_fn: impl FnMut(&mut Magnetron, <S::Created as AutomatedValue>::Value) + Send + 'static,
    ) -> Stage<A>
    where
        S::Created: AutomatedValue<Storage = A::Storage> + Send + 'static,
    {
        let mut input = self.create(input);
        Stage {
            stage_fn: Box::new(move |magnetron, context| {
                stage_fn(magnetron, context.read(&mut input))
            }),
        }
    }

    pub fn create_automation<S: Spec>(
        &self,
        input: S,
        mut automation_fn: impl FnMut(
                &AutomationContext<<S::Created as AutomatedValue>::Storage>,
                <S::Created as AutomatedValue>::Value,
            ) -> f64
            + Send
            + 'static,
    ) -> Automation<<S::Created as AutomatedValue>::Storage>
    where
        S::Created: AutomatedValue + Send + 'static,
    {
        let mut input = self.create(input);
        Automation {
            automation_fn: Box::new(move |context| {
                automation_fn(context, context.read(&mut input))
            }),
        }
    }
}

pub trait Spec {
    type Created;

    fn use_creator(&self, creator: &Creator) -> Self::Created;
}

impl<C> Spec for PhantomData<C> {
    type Created = PhantomData<C>;

    fn use_creator(&self, _creator: &Creator) -> Self::Created {
        PhantomData
    }
}

impl<S: Spec> Spec for &S {
    type Created = S::Created;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        S::use_creator(self, creator)
    }
}

impl<S1: Spec, S2: Spec> Spec for (S1, S2) {
    type Created = (S1::Created, S2::Created);

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        (creator.create(&self.0), creator.create(&self.1))
    }
}

impl<S1: Spec, S2: Spec, S3: Spec> Spec for (S1, S2, S3) {
    type Created = (S1::Created, S2::Created, S3::Created);

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        (
            creator.create(&self.0),
            creator.create(&self.1),
            creator.create(&self.2),
        )
    }
}

pub trait AutomationSpec: Spec<Created = Automation<Self::Storage>> {
    type Storage: 'static;
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
