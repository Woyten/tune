use crate::wave::Waveform;
use nannou_audio::Buffer;
use std::{collections::HashMap, hash::Hash};
use tune::pitch::Pitch;

pub struct WaveformSynth<E> {
    active_waveforms: HashMap<WaveformId<E>, Waveform>,
    last_id: u64,
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn new() -> Self {
        Self {
            active_waveforms: HashMap::new(),
            last_id: 0,
        }
    }

    pub fn start_waveform(&mut self, id: E, waveform: Waveform) {
        self.active_waveforms
            .insert(WaveformId::Active(id), waveform);
    }

    pub fn update_waveform(&mut self, id: E, pitch: Pitch) {
        if let Some(waveform) = self.active_waveforms.get_mut(&WaveformId::Active(id)) {
            waveform.set_frequency(pitch);
        }
    }

    pub fn stop_waveform(&mut self, id: E) {
        if let Some(sound) = self.active_waveforms.remove(&WaveformId::Active(id)) {
            self.active_waveforms
                .insert(WaveformId::Fading(self.last_id), sound);
            self.last_id += 1;
        }
    }

    pub fn write(&mut self, buffer: &mut Buffer) {
        let mut total_amplitude = 0.0;
        self.active_waveforms.retain(|_, waveform| {
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

        for frame in buffer.frames_mut() {
            let mut total_signal = 0.0;

            for (id, waveform) in &mut self.active_waveforms {
                let signal = waveform.signal() * waveform.amplitude();
                waveform.advance_secs(sample_width);
                if let WaveformId::Fading(_) = id {
                    waveform.advance_fade_secs(sample_width)
                }
                total_signal += signal;
            }

            for channel in frame {
                *channel += (total_signal * volume) as f32;
            }
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformId<E> {
    Active(E),
    Fading(u64),
}
