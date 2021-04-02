use serde::{Deserialize, Serialize};
use tune::pitch::{Pitch, Ratio};

use super::{envelope::Envelope, source::LfSource, Magnetron, WaveformControl};

pub struct Waveform<S> {
    pub envelope: Envelope,
    pub stages: Vec<Stage<S>>,
    pub properties: WaveformProperties,
}

pub struct WaveformProperties {
    pub pitch: Pitch,
    pub pitch_bend: Ratio,
    pub velocity: f64,
    pub pressure: f64,
    pub secs_since_pressed: f64,
    pub secs_since_released: f64,
}

pub type Stage<S> = Box<dyn FnMut(&mut Magnetron, &WaveformControl<S>) + Send>;

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Source {
    Buffer(usize),
    AudioIn(AudioIn),
}

impl Source {
    pub fn audio_in() -> Self {
        Self::AudioIn(AudioIn::AudioIn)
    }
}

// Single variant enum for nice serialization
#[derive(Clone, Deserialize, Serialize)]
pub enum AudioIn {
    AudioIn,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Destination<C> {
    pub buffer: OutBuffer,
    pub intensity: LfSource<C>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutBuffer {
    Buffer(usize),
    AudioOut(AudioOut),
}

impl OutBuffer {
    pub fn audio_out() -> Self {
        Self::AudioOut(AudioOut::AudioOut)
    }
}

// Single variant enum for nice serialization
#[derive(Clone, Deserialize, Serialize)]
pub enum AudioOut {
    AudioOut,
}
