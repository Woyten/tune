use std::{
    collections::HashMap,
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};

use nannou_audio::Buffer;
use tune::{pitch::Pitch, ratio::Ratio};

use crate::waveform::{Buffers, Waveform};

pub struct WaveformSynth<E> {
    state: SynthState<E>,
    messages: Receiver<WaveformMessage<E>>,
    message_sender: Sender<WaveformMessage<E>>,
}

pub enum WaveformMessage<E> {
    Lifecycle { id: E, action: WaveformLifecycle },
    DamperPedal { pressure: f64 },
    PitchBend { bend_level: f64 },
}

pub enum WaveformLifecycle {
    Start { waveform: Waveform },
    Update { pitch: Pitch },
    Stop,
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn new(pitch_wheel_sensivity: Ratio) -> Self {
        let state = SynthState {
            playing: HashMap::new(),
            buffers: Buffers::new(),
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

    pub fn write(&mut self, buffer: &mut Buffer) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        self.state
            .playing
            .retain(|_, waveform| waveform.amplitude() > 0.0001);

        let sample_width = 1.0 / buffer.sample_rate() as f64;

        self.state.buffers.clear(buffer.len() / 2);
        for (id, waveform) in &mut self.state.playing {
            let sample_width = match id {
                WaveformId::Stable(_) => sample_width * self.state.pitch_bend.as_float(),
                WaveformId::Fading(_) => sample_width, // Do no bend released notes
            };
            waveform.write(&mut self.state.buffers, sample_width);
        }

        for (&out, target) in self.state.buffers.total().iter().zip(buffer.chunks_mut(2)) {
            if let [left, right] = target {
                *left += out as f32 / 10.0;
                *right += out as f32 / 10.0;
            }
        }
    }
}

struct SynthState<E> {
    playing: HashMap<WaveformId<E>, Waveform>,
    buffers: Buffers,
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
                WaveformLifecycle::Update { pitch } => {
                    if let Some(waveform) = self.playing.get_mut(&WaveformId::Stable(id)) {
                        waveform.set_pitch(pitch);
                    }
                }
                WaveformLifecycle::Stop => {
                    if let Some(mut waveform) = self.playing.remove(&WaveformId::Stable(id)) {
                        // Since released notes are not bent we need to freeze the pitch bend level to avoid a pitch flip
                        waveform.set_pitch(waveform.pitch() * self.pitch_bend);
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
        }
    }
}
