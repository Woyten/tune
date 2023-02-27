use crate::{Stage, StageState};

pub struct Waveform<T> {
    stages: Vec<Stage<T>>,
    envelope: Stage<T>,
}

impl<T> Waveform<T> {
    pub fn new(stages: Vec<Stage<T>>, envelope: Stage<T>) -> Self {
        Self { stages, envelope }
    }

    pub fn stages(&mut self) -> impl IntoIterator<Item = &mut Stage<T>> {
        self.stages.iter_mut().chain([&mut self.envelope])
    }

    pub fn is_active(&self) -> bool {
        self.envelope.state == StageState::Active
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
