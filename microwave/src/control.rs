use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct LiveParameterMapper {
    ccn_mapping: HashMap<LiveParameter, u8>,
}

impl LiveParameterMapper {
    pub fn new() -> Self {
        Self {
            ccn_mapping: HashMap::new(),
        }
    }

    pub fn push_mapping(&mut self, parameter: LiveParameter, ccn: u8) {
        self.ccn_mapping.insert(parameter, ccn);
    }

    pub fn get_ccn(&self, parameter: LiveParameter) -> Option<u8> {
        self.ccn_mapping.get(&parameter).copied()
    }

    pub fn resolve_ccn(&self, controller: u8) -> Vec<LiveParameter> {
        self.ccn_mapping
            .iter()
            .filter_map(move |(&parameter, &ccn)| (ccn == controller).then_some(parameter))
            .collect()
    }
}

#[derive(Copy, Clone, Default)]
pub struct LiveParameterStorage {
    modulation: f64,
    breath: f64,
    foot: f64,
    expression: f64,
    damper: f64,
    sostenuto: f64,
    soft: f64,
    legato: f64,
    channel_pressure: f64,
}

impl LiveParameterStorage {
    pub fn set_parameter(&mut self, parameter: LiveParameter, value: impl ParameterValue) {
        *match parameter {
            LiveParameter::Modulation => &mut self.modulation,
            LiveParameter::Breath => &mut self.breath,
            LiveParameter::Foot => &mut self.foot,
            LiveParameter::Expression => &mut self.expression,
            LiveParameter::Damper => &mut self.damper,
            LiveParameter::Sostenuto => &mut self.sostenuto,
            LiveParameter::Soft => &mut self.soft,
            LiveParameter::Legato => &mut self.legato,
            LiveParameter::ChannelPressure => &mut self.channel_pressure,
            LiveParameter::KeyPressure => panic!("Unexpected parameter {:?}", parameter),
        } = value.as_f64().clamp(0.0, 1.0)
    }

    pub fn read_parameter(&self, parameter: LiveParameter) -> f64 {
        match parameter {
            LiveParameter::Modulation => self.modulation,
            LiveParameter::Breath => self.breath,
            LiveParameter::Foot => self.foot,
            LiveParameter::Expression => self.expression,
            LiveParameter::Damper => self.damper,
            LiveParameter::Sostenuto => self.sostenuto,
            LiveParameter::Soft => self.soft,
            LiveParameter::Legato => self.legato,
            LiveParameter::ChannelPressure => self.channel_pressure,
            LiveParameter::KeyPressure => panic!("Unexpected parameter {:?}", parameter),
        }
    }

    pub fn is_active(&self, parameter: LiveParameter) -> bool {
        self.read_parameter(parameter) >= 0.5
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum LiveParameter {
    Modulation,
    Breath,
    Foot,
    Expression,
    Damper,
    Sostenuto,
    Soft,
    Legato,
    ChannelPressure,
    KeyPressure,
}

pub trait ParameterValue: Copy {
    fn as_f64(self) -> f64;

    fn as_u8(self) -> u8;
}

impl ParameterValue for f64 {
    fn as_f64(self) -> f64 {
        self
    }

    fn as_u8(self) -> u8 {
        (self * 127.0).round() as u8
    }
}

impl ParameterValue for u8 {
    fn as_f64(self) -> f64 {
        f64::from(self) / 127.0
    }

    fn as_u8(self) -> u8 {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_f64_as_18_invertibility() {
        for i in 0..128 {
            assert_eq!(i.as_f64().as_u8(), i);
        }
        for i in 0..128 {
            assert_eq!((i.as_f64() * 1.001).as_u8(), i);
        }
        for i in 0..128 {
            assert_eq!((i.as_f64() * 0.999).as_u8(), i);
        }
    }
}
