use crate::wave::Waveform;
use nannou_audio::Buffer;
use std::{
    collections::HashMap,
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};
use tune::pitch::Pitch;

pub struct WaveformSynth<E> {
    state: WaveformState<E>,
    messages: Receiver<WaveformMessage<E>>,
    message_sender: Sender<WaveformMessage<E>>,
}

pub struct WaveformMessage<E> {
    pub id: E,
    pub action: WaveformAction,
}

pub enum WaveformAction {
    Start { waveform: Waveform },
    Update { pitch: Pitch },
    Stop,
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn new() -> Self {
        let state = WaveformState {
            active: HashMap::new(),
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
        self.state.active.retain(|_, waveform| {
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

        for waveform in self.state.active.values_mut() {
            waveform.advance_secs(&mut buffer[..], sample_width, volume);
        }
    }
}

struct WaveformState<E> {
    active: HashMap<WaveformId<E>, Waveform>,
    last_id: u64,
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformId<E> {
    Active(E),
    Fading(u64),
}

impl<E: Eq + Hash> WaveformState<E> {
    fn process_message(&mut self, message: WaveformMessage<E>) {
        match message.action {
            WaveformAction::Start { waveform } => {
                self.active.insert(WaveformId::Active(message.id), waveform);
            }
            WaveformAction::Update { pitch } => {
                if let Some(waveform) = self.active.get_mut(&WaveformId::Active(message.id)) {
                    waveform.set_frequency(pitch);
                }
            }
            WaveformAction::Stop => {
                if let Some(mut sound) = self.active.remove(&WaveformId::Active(message.id)) {
                    sound.start_fading();
                    self.active.insert(WaveformId::Fading(self.last_id), sound);
                    self.last_id += 1;
                }
            }
        }
    }
}
