use crate::{
    automation::{AutomationContext, AutomationSpec},
    buffer::BufferWriter,
};

pub struct Waveform<A: AutomationSpec> {
    pub envelope: Envelope,
    pub stages: Vec<Stage<A>>,
    pub state: WaveformState,
}

pub struct WaveformState {
    pub pitch_hz: f64,
    pub velocity: f64,
    pub secs_since_pressed: f64,
    pub secs_since_released: f64,
}

impl<A: AutomationSpec> Waveform<A> {
    pub fn is_active(&self) -> bool {
        self.envelope.is_active(self.state.secs_since_released)
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

    pub fn is_active(&self, secs_since_released: f64) -> bool {
        secs_since_released <= self.release_time
    }
}

pub struct Stage<A: AutomationSpec> {
    pub(crate) stage_fn: Box<dyn FnMut(&mut BufferWriter, &AutomationContext<A::Storage>) + Send>,
}

impl<A: AutomationSpec> Stage<A> {
    pub fn render(&mut self, buffers: &mut BufferWriter, context: &AutomationContext<A::Storage>) {
        (self.stage_fn)(buffers, context);
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
        assert!(envelope.is_active(0.000));
        assert!(envelope.is_active(1.000));
        assert!(envelope.is_active(1.999));
        assert!(!envelope.is_active(2.001));
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
        assert!(envelope.is_active(0.000));
        assert!(!envelope.is_active(0.001));
    }
}
