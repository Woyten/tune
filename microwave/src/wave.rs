use std::f64::consts::PI;
use tune::pitch::Pitch;

pub struct Wave {
    phase: f64,
    pitch: Pitch,
    decay_time_secs: f64,
    fade: Option<f64>,
}

impl Wave {
    pub fn new(pitch: Pitch, decay_time_secs: f64) -> Self {
        Self {
            phase: 0.0,
            pitch,
            decay_time_secs,
            fade: None,
        }
    }

    pub fn advance_secs(&mut self, d_secs: f64) {
        self.phase += d_secs * self.pitch.as_hz();
        self.phase %= 1.0;
        self.fade = self
            .fade
            .map(|fade| fade * (-d_secs / self.decay_time_secs).exp());
    }

    pub fn set_frequency(&mut self, pitch: Pitch) {
        self.pitch = pitch;
    }

    pub fn start_fading(&mut self) {
        self.fade = Some(1.0);
    }

    pub fn sine(&self) -> f64 {
        (2.0 * PI * self.phase).sin()
    }

    pub fn triangle(&self) -> f64 {
        ((self.phase + 0.75) % 1.0 - 0.5).abs() * 4.0 - 1.0
    }

    pub fn square(&self) -> f64 {
        let loudness_correction = 4.0;
        (self.phase - 0.5).signum() / loudness_correction
    }

    pub fn sawtooth(&self) -> f64 {
        let loudness_correction = 2.0;
        (((self.phase + 0.5) % 1.0) * 2.0 - 1.0) / loudness_correction
    }

    pub fn amplitude(&self) -> f64 {
        self.fade.unwrap_or(1.0)
    }
}
