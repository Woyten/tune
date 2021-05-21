use std::{convert::TryFrom, fmt::Debug, hash::Hash, ops::Range};

use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    note::Note,
    pitch::Pitched,
    tuner::ChannelTuner,
    tuning::{LinearMapping, Scale, SortedTuning, Tuning},
};

use crate::keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction};

pub struct MidiBackendHelper<'a, E, S> {
    tuner: &'a mut ChannelTuner<i32>,
    keypress_tracker: &'a mut KeypressTracker<E, (u8, u8)>,
    sender: S,
}

impl<'a, E: Eq + Hash + Debug, S: PolyphonicSender> MidiBackendHelper<'a, E, S> {
    pub fn new(
        tuner: &'a mut ChannelTuner<i32>,
        keypress_tracker: &'a mut KeypressTracker<E, (u8, u8)>,
        sender: S,
    ) -> Self {
        Self {
            tuner,
            keypress_tracker,
            sender,
        }
    }

    pub fn set_tuning<T: Scale, D>(
        &mut self,
        tuning: T,
        apply_tuning: impl Fn(LinearMapping<SortedTuning<T>>, Range<i32>) -> (ChannelTuner<i32>, Vec<D>),
    ) -> Vec<D> {
        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let (tuner, channel_tunings) = apply_tuning(
            tuning.as_sorted_tuning().as_linear_mapping(),
            lowest_key..highest_key,
        );

        if channel_tunings.len() > 16 {
            println!("[WARNING] Cannot apply tuning. More than 16 channels are required.");
            return vec![];
        }

        *self.tuner = tuner;

        channel_tunings
    }

    pub fn start(&mut self, id: E, degree: i32, velocity: u8) {
        if let Some(location) = self.channel_and_note_for_degree(degree) {
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
    }

    pub fn update(&mut self, id: E, degree: i32) {
        if let Some(location) = self.channel_and_note_for_degree(degree) {
            match self.keypress_tracker.move_finger_to(&id, location) {
                Ok((LiftAction::KeyReleased(released), _)) => {
                    self.send_note_off(released, 100);
                    self.send_note_on(location, 100);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    self.send_note_on(location, 100);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {}
            }
        }
    }

    pub fn update_pressure(&mut self, id: E, pressure: u8) {
        if let Some(&(channel, note)) = self.keypress_tracker.location_of(&id) {
            self.sender.send(
                ChannelMessageType::PolyphonicKeyPressure {
                    key: note,
                    pressure,
                }
                .in_channel(channel)
                .unwrap(),
            )
        }
    }

    pub fn stop(&mut self, id: E, velocity: u8) {
        match self.keypress_tracker.lift_finger(&id) {
            Ok(LiftAction::KeyReleased(location)) => self.send_note_off(location, velocity),
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {}
        }
    }

    fn channel_and_note_for_degree(&self, degree: i32) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(degree) {
            if let Some(key) = note.checked_midi_number() {
                return Some((u8::try_from(channel).unwrap(), key));
            }
        }
        None
    }

    fn send_note_on(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.sender.send(
            ChannelMessageType::NoteOn {
                key: note,
                velocity,
            }
            .in_channel(channel)
            .unwrap(),
        );
    }

    fn send_note_off(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.sender.send(
            ChannelMessageType::NoteOff {
                key: note,
                velocity,
            }
            .in_channel(channel)
            .unwrap(),
        );
    }
}

pub trait PolyphonicSender {
    fn send(&mut self, message: ChannelMessage);
}
