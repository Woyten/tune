use std::collections::HashMap;

use magnetron::automation::AutomatableParam;
use magnetron::stage::Stage;
use serde::Deserialize;
use serde::Serialize;

use crate::magnetron::BufferName;
use crate::magnetron::Factory;
use crate::magnetron::StageType;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::source::StorageAccess;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NamedEnvelopeSpec<A> {
    pub name: BufferName,
    #[serde(flatten)]
    pub spec: EnvelopeSpec<A>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WaveformSpec<A> {
    pub name: BufferName,
    pub envelope: BufferName,
    pub stages: Vec<StageType<A>>,
}

impl<A: AutomatableParam> WaveformSpec<A> {
    pub fn create(
        &self,
        factory: &mut Factory<A>,
        envelopes: &HashMap<BufferName, EnvelopeSpec<A>>,
    ) -> Vec<Stage<A>> {
        self.create_with_envelope_name(factory, envelopes, &self.envelope)
    }

    pub fn create_with_envelope_name(
        &self,
        factory: &mut Factory<A>,
        envelopes: &HashMap<BufferName, EnvelopeSpec<A>>,
        envelope_name: &str,
    ) -> Vec<Stage<A>> {
        let envelope = envelopes.get(envelope_name);
        if envelope.is_none() {
            log::warn!("Unknown envelope {envelope_name}");
        }

        self.create_with_envelope(factory, envelope)
    }

    pub(crate) fn create_with_envelope(
        &self,
        factory: &mut Factory<A>,
        envelope: Option<&EnvelopeSpec<A>>,
    ) -> Vec<Stage<A>> {
        factory.reset_internal_buffers();

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
