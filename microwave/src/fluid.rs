use fluidlite_lib as _;

use std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    hash::Hash,
    path::Path,
    sync::mpsc::{self, Sender},
};

use fluidlite::{IsPreset, Settings, Synth};
use mpsc::Receiver;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    note::Note,
    pitch::{Pitch, Pitched},
    scala::{KbmRoot, Scl},
    tuner::{ChannelTuner, FullKeyboardDetuning},
    tuning::{Scale, Tuning},
};

use crate::{
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    model::SelectedProgram,
    piano::Backend,
};

pub fn create<E: Eq + Hash>(
    soundfont_file_location: Option<&Path>,
    program_updates: Sender<SelectedProgram>,
) -> (FluidSynth, FluidBackend<E>) {
    let settings = Settings::new().unwrap();
    let synth = Synth::new(settings).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        synth.sfload(soundfont_file_location, false).unwrap();
    }

    for channel in 0..16 {
        // Initialize the bank s.t. channel 9 will not have a drum kit loaded
        synth.bank_select(channel, 0).unwrap();

        // Initilize the program s.t. fluidsynth will not error on note_on
        synth.program_change(channel, 0).unwrap();
    }

    let (send, recv) = mpsc::channel();

    (
        FluidSynth {
            synth,
            messages: recv,
            program_updates,
        },
        FluidBackend {
            messages: send,
            tuner: ChannelTuner::empty(),
            keypress_tracker: KeypressTracker::new(),
        },
    )
}

pub struct FluidBackend<E> {
    messages: Sender<FluidMessage>,
    tuner: ChannelTuner<i32>,
    keypress_tracker: KeypressTracker<E, (u8, u8)>,
}

impl<E: Send + Eq + Hash + Debug> Backend<E> for FluidBackend<E> {
    fn start(&mut self, id: E, degree: i32, _pitch: Pitch, velocity: u8) {
        if let Some(location) = self.channel_and_note_for_degree(degree) {
            self.start_note(id, location, velocity);
        }
    }

    fn update(&mut self, id: E, degree: i32, _pitch: Pitch) {
        // TODO: Test that we only do something if we know the eventId
        if let Some(location) = self.channel_and_note_for_degree(degree) {
            self.update_note(id, location);
        }
    }

    fn stop(&mut self, id: E, velocity: u8) {
        self.stop_note(id, velocity);
    }

    fn update_program(&mut self, update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.send(FluidMessage::UpdateProgram { update_fn });
    }

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        self.set_tuning(tuning);
    }

    fn polyphonic_key_pressure(&mut self, id: E, pressure: u8) {
        if let Some(&(channel, note)) = self.keypress_tracker.location_of(&id) {
            self.send(FluidMessage::Polyphonic(
                ChannelMessageType::PolyphonicKeyPressure {
                    key: note,
                    pressure,
                }
                .in_channel(channel)
                .unwrap(),
            ));
        }
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.send(FluidMessage::Monophonic(
            ChannelMessageType::ControlChange { controller, value },
        ));
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.send(FluidMessage::Monophonic(
            ChannelMessageType::ChannelPressure { pressure },
        ));
    }
}

impl<E: Eq + Hash + Debug> FluidBackend<E> {
    fn channel_and_note_for_degree(&self, degree: i32) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(degree) {
            if let Some(key) = note.checked_midi_number() {
                return Some((channel.try_into().unwrap(), key));
            }
        }
        None
    }

    fn start_note(&mut self, id: E, location: (u8, u8), velocity: u8) {
        match self.keypress_tracker.place_finger_at(id, location) {
            Ok(PlaceAction::KeyPressed) | Ok(PlaceAction::KeyAlreadyPressed) => {
                self.send_note_on(location, velocity);
            }
            Err(id) => eprintln!(
                "[WARNING] location {:?} with ID {:?} released before pressed",
                location, id
            ),
        }
    }

    fn update_note(&mut self, id: E, location: (u8, u8)) {
        match self.keypress_tracker.move_finger_to(&id, location) {
            Ok((LiftAction::KeyReleased(released), _)) => {
                self.send_note_off(released, 100);
                self.send_note_on(location, 100);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                self.send_note_on(location, 100);
            }
            Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
            Err(IllegalState) => {
                // Occurs when mouse moved
            }
        }
    }

    fn stop_note(&mut self, id: E, velocity: u8) {
        match self.keypress_tracker.lift_finger(&id) {
            Ok(LiftAction::KeyReleased(location)) => self.send_note_off(location, velocity),
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {
                // Occurs when in waveform mode
            }
        }
    }

    fn send_note_on(&self, (channel, note): (u8, u8), velocity: u8) {
        self.send(FluidMessage::Polyphonic(
            ChannelMessageType::NoteOn {
                key: note,
                velocity,
            }
            .in_channel(channel)
            .unwrap(),
        ));
    }

    fn send_note_off(&self, (channel, note): (u8, u8), velocity: u8) {
        self.send(FluidMessage::Polyphonic(
            ChannelMessageType::NoteOff {
                key: note,
                velocity,
            }
            .in_channel(channel)
            .unwrap(),
        ));
    }

    fn set_tuning(&mut self, tuning: impl Scale) {
        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let (tuner, channel_tunings) = ChannelTuner::apply_full_keyboard_tuning(
            tuning.as_sorted_tuning().as_linear_mapping(),
            lowest_key..highest_key,
        );

        self.tuner = tuner;

        assert!(
            channel_tunings.len() <= 16,
            "Cannot apply tuning: There are too many notes in one semitone"
        );

        self.send(FluidMessage::Retune {
            channel_tunings: channel_tunings
                .iter()
                .map(FullKeyboardDetuning::to_fluid_format)
                .collect(),
        });
    }

    fn send(&self, message: FluidMessage) {
        self.messages.send(message).unwrap();
    }
}

pub struct FluidSynth {
    synth: Synth,
    messages: Receiver<FluidMessage>,
    program_updates: Sender<SelectedProgram>,
}

impl FluidSynth {
    pub fn write(&mut self, buffer: &mut [f32]) {
        for message in self.messages.try_iter() {
            self.process_message(message)
        }
        self.synth.write(&mut buffer[..]).unwrap();
    }

    fn process_message(&self, message: FluidMessage) {
        match message {
            FluidMessage::Polyphonic(channel_message) => self.process_message_type(
                channel_message.channel().into(),
                channel_message.message_type(),
            ),
            FluidMessage::Monophonic(message_type) => {
                for channel in 0..16 {
                    self.process_message_type(channel, message_type)
                }
                if let ChannelMessageType::ProgramChange { program } = message_type {
                    self.program_updates
                        .send(SelectedProgram {
                            program_number: program,
                            program_name: self
                                .synth
                                .get_channel_preset(0)
                                .and_then(|preset| preset.get_name().map(str::to_owned)),
                        })
                        .unwrap();
                }
            }
            FluidMessage::Retune { channel_tunings } => {
                for (channel, channel_tuning) in channel_tunings.iter().enumerate() {
                    let channel = channel.try_into().unwrap();
                    self.synth
                        .create_key_tuning(0, channel, "microwave-dynamic-tuning", &channel_tuning)
                        .unwrap();
                    self.synth
                        .activate_tuning(channel, 0, channel, true)
                        .unwrap();
                }
            }
            FluidMessage::UpdateProgram { mut update_fn } => {
                for channel in 0..16 {
                    let curr_program =
                        usize::try_from(self.synth.get_program(channel).unwrap().2).unwrap();
                    let next_program = u32::try_from(update_fn(curr_program + 128) % 128).unwrap();
                    self.synth.program_change(channel, next_program).unwrap();
                }
            }
        }
    }

    fn process_message_type(&self, channel: u32, message_type: ChannelMessageType) {
        match message_type {
            ChannelMessageType::NoteOff { key, .. } => {
                // When note_on is called for a note that is not supported by the current sound program FluidLite ignores that call.
                // When note_off is sent for the same note afterwards FluidLite reports an error since the note is considered off.
                // This error cannot be anticipated so we just ignore it.
                let _ = self.synth.note_off(channel, key.into());
            }
            ChannelMessageType::NoteOn { key, velocity } => self
                .synth
                .note_on(channel, key.into(), velocity.into())
                .unwrap(),
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => self
                .synth
                .key_pressure(channel, key.into(), pressure.into())
                .unwrap(),
            ChannelMessageType::ControlChange { controller, value } => {
                self.synth
                    .cc(channel, controller.into(), value.into())
                    .unwrap();
            }
            ChannelMessageType::ProgramChange { program } => {
                self.synth.program_change(channel, program.into()).unwrap();
            }
            ChannelMessageType::ChannelPressure { pressure } => {
                self.synth
                    .channel_pressure(channel, pressure.into())
                    .unwrap();
            }
            ChannelMessageType::PitchBendChange { value } => self
                .synth
                .pitch_bend(channel, (value + 8192) as u32)
                .unwrap(),
        }
    }
}

enum FluidMessage {
    Polyphonic(ChannelMessage),
    Monophonic(ChannelMessageType),
    Retune {
        channel_tunings: Vec<[f64; 128]>,
    },
    UpdateProgram {
        update_fn: Box<dyn FnMut(usize) -> usize + Send>,
    },
}
