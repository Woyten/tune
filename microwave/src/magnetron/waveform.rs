use std::collections::HashMap;

use magnetron::{
    automation::{AutomatableParam, AutomationFactory},
    envelope::EnvelopeSpec,
    stage::Stage,
};
use serde::{Deserialize, Serialize};

use super::{source::StorageAccess, StageType};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NamedEnvelopeSpec<A> {
    pub name: String,
    #[serde(flatten)]
    pub spec: EnvelopeSpec<A>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WaveformSpec<A> {
    pub name: String,
    pub envelope: String,
    pub stages: Vec<StageType<A>>,
}

impl<A: AutomatableParam> WaveformSpec<A> {
    pub fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        envelopes: &HashMap<String, EnvelopeSpec<A>>,
    ) -> Vec<Stage<A>> {
        let envelope = envelopes.get(&self.envelope);
        if envelope.is_none() {
            log::warn!("Unknown envelope {}", self.envelope);
        }

        let mut stages = Vec::new();
        stages.extend(self.stages.iter().map(|spec| spec.create(factory)));
        stages.extend(envelope.iter().map(|spec| spec.create(factory)));

        stages
    }
}

#[derive(Copy, Clone)]
pub struct WaveformProperties {
    pub pitch_hz: f64,
    pub velocity: f64,
    pub key_pressure: Option<f64>,
    pub off_velocity: Option<f64>,
}

impl WaveformProperties {
    pub fn initial(pitch_hz: f64, velocity: f64) -> Self {
        Self {
            pitch_hz,
            velocity,
            key_pressure: None,
            off_velocity: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
