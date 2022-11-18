use crate::Stage;

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
