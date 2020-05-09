use crate::{model::Waveform, wave::Wave};
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
    wave: Wave,
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

    pub fn start(&mut self, id: E, pitch: Pitch, waveform: Waveform) {
        self.stream
            .send(move |audio| {
                let new_sound = Sound {
                    wave: Wave::new(pitch, 1.0),
                    waveform,
                };
                audio.sounds.insert(id, new_sound);
            })
            .unwrap();
    }

    pub fn update(&mut self, id: E, pitch: Pitch) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.sounds.get_mut(&id) {
                    sound.wave.set_frequency(pitch);
                }
            })
            .unwrap();
    }

    pub fn stop(&mut self, id: E) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.sounds.get_mut(&id) {
                    sound.wave.start_fading();
                }
            })
            .unwrap();
    }
}

fn render_audio<E: Eq + Hash>(audio: &mut AudioModel<E>, buffer: &mut Buffer) {
    let mut total_amplitude = 0.0;
    audio.sounds.retain(|_, sound| {
        let amplitude = sound.wave.amplitude();
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
            let wave = &mut sound.wave;
            let raw_signal = match sound.waveform {
                Waveform::Sine => wave.sine(),
                Waveform::Triangle => wave.triangle(),
                Waveform::Square => wave.square(),
                Waveform::Sawtooth => wave.sawtooth(),
            };
            let signal = raw_signal * wave.amplitude();
            wave.advance_secs(sample_width);
            total_signal += signal;
        }

        for channel in frame {
            *channel = (total_signal * volume) as f32;
        }
    }
}
