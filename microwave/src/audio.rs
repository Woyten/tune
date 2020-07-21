use crate::{
    effects::Delay,
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    midi::{ChannelMessage, ChannelMessageType},
    model::SelectedProgram,
    wave::Patch,
    wave::Waveform,
};
use fluidlite::{IsPreset, Settings, Synth};
use nannou_audio::{stream, Buffer, Host, Stream};
use std::{collections::HashMap, convert::TryInto, hash::Hash, path::PathBuf, sync::mpsc::Sender};
use tune::{key::PianoKey, note::Note, pitch::Pitch, ratio::Ratio, tuning::Tuning};

pub struct Audio<E> {
    stream: Stream<AudioModel<E>>,
    keypress_tracker: KeypressTracker<E, i32>,
}

struct AudioModel<E> {
    active_waveforms: HashMap<WaveformId<E>, Sound>,
    last_id: u64,
    fluid_synthesizer: Synth,
    program_updates: Sender<SelectedProgram>,
    delay: Delay,
}

impl<E> AudioModel<E> {
    fn send_fluid_message(&self, message: ChannelMessage) {
        let channel = message.channel.into();
        match message.message_type {
            ChannelMessageType::NoteOff {
                key,
                velocity: _, // FluidLite cannot handle release velocities
            } => {
                let _ = self.fluid_synthesizer.note_off(channel, key.into());
            }
            ChannelMessageType::NoteOn { key, velocity } => {
                self.fluid_synthesizer
                    .note_on(channel, key.into(), velocity.into())
                    .unwrap();
            }
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => self
                .fluid_synthesizer
                .key_pressure(channel, key.into(), pressure.into())
                .unwrap(),
            ChannelMessageType::ControlChange { controller, value } => self
                .fluid_synthesizer
                .cc(channel, controller.into(), value.into())
                .unwrap(),
            ChannelMessageType::ProgramChange { program } => {
                self.fluid_synthesizer
                    .program_change(channel, program.into())
                    .unwrap();
                self.program_updates
                    .send(SelectedProgram {
                        program_number: program,
                        program_name: self
                            .fluid_synthesizer
                            .get_channel_preset(0)
                            .and_then(|preset| preset.get_name().map(str::to_owned)),
                    })
                    .unwrap();
            }
            ChannelMessageType::ChannelPressure { pressure } => self
                .fluid_synthesizer
                .channel_pressure(channel, pressure.into())
                .unwrap(),
            ChannelMessageType::PitchBendChange { value } => {
                self.fluid_synthesizer.pitch_bend(channel, value).unwrap()
            }
        }
    }
}

pub struct Sound {
    waveform: Waveform,
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformId<E> {
    Active(E),
    Fading(u64),
}

impl<E: 'static + Eq + Hash + Send> Audio<E> {
    pub fn new(
        soundfont_file_location: Option<PathBuf>,
        buffer_size: usize,
        delay_secs: f32,
        delay_feedback: f32,
        program_updates: Sender<SelectedProgram>,
    ) -> Self {
        let settings = Settings::new().unwrap();
        let synth = Synth::new(settings).unwrap();

        if let Some(soundfont_file_location) = soundfont_file_location {
            synth.sfload(soundfont_file_location, false).unwrap();
        }

        let audio_model = AudioModel {
            active_waveforms: HashMap::new(),
            last_id: 0,
            fluid_synthesizer: synth,
            program_updates,
            delay: Delay::new(
                (delay_secs * (stream::DEFAULT_SAMPLE_RATE * 2) as f32).round() as usize,
                delay_feedback,
            ),
        };

        Self {
            stream: Host::new()
                .new_output_stream(audio_model)
                .frames_per_buffer(buffer_size)
                .render(render_audio)
                .build()
                .unwrap(),
            keypress_tracker: KeypressTracker::new(),
        }
    }

    pub fn retune(&self, tuning: impl Tuning<PianoKey>) {
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
                    .create_key_tuning(0, 0, "microwave-dynamic-tuning", &tunings)
                    .unwrap();
                audio
                    .fluid_synthesizer
                    .activate_tuning(0, 0, 0, true)
                    .unwrap();
            })
            .unwrap();
    }

    pub fn start_waveform(&self, id: E, pitch: Pitch, waveform_factory: &Patch) {
        let new_waveform = waveform_factory.new_waveform(pitch, 1.0);
        self.stream
            .send(move |audio| {
                let new_sound = Sound {
                    waveform: new_waveform,
                };
                audio
                    .active_waveforms
                    .insert(WaveformId::Active(id), new_sound);
            })
            .unwrap();
    }

    pub fn update_waveform(&self, id: E, pitch: Pitch) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.active_waveforms.get_mut(&WaveformId::Active(id)) {
                    sound.waveform.set_frequency(pitch);
                }
            })
            .unwrap();
    }

    pub fn stop_waveform(&self, id: E) {
        self.stream
            .send(move |audio| {
                if let Some(sound) = audio.active_waveforms.remove(&WaveformId::Active(id)) {
                    audio
                        .active_waveforms
                        .insert(WaveformId::Fading(audio.last_id), sound);
                    audio.last_id += 1;
                }
            })
            .unwrap();
    }

    pub fn start_fluid_note(&mut self, id: E, note: i32, velocity: u8) {
        self.keypress_tracker.place_finger_at(id, note).unwrap();
        self.fluid_note_on(note, velocity);
    }

    pub fn update_fluid_note(&mut self, id: &E, note: i32, velocity: u8) {
        match self.keypress_tracker.move_finger_to(id, note) {
            Ok((LiftAction::KeyReleased(released_key), _)) => {
                self.fluid_note_off(released_key);
                self.fluid_note_on(note, velocity);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                self.fluid_note_on(note, velocity);
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

    pub fn submit_fluid_message(&self, message: ChannelMessage) {
        self.stream
            .send(|audio| {
                audio.send_fluid_message(message);
            })
            .unwrap();
    }

    fn fluid_note_on(&self, note: i32, velocity: u8) {
        if let Ok(key) = note.try_into() {
            if key < 128 {
                self.submit_fluid_message(ChannelMessage {
                    channel: 0,
                    message_type: ChannelMessageType::NoteOn { key, velocity },
                })
            }
        }
    }

    fn fluid_note_off(&self, note: i32) {
        if let Ok(key) = note.try_into() {
            if key < 128 {
                self.submit_fluid_message(ChannelMessage {
                    channel: 0,
                    message_type: ChannelMessageType::NoteOff { key, velocity: 100 },
                })
            }
        }
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

        for (id, sound) in &mut audio.active_waveforms {
            let waveform = &mut sound.waveform;
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

    audio.delay.process(&mut buffer[..])
}
