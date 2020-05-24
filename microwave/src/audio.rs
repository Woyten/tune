use crate::{
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    wave::Patch,
    wave::Waveform,
};
use fluidlite::{IsPreset, Settings, Synth};
use nannou_audio::{Buffer, Host, Stream};
use std::{
    collections::HashMap,
    convert::TryFrom,
    hash::Hash,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tune::{key::PianoKey, note::Note, pitch::Pitch, ratio::Ratio, tuning::Tuning};

pub struct Audio<E> {
    stream: Stream<AudioModel<E>>,
    keypress_tracker: KeypressTracker<E, i32>,
}

struct AudioModel<E> {
    active_waveforms: HashMap<E, Sound>,
    fluid_synthesizer: Synth,
}

pub struct Sound {
    waveform: Waveform,
}

impl<E: 'static + Eq + Hash + Send> Audio<E> {
    pub fn new(soundfont_file_location: Option<PathBuf>) -> Self {
        let settings = Settings::new().unwrap();
        let synth = Synth::new(settings).unwrap();

        if let Some(soundfont_file_location) = soundfont_file_location {
            synth.sfload(soundfont_file_location, false).unwrap();
        }

        let audio_model = AudioModel {
            active_waveforms: HashMap::new(),
            fluid_synthesizer: synth,
        };

        Self {
            stream: Host::new()
                .new_output_stream(audio_model)
                .render(render_audio)
                .build()
                .unwrap(),
            keypress_tracker: KeypressTracker::new(),
        }
    }

    pub fn set_program(&mut self, program_number: u32, name: Arc<Mutex<Option<String>>>) {
        self.stream
            .send(move |audio| {
                audio
                    .fluid_synthesizer
                    .program_change(0, program_number)
                    .unwrap();
                if let Some(preset) = audio.fluid_synthesizer.get_channel_preset(0) {
                    *name.lock().unwrap() = preset.get_name().map(str::to_owned);
                }
            })
            .unwrap()
    }

    pub fn retune(&mut self, tuning: impl Tuning<PianoKey>) {
        let mut tunings = [0.0; 128];

        for midi_number in 0..128 {
            let piano_key = Note::from_midi_number(midi_number).as_piano_key();
            let tuned_pitch = tuning.pitch_of(piano_key);
            let tuning_diff = Ratio::between_pitches(Note::from_midi_number(0), tuned_pitch);
            tunings[midi_number as usize] = tuning_diff.as_cents();
        }

        self.stream
            .send(move |audio| {
                audio
                    .fluid_synthesizer
                    .create_key_tuning(0, 0, "bla", &tunings)
                    .unwrap();
                audio
                    .fluid_synthesizer
                    .activate_tuning(0, 0, 0, true)
                    .unwrap();
            })
            .unwrap();
    }

    pub fn start_waveform(&mut self, id: E, pitch: Pitch, waveform_factory: &Patch) {
        let new_waveform = waveform_factory.new_waveform(pitch, 1.0);
        self.stream
            .send(move |audio| {
                let new_sound = Sound {
                    waveform: new_waveform,
                };
                audio.active_waveforms.insert(id, new_sound);
            })
            .unwrap();
    }

    pub fn update_waveform(&mut self, id: E, pitch: Pitch) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.active_waveforms.get_mut(&id) {
                    sound.waveform.set_frequency(pitch);
                }
            })
            .unwrap();
    }

    pub fn stop_waveform(&mut self, id: E) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.active_waveforms.get_mut(&id) {
                    sound.waveform.start_fading();
                }
            })
            .unwrap();
    }

    pub fn start_fluid_note(&mut self, id: E, note: i32) {
        match self.keypress_tracker.place_finger_at(id, note) {
            Ok(_) => self.fluid_note_on(note),
            Err(_) => unreachable!(),
        };
    }

    pub fn update_fluid_note(&mut self, id: &E, note: i32) {
        match self.keypress_tracker.move_finger_to(id, note) {
            Ok((LiftAction::KeyReleased(released_key), _)) => {
                self.fluid_note_off(released_key);
                self.fluid_note_on(note);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                self.fluid_note_on(note);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
            Err(IllegalState) => {
                // Occurs when mouse moved
            }
        };
    }

    pub fn stop_fluid_note(&mut self, id: &E) {
        match self.keypress_tracker.lift_finger(id) {
            Ok(LiftAction::KeyReleased(released_note)) => self.fluid_note_off(released_note),
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {
                // Occurs when in waveform mode
            }
        }
    }

    fn fluid_note_on(&self, note: i32) {
        self.stream
            .send(move |audio| {
                if let Ok(note) = u32::try_from(note) {
                    if note < 128 {
                        audio.fluid_synthesizer.note_on(0, note, 100).unwrap();
                    }
                }
            })
            .unwrap();
    }

    fn fluid_note_off(&mut self, note: i32) {
        self.stream
            .send(move |audio| {
                if let Ok(note) = u32::try_from(note) {
                    if note < 128 {
                        let _ = audio.fluid_synthesizer.note_off(0, note);
                    }
                }
            })
            .unwrap();
    }
}

fn render_audio<E: Eq + Hash>(audio: &mut AudioModel<E>, buffer: &mut Buffer) {
    audio.fluid_synthesizer.write(&mut buffer[..]).unwrap();

    let mut total_amplitude = 0.0;
    audio.active_waveforms.retain(|_, sound| {
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

        for sound in audio.active_waveforms.values_mut() {
            let waveform = &mut sound.waveform;
            let signal = waveform.signal() * waveform.amplitude();
            waveform.advance_secs(sample_width);
            total_signal += signal;
        }

        for channel in frame {
            *channel += (total_signal * volume) as f32;
        }
    }
}
