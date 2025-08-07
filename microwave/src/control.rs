use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

use crate::magnetron::source::StorageAccess;

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

#[derive(Clone, Default)]
pub struct LiveParameterStorage {
    modulation: f64,
    breath: f64,
    foot: f64,
    volume: f64,
    balance: f64,
    pan: f64,
    expression: f64,
    damper: f64,
    sostenuto: f64,
    soft: f64,
    legato: f64,
    sound_1: f64,
    sound_2: f64,
    sound_3: f64,
    sound_4: f64,
    sound_5: f64,
    sound_6: f64,
    sound_7: f64,
    sound_8: f64,
    sound_9: f64,
    sound_10: f64,
    channel_pressure: f64,
    pitch_bend: f64,
}

impl LiveParameterStorage {
    pub fn set_parameter(&mut self, parameter: LiveParameter, value: f64) {
        *match parameter {
            LiveParameter::Modulation => &mut self.modulation,
            LiveParameter::Breath => &mut self.breath,
            LiveParameter::Foot => &mut self.foot,
            LiveParameter::Volume => &mut self.volume,
            LiveParameter::Balance => &mut self.balance,
            LiveParameter::Pan => &mut self.pan,
            LiveParameter::Expression => &mut self.expression,
            LiveParameter::Damper => &mut self.damper,
            LiveParameter::Sostenuto => &mut self.sostenuto,
            LiveParameter::Soft => &mut self.soft,
            LiveParameter::Legato => &mut self.legato,
            LiveParameter::Sound1 => &mut self.sound_1,
            LiveParameter::Sound2 => &mut self.sound_2,
            LiveParameter::Sound3 => &mut self.sound_3,
            LiveParameter::Sound4 => &mut self.sound_4,
            LiveParameter::Sound5 => &mut self.sound_5,
            LiveParameter::Sound6 => &mut self.sound_6,
            LiveParameter::Sound7 => &mut self.sound_7,
            LiveParameter::Sound8 => &mut self.sound_8,
            LiveParameter::Sound9 => &mut self.sound_9,
            LiveParameter::Sound10 => &mut self.sound_10,
            LiveParameter::ChannelPressure => &mut self.channel_pressure,
            LiveParameter::PitchBend => &mut self.pitch_bend,
        } = value.max(-1.0).min(1.0)
    }

    pub fn read_parameter(&self, parameter: LiveParameter) -> f64 {
        match parameter {
            LiveParameter::Modulation => self.modulation,
            LiveParameter::Breath => self.breath,
            LiveParameter::Foot => self.foot,
            LiveParameter::Volume => self.volume,
            LiveParameter::Balance => self.balance,
            LiveParameter::Pan => self.pan,
            LiveParameter::Expression => self.expression,
            LiveParameter::Damper => self.damper,
            LiveParameter::Sostenuto => self.sostenuto,
            LiveParameter::Soft => self.soft,
            LiveParameter::Legato => self.legato,
            LiveParameter::Sound1 => self.sound_1,
            LiveParameter::Sound2 => self.sound_2,
            LiveParameter::Sound3 => self.sound_3,
            LiveParameter::Sound4 => self.sound_4,
            LiveParameter::Sound5 => self.sound_5,
            LiveParameter::Sound6 => self.sound_6,
            LiveParameter::Sound7 => self.sound_7,
            LiveParameter::Sound8 => self.sound_8,
            LiveParameter::Sound9 => self.sound_9,
            LiveParameter::Sound10 => self.sound_10,
            LiveParameter::ChannelPressure => self.channel_pressure,
            LiveParameter::PitchBend => self.pitch_bend,
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
    Volume,
    Balance,
    Pan,
    Expression,
    Damper,
    Sostenuto,
    Soft,
    Legato,
    Sound1,
    Sound2,
    Sound3,
    Sound4,
    Sound5,
    Sound6,
    Sound7,
    Sound8,
    Sound9,
    Sound10,
    ChannelPressure,
    PitchBend,
}

impl StorageAccess for LiveParameter {
    type Storage = LiveParameterStorage;

    fn access(&mut self, storage: &Self::Storage) -> f64 {
        storage.read_parameter(*self)
    }
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
        if self < 0.0 {
            0
        } else if self < 0.5 {
            (self * 128.0).round() as u8
        } else if self < 1.0 {
            64 + ((self - 0.5) * 63.0 * 2.0).round() as u8
        } else if self.is_nan() {
            64
        } else {
            127
        }
    }
}

impl ParameterValue for u8 {
    fn as_f64(self) -> f64 {
        if self < 64 {
            f64::from(self) / 128.0
        } else if self < 128 {
            0.5 + f64::from(self - 64) / 63.0 * 0.5
        } else {
            1.0
        }
    }

    fn as_u8(self) -> u8 {
        self
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn as_f64_correctness() {
        assert_approx_eq!(0.as_f64(), 0.0);
        assert_approx_eq!(32.as_f64(), 0.25);
        assert_approx_eq!(64.as_f64(), 0.5);
        assert_approx_eq!(95.as_f64(), 0.746032);
        assert_approx_eq!(96.as_f64(), 0.753968);
        assert_approx_eq!(127.as_f64(), 1.0);
        assert_approx_eq!(128.as_f64(), 1.0);
        assert_approx_eq!(255.as_f64(), 1.0);
    }

    #[test]
    fn as_u8_correctness() {
        assert_eq!((f64::NEG_INFINITY).as_u8(), 0);
        assert_eq!((-100.0).as_u8(), 0);
        assert_eq!((-10.0).as_u8(), 0);
        assert_eq!((-1.0).as_u8(), 0);
        assert_eq!(0.0.as_u8(), 0);
        assert_eq!(0.25.as_u8(), 32);
        assert_eq!(0.5.as_u8(), 64);
        assert_eq!(0.746032.as_u8(), 95);
        assert_eq!(0.753968.as_u8(), 96);
        assert_eq!(1.0.as_u8(), 127);
        assert_eq!(10.0.as_u8(), 127);
        assert_eq!(100.0.as_u8(), 127);
        assert_eq!((f64::INFINITY).as_u8(), 127);

        assert_eq!((f64::NAN).as_u8(), 64);
    }

    #[test]
    fn as_f64_as_u8_invertibility() {
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
