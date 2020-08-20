use crate::wave::Waveform;
use nannou_audio::Buffer;
use std::{
    collections::HashMap,
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};
use tune::pitch::Pitch;

pub struct WaveformSynth<E> {
    state: SynthState<E>,
    messages: Receiver<WaveformMessage<E>>,
    message_sender: Sender<WaveformMessage<E>>,
}

pub enum WaveformMessage<E> {
    Lifecycle { id: E, action: WaveformLifecycle },
    DamperPedal { pressure: f64 },
}

pub enum WaveformLifecycle {
    Start { waveform: Waveform },
    Update { pitch: Pitch },
    Stop,
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn new() -> Self {
        let state = SynthState {
            playing: HashMap::new(),
            damper_pedal_pressure: 0.0,
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

        let mut total_amplitude = 0.0;
        self.state.playing.retain(|_, waveform| {
            let amplitude = waveform.amplitude();
            if amplitude > 0.0001 {
                total_amplitude += amplitude;
                true
            } else {
                false
            }
        });

        let volume = (0.1f64).min(0.5 / total_amplitude); // 1/10 per wave, but at most 1/2 in total

        let sample_width = 1.0 / buffer.sample_rate() as f64;

        for waveform in self.state.playing.values_mut() {
            waveform.advance_secs(&mut buffer[..], sample_width, volume);
        }
    }
}

struct SynthState<E> {
    playing: HashMap<WaveformId<E>, Waveform>,
    damper_pedal_pressure: f64,
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
                        waveform.set_frequency(pitch);
                    }
                }
                WaveformLifecycle::Stop => {
                    if let Some(mut waveform) = self.playing.remove(&WaveformId::Stable(id)) {
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
        }
    }
}
