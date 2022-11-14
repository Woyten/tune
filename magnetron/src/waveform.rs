use std::cmp::Ordering;

use crate::{Stage, StageState};

pub struct Waveform<T> {
    pub stages: Vec<Stage<T>>,
    pub envelope: Stage<T>,
    pub is_active: bool,
}

#[derive(Copy, Clone)]
pub struct WaveformProperties {
    pub pitch_hz: f64,
    pub velocity: f64,
    pub key_pressure: f64,
    pub fadeout: f64,
}

impl WaveformProperties {
    pub fn initial(pitch_hz: f64, velocity: f64) -> Self {
        Self {
            pitch_hz,
            velocity,
            key_pressure: 0.0,
            fadeout: 0.0,
        }
    }
}

#[derive(Clone)]
pub struct Envelope {
    pub attack_time: f64,
    pub release_time: f64,
    pub decay_rate: f64,
}

impl Envelope {
    pub fn get_value(&self, secs_since_pressed: f64, secs_since_released: f64) -> f64 {
        let signal_without_release = if secs_since_pressed < self.attack_time {
            secs_since_pressed / self.attack_time
        } else {
            ((self.attack_time - secs_since_pressed) * self.decay_rate).exp2()
        };

        if secs_since_released < self.release_time {
            signal_without_release * (1.0 - secs_since_released / self.release_time)
        } else {
            0.0
        }
    }

    pub fn state(&self, secs_since_released: f64) -> StageState {
        match secs_since_released.partial_cmp(&self.release_time) {
            Some(Ordering::Less | Ordering::Equal) => StageState::Active,
            Some(Ordering::Greater) | None => StageState::Exhausted,
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn normal_envelope() {
        let envelope = Envelope {
            attack_time: 1.0,
            release_time: 2.0,
            decay_rate: 0.0,
        };

        assert_approx_eq!(envelope.get_value(0.0, 0.0), 0.00);
        assert_approx_eq!(envelope.get_value(0.5, 0.0), 0.50);
        assert_approx_eq!(envelope.get_value(1.0, 0.0), 1.00);
        assert_approx_eq!(envelope.get_value(1.5, 0.0), 1.00);
        assert_approx_eq!(envelope.get_value(2.0, 0.0), 1.00);
        assert_approx_eq!(envelope.get_value(2.0, 0.5), 0.75);
        assert_approx_eq!(envelope.get_value(2.0, 1.0), 0.50);
        assert_approx_eq!(envelope.get_value(2.0, 1.5), 0.25);
        assert_approx_eq!(envelope.get_value(2.0, 2.0), 0.00);
        assert_eq!(envelope.state(0.000), StageState::Active);
        assert_eq!(envelope.state(1.000), StageState::Active);
        assert_eq!(envelope.state(1.999), StageState::Active);
        assert_eq!(envelope.state(2.001), StageState::Exhausted);
    }

    #[test]
    fn trivial_envelope() {
        let envelope = Envelope {
            attack_time: 1e-10,
            release_time: 1e-10,
            decay_rate: 0.0,
        };

        assert_approx_eq!(envelope.get_value(0.000, 0.000), 0.0);
        assert_approx_eq!(envelope.get_value(0.001, 0.000), 1.0);
        assert_approx_eq!(envelope.get_value(0.001, 0.001), 0.0);
        assert_eq!(envelope.state(0.000), StageState::Active);
        assert_eq!(envelope.state(0.001), StageState::Exhausted);
    }
}
