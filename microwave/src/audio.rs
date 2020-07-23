use crate::{
    effects::Delay,
    fluid::{FluidGlobalMessage, FluidMessage, FluidPolyphonicMessage, FluidSynth},
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    midi::ChannelMessageType,
    synth::WaveformSynth,
    tuner::ChannelTuner,
    wave::Patch,
};
use nannou_audio::{stream, Buffer, Host, Stream};
use std::{convert::TryInto, fmt::Debug, hash::Hash, sync::mpsc::Sender};
use tune::{key::PianoKey, pitch::Pitch, tuning::Tuning};

pub struct Audio<E> {
    stream: Stream<AudioModel<E>>,
    keypress_tracker: KeypressTracker<E, PianoKey>,
    channel_tuner: ChannelTuner,
    fluid_messages: Sender<FluidMessage>,
}

struct AudioModel<E> {
    waveform_synth: WaveformSynth<E>,
    fluid_synth: FluidSynth,
    delay: Delay,
}

impl<E: 'static + Eq + Hash + Send + Debug> Audio<E> {
    pub fn new(
        fluid_synth: FluidSynth,
        buffer_size: usize,
        delay_secs: f32,
        delay_feedback: f32,
    ) -> Self {
        let fluid_messages = fluid_synth.messages();

        let audio_model = AudioModel {
            waveform_synth: WaveformSynth::new(),
            fluid_synth,
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
            fluid_messages,
            keypress_tracker: KeypressTracker::new(),
            channel_tuner: ChannelTuner::new(),
        }
    }

    pub fn retune(&mut self, tuning: impl Tuning<PianoKey>) -> (PianoKey, PianoKey) {
        let channel_tunings = self
            .channel_tuner
            .set_tuning(&tuning)
            .expect("Cannot apply tuning: There are too many notes in one semitone");

        self.fluid_messages
            .send(FluidMessage::Retune { channel_tunings })
            .unwrap();

        self.channel_tuner.boundaries()
    }

    pub fn start_waveform(&self, id: E, pitch: Pitch, patch: &Patch) {
        let waveform = patch.new_waveform(pitch, 1.0);
        self.stream
            .send(move |audio| audio.waveform_synth.start_waveform(id, waveform))
            .unwrap();
    }

    pub fn update_waveform(&self, id: E, pitch: Pitch) {
        self.stream
            .send(move |audio| audio.waveform_synth.update_waveform(id, pitch))
            .unwrap();
    }

    pub fn stop_waveform(&self, id: E) {
        self.stream
            .send(move |audio| audio.waveform_synth.stop_waveform(id))
            .unwrap();
    }

    pub fn start_fluid_note(&mut self, id: E, key: PianoKey, velocity: u8) {
        match self.keypress_tracker.place_finger_at(id, key) {
            Ok(PlaceAction::KeyPressed) | Ok(PlaceAction::KeyAlreadyPressed) => {
                self.fluid_note_on(key, velocity)
            }
            Err(id) => eprintln!(
                "[WARNING] key {:?} with ID {:?} pressed before released",
                key, id
            ),
        }
    }

    pub fn update_fluid_note(&mut self, id: &E, key: PianoKey, velocity: u8) {
        match self.keypress_tracker.move_finger_to(id, key) {
            Ok((LiftAction::KeyReleased(released_key), _)) => {
                self.fluid_note_off(released_key);
                self.fluid_note_on(key, velocity);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                self.fluid_note_on(key, velocity);
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

    fn fluid_note_on(&self, key: PianoKey, velocity: u8) {
        if let Some((channel, note)) = self.channel_and_note_for_key(key) {
            self.fluid_messages
                .send(FluidMessage::Polyphonic {
                    channel,
                    note,
                    event: FluidPolyphonicMessage::NoteOn { velocity },
                })
                .unwrap();
        }
    }

    fn fluid_note_off(&self, key: PianoKey) {
        if let Some((channel, note)) = self.channel_and_note_for_key(key) {
            self.fluid_messages
                .send(FluidMessage::Polyphonic {
                    channel,
                    note,
                    event: FluidPolyphonicMessage::NoteOff,
                })
                .unwrap();
        }
    }

    pub fn submit_fluid_message(&self, message_type: ChannelMessageType) {
        // We currently do not support multiple input channels, s.t. the channel is ignored
        let message = match message_type {
            ChannelMessageType::NoteOff { .. } | ChannelMessageType::NoteOn { .. } => {
                unreachable!("NoteOff or NoteOn not expected here")
            }
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                if let Some((channel, note)) =
                    self.channel_and_note_for_key(PianoKey::from_midi_number(key.into()))
                {
                    FluidMessage::Polyphonic {
                        channel,
                        note,
                        event: FluidPolyphonicMessage::KeyPressure { pressure },
                    }
                } else {
                    return;
                }
            }
            ChannelMessageType::ControlChange { controller, value } => FluidMessage::Channel {
                event: FluidGlobalMessage::ControlChange { controller, value },
            },
            ChannelMessageType::ProgramChange { program } => FluidMessage::Channel {
                event: FluidGlobalMessage::ProgramChange { program },
            },
            ChannelMessageType::ChannelPressure { pressure } => FluidMessage::Channel {
                event: FluidGlobalMessage::ChannelPressure { pressure },
            },
            ChannelMessageType::PitchBendChange { value } => FluidMessage::Channel {
                event: FluidGlobalMessage::PitchBendChange { value },
            },
        };
        self.fluid_messages.send(message).unwrap();
    }

    fn channel_and_note_for_key(&self, key: PianoKey) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.channel_tuner.get_channel_and_note_for_key(key) {
            if let Ok(key) = note.midi_number().try_into() {
                if key < 128 {
                    return Some((channel, key));
                }
            }
        }
        None
    }
}

fn render_audio<E: Eq + Hash>(audio: &mut AudioModel<E>, buffer: &mut Buffer) {
    audio.fluid_synth.write(buffer);
    audio.waveform_synth.write(buffer);
    audio.delay.process(&mut buffer[..])
}
