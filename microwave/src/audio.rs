use crate::{wave::Patch, wave::Waveform};
use nannou_audio::{Buffer, Host, Stream};
use std::{collections::HashMap, hash::Hash};
use tune::pitch::Pitch;

pub struct Audio<E> {
    stream: Stream<AudioModel<E>>,
}

struct AudioModel<E> {
    sounds: HashMap<E, Sound>,
}

pub struct Sound {
    waveform: Waveform,
}

impl<E: 'static + Eq + Hash + Send> Audio<E> {
    pub fn new() -> Self {
        let audio_model = AudioModel {
            sounds: HashMap::new(),
        };

        Self {
            stream: Host::new()
                .new_output_stream(audio_model)
                .render(render_audio)
                .build()
                .unwrap(),
        }
    }

    pub fn start(&mut self, id: E, pitch: Pitch, waveform_factory: &Patch) {
        let new_waveform = waveform_factory.new_waveform(pitch, 1.0);
        self.stream
            .send(move |audio| {
                let new_sound = Sound {
                    waveform: new_waveform,
                };
                audio.sounds.insert(id, new_sound);
            })
            .unwrap();
    }

    pub fn update(&mut self, id: E, pitch: Pitch) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.sounds.get_mut(&id) {
                    sound.waveform.set_frequency(pitch);
                }
            })
            .unwrap();
    }

    pub fn stop(&mut self, id: E) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.sounds.get_mut(&id) {
                    sound.waveform.start_fading();
                }
            })
            .unwrap();
    }
}

fn render_audio<E: Eq + Hash>(audio: &mut AudioModel<E>, buffer: &mut Buffer) {
    let mut total_amplitude = 0.0;
    audio.sounds.retain(|_, sound| {
        let amplitude = sound.waveform.amplitude();
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

        for sound in audio.sounds.values_mut() {
            let waveform = &mut sound.waveform;
            let signal = waveform.signal() * waveform.amplitude();
            waveform.advance_secs(sample_width);
            total_signal += signal;
        }

        for channel in frame {
            *channel = (total_signal * volume) as f32;
        }
    }
}
