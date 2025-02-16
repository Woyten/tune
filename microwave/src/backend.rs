use flume::Sender;
use serde::Deserialize;
use serde::Serialize;
use tune::pitch::Pitch;
use tune::scala::KbmRoot;
use tune::scala::Scl;

pub type DynBackend<S> = Box<dyn Backend<S>>;
pub type Backends<S> = Vec<DynBackend<S>>;

// A music backend generic over a key identifier `K`.
pub trait Backend<K>: Send {
    fn note_input(&self) -> NoteInput;

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot));

    fn set_no_tuning(&mut self);

    fn request_status(&mut self);

    fn start(&mut self, key_id: K, degree: i32, pitch: Pitch, velocity: u8);

    fn update_pitch(&mut self, key_id: K, degree: i32, pitch: Pitch, velocity: u8);

    fn update_pressure(&mut self, key_id: K, pressure: u8);

    fn stop(&mut self, key_id: K, velocity: u8);

    fn bank_select(&mut self, bank_select: BankSelect);

    fn program_change(&mut self, program_change: ProgramChange);

    fn control_change(&mut self, controller: u8, value: u8);

    fn channel_pressure(&mut self, pressure: u8);

    fn pitch_bend(&mut self, value: i16);

    fn toggle_envelope_type(&mut self);

    fn has_legato(&self) -> bool;
}

pub enum BankSelect {
    Inc,
    Dec,
}

pub enum ProgramChange {
    /// Use `u8` since this variant is only used in MIDI-to-MIDI communication for now.
    ProgramId(u8),
    Inc,
    Dec,
}

/// A backend that does nothing and always responds with a constant message.
pub struct IdleBackend<E, M> {
    events: Sender<E>,
    message: M,
}

impl<E, M> IdleBackend<E, M> {
    pub fn new(events: &Sender<E>, message: M) -> Self {
        Self {
            events: events.clone(),
            message,
        }
    }
}

impl<K, E: From<M> + Send, M: Send + Clone> Backend<K> for IdleBackend<E, M> {
    fn note_input(&self) -> NoteInput {
        NoteInput::Foreground
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn request_status(&mut self) {
        self.events.send(self.message.clone().into()).unwrap();
    }

    fn start(&mut self, _key_id: K, _degree: i32, _pitch: Pitch, _velocity: u8) {}

    fn update_pitch(&mut self, _key_id: K, _degree: i32, _pitch: Pitch, _velocity: u8) {}

    fn update_pressure(&mut self, _key_id: K, _pressure: u8) {}

    fn stop(&mut self, _key_id: K, _velocity: u8) {}

    fn bank_select(&mut self, _bank_select: BankSelect) {}

    fn program_change(&mut self, _program_change: ProgramChange) {}

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
