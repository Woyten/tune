use crate::{
    audio::AudioModel,
    piano::{PianoEngine, PianoEngineSnapshot},
};
use midir::MidiInputConnection;
use std::{
    collections::HashSet,
    ops::Deref,
    sync::{mpsc::Receiver, Arc},
};
use tune::{
    key::Keyboard,
    note::NoteLetter,
    pitch::{Pitch, Pitched},
};

pub struct Model {
    audio: AudioModel<EventId>,
    recording_active: bool,
    pub engine: Arc<PianoEngine>,
    engine_snapshot: PianoEngineSnapshot,
    keyboard: Keyboard,
    pub limit: u16,
    #[allow(dead_code)] // Not dead. Field keeps MIDI connection alive.
    midi_in: Option<MidiInputConnection<()>>,
    pub lowest_note: Pitch,
    pub highest_note: Pitch,
    pressed_physical_keys: HashSet<(i8, i8)>,
    pub selected_program: SelectedProgram,
    program_updates: Receiver<SelectedProgram>,
}

pub struct SelectedProgram {
    pub program_number: u8,
    pub program_name: Option<String>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum EventId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Midi(u8),
}

pub enum EventPhase {
    Pressed(u8),
    Moved,
    Released,
}

impl Model {
    pub(crate) fn new(
        audio: AudioModel<EventId>,
        engine: Arc<PianoEngine>,
        engine_snapshot: PianoEngineSnapshot,
        keyboard: Keyboard,
        limit: u16,
        midi_in: Option<MidiInputConnection<()>>,
        program_updates: Receiver<SelectedProgram>,
    ) -> Self {
        let lowest_note = NoteLetter::Fsh.in_octave(2).pitch();
        let highest_note = NoteLetter::Ash.in_octave(5).pitch();
        Self {
            audio,
            recording_active: false,
            engine,
            engine_snapshot,
            keyboard,
            limit,
            midi_in,
            lowest_note,
            highest_note,
            pressed_physical_keys: HashSet::new(),
            selected_program: SelectedProgram {
                program_number: 0,
                program_name: None,
            },
            program_updates,
        }
    }

    pub fn recording_active(&self) -> bool {
        self.recording_active
    }

    pub fn update(&mut self) {
        for update in self.program_updates.try_iter() {
            self.selected_program = update
        }
        self.engine.take_snapshot(&mut self.engine_snapshot);
    }

    pub fn keyboard_event(&mut self, (x, y): (i8, i8), pressed: bool) {
        let key_number = self.keyboard.get_key(x.into(), y.into()).midi_number();

        let (phase, net_change) = if pressed {
            (
                EventPhase::Pressed(100),
                self.pressed_physical_keys.insert((x, y)),
            )
        } else {
            (
                EventPhase::Released,
                self.pressed_physical_keys.remove(&(x, y)),
            )
        };

        // While a key is held down the pressed event is sent repeatedly. We ignore this case by checking net_change
        if net_change {
            self.engine
                .handle_key_offset_event(EventId::Keyboard(x, y), key_number, phase);
        }
    }

    pub fn toggle_recording(&mut self) {
        self.recording_active = !self.recording_active;
        if self.recording_active {
            self.audio.start_recording();
        } else {
            self.audio.stop_recording();
        }
    }
}

impl Deref for Model {
    type Target = PianoEngineSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.engine_snapshot
    }
}
