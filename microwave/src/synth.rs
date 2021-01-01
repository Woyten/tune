use std::{
    collections::HashMap,
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};

use nannou_audio::Buffer;
use ringbuf::Consumer;
use serde::{Deserialize, Serialize};
use tune::{pitch::Pitch, ratio::Ratio};

use crate::magnetron::{control::Controller, waveform::Waveform, Magnetron};

pub struct WaveformSynth<E> {
    state: SynthState<E>,
    messages: Receiver<WaveformMessage<E>>,
    message_sender: Sender<WaveformMessage<E>>,
}

pub enum WaveformMessage<E> {
    Lifecycle { id: E, action: WaveformLifecycle },
    DamperPedal { pressure: f64 },
    PitchBend { bend_level: f64 },
    Control { control: SynthControl, value: f64 },
}

pub enum WaveformLifecycle {
    Start { waveform: Waveform<ControlStorage> },
    UpdatePitch { pitch: Pitch },
    UpdatePressure { pressure: f64 },
    Stop,
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn new(pitch_wheel_sensivity: Ratio) -> Self {
        let state = SynthState {
            playing: HashMap::new(),
            storage: ControlStorage {
                values: HashMap::new(),
            },
            magnetron: Magnetron::new(),
            damper_pedal_pressure: 0.0,
            pitch_wheel_sensivity,
            pitch_bend: Ratio::default(),
            last_id: 0,
        };
        let (sender, receiver) = mpsc::channel();

        Self {
            state,
            messages: receiver,
            message_sender: sender,
        }
    }

    pub fn messages(&self) -> Sender<WaveformMessage<E>> {
        self.message_sender.clone()
    }

    pub fn write(&mut self, buffer: &mut Buffer, audio_in: &mut Consumer<f32>) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        let sample_width = 1.0 / buffer.sample_rate() as f64;

        let SynthState {
            playing,
            magnetron: buffers,
            storage: control,
            pitch_bend,
            ..
        } = &mut self.state;

        buffers.clear(buffer.len() / 2);
        buffers.set_audio_in(audio_in);

        playing.retain(|id, waveform| {
            if waveform.properties.curr_amplitude < 0.0001 {
                return false;
            }
            let sample_width = match id {
                WaveformId::Stable(_) => sample_width * pitch_bend.as_float(),
                WaveformId::Fading(_) => sample_width, // Do no bend released notes
            };
            buffers.write(waveform, control, sample_width);
            true
        });

        for (&out, target) in buffers.total().iter().zip(buffer.chunks_mut(2)) {
            if let [left, right] = target {
                *left += out as f32 / 10.0;
                *right += out as f32 / 10.0;
            }
        }
    }
}

struct SynthState<E> {
    playing: HashMap<WaveformId<E>, Waveform<ControlStorage>>,
    storage: ControlStorage,
    magnetron: Magnetron,
    damper_pedal_pressure: f64,
    pitch_wheel_sensivity: Ratio,
    pitch_bend: Ratio,
    last_id: u64,
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformId<E> {
    Stable(E),
    Fading(u64),
}

impl<E: Eq + Hash> SynthState<E> {
    fn process_message(&mut self, message: WaveformMessage<E>) {
        match message {
            WaveformMessage::Lifecycle { id, action } => match action {
                WaveformLifecycle::Start { waveform } => {
                    self.playing.insert(WaveformId::Stable(id), waveform);
                }
                WaveformLifecycle::UpdatePitch { pitch } => {
                    if let Some(waveform) = self.playing.get_mut(&WaveformId::Stable(id)) {
                        waveform.properties.pitch = pitch;
                    }
                }
                WaveformLifecycle::UpdatePressure { pressure } => {
                    if let Some(waveform) = self.playing.get_mut(&WaveformId::Stable(id)) {
                        waveform.properties.pressure = pressure
                    }
                }
                WaveformLifecycle::Stop => {
                    if let Some(mut waveform) = self.playing.remove(&WaveformId::Stable(id)) {
                        // Since released notes are not bent we need to freeze the pitch bend level to avoid a pitch flip
                        waveform.properties.pitch = waveform.properties.pitch * self.pitch_bend;
                        waveform.set_fade(self.damper_pedal_pressure);
                        self.playing
                            .insert(WaveformId::Fading(self.last_id), waveform);
                        self.last_id += 1;
                    }
                }
            },
            WaveformMessage::DamperPedal { pressure } => {
                let curve = pressure.max(0.0).min(1.0).cbrt();
                self.damper_pedal_pressure = curve;
                for (id, waveform) in &mut self.playing {
                    if let WaveformId::Fading(_) = id {
                        waveform.set_fade(self.damper_pedal_pressure)
                    }
                }
            }
            WaveformMessage::PitchBend { bend_level } => {
                self.pitch_bend = self.pitch_wheel_sensivity.repeated(bend_level);
            }
            WaveformMessage::Control { control, value } => {
                self.storage.write(control, value);
            }
        }
    }
}

#[derive(Clone)]
pub struct ControlStorage {
    values: HashMap<SynthControl, f64>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum SynthControl {
    Modulation,
    Breath,
    Foot,
    Expression,
    Damper,
    Sostenuto,
    SoftPedal,
    ChannelPressure,
    MouseY,
}

impl Controller for SynthControl {
    type Storage = ControlStorage;

    fn read(&self, storage: &Self::Storage) -> f64 {
        storage.values.get(self).copied().unwrap_or_default()
    }
}

impl ControlStorage {
    pub fn write(&mut self, control: SynthControl, value: f64) {
        self.values.insert(control, value);
    }
}
