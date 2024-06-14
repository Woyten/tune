use flume::Sender;
use serde::{Deserialize, Serialize};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};

pub type DynBackend<S> = Box<dyn Backend<S>>;
pub type Backends<S> = Vec<DynBackend<S>>;

pub trait Backend<S>: Send {
    fn note_input(&self) -> NoteInput;

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot));

    fn set_no_tuning(&mut self);

    fn send_status(&mut self);

    fn start(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8);

    fn update_pitch(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8);

    fn update_pressure(&mut self, id: S, pressure: u8);

    fn stop(&mut self, id: S, velocity: u8);

    fn program_change(&mut self, update_fn: Box<dyn FnMut(usize) -> usize + Send>);

    fn control_change(&mut self, controller: u8, value: u8);

    fn channel_pressure(&mut self, pressure: u8);

    fn pitch_bend(&mut self, value: i16);

    fn toggle_envelope_type(&mut self);

    fn has_legato(&self) -> bool;
}

pub struct IdleBackend<I, M> {
    info_updates: Sender<I>,
    message: M,
}

impl<I, M> IdleBackend<I, M> {
    pub fn new(info_updates: &Sender<I>, message: M) -> Self {
        Self {
            info_updates: info_updates.clone(),
            message,
        }
    }
}

impl<E, I: From<M> + Send, M: Send + Clone> Backend<E> for IdleBackend<I, M> {
    fn note_input(&self) -> NoteInput {
        NoteInput::Foreground
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        self.info_updates.send(self.message.clone().into()).unwrap();
    }

    fn start(&mut self, _id: E, _degree: i32, _pitch: Pitch, _velocity: u8) {}

    fn update_pitch(&mut self, _id: E, _degree: i32, _pitch: Pitch, _velocity: u8) {}

    fn update_pressure(&mut self, _id: E, _pressure: u8) {}

    fn stop(&mut self, _id: E, _velocity: u8) {}

    fn program_change(&mut self, _update_fn: Box<dyn FnMut(usize) -> usize + Send>) {}

    fn control_change(&mut self, _controller: u8, _value: u8) {}

    fn channel_pressure(&mut self, _pressure: u8) {}

    fn pitch_bend(&mut self, _value: i16) {}

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        true
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum NoteInput {
    Foreground,
    Background,
}
