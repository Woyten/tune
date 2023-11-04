use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, MutexGuard},
};

use crossbeam::channel::Sender;
use tune::{
    key::PianoKey,
    midi::ChannelMessageType,
    pitch::Pitch,
    scala::{Kbm, Scl},
    tuning::Tuning,
};
use tune_cli::shared::midi::MultiChannelOffset;

use crate::{
    backend::{Backend, Backends, NoteInput},
    control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue},
};

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

#[derive(Clone)]
pub struct PianoEngineState {
    pub curr_backend: usize,
    pub scl: Scl,
    pub kbm: Kbm,
    pub tuning_mode: TuningMode,
    pub mapper: LiveParameterMapper,
    pub storage: LiveParameterStorage,
    pub pressed_keys: HashMap<(SourceId, usize), PressedKey>,
    pub keys_updated: bool,
    pub tuning_updated: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum TuningMode {
    Fixed,
    Continuous,
}

impl TuningMode {
    fn toggle(&mut self) {
        *self = match *self {
            TuningMode::Fixed => TuningMode::Continuous,
            TuningMode::Continuous => TuningMode::Fixed,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PressedKey {
    pub pitch: Pitch,
    pub shadow: bool,
}

struct PianoEngineModel {
    state: PianoEngineState,
    backends: Backends<SourceId>,
    storage_updates: Sender<LiveParameterStorage>,
}

impl Deref for PianoEngineModel {
    type Target = PianoEngineState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for PianoEngineModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl PianoEngine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scl: Scl,
        kbm: Kbm,
        backends: Backends<SourceId>,
        program_number: u8,
        mapper: LiveParameterMapper,
        storage: LiveParameterStorage,
        storage_updates: Sender<LiveParameterStorage>,
    ) -> (Arc<Self>, PianoEngineState) {
        let state = PianoEngineState {
            curr_backend: 0,
            scl,
            kbm,
            tuning_mode: TuningMode::Fixed,
            storage,
            mapper,
            pressed_keys: HashMap::new(),
            keys_updated: false,
            tuning_updated: false,
        };

        let mut model = PianoEngineModel {
            state: state.clone(),
            backends,
            storage_updates,
        };

        model.retune();
        model.set_program(program_number);

        let engine = Self {
            model: Mutex::new(model),
        };

        (Arc::new(engine), state)
    }

    pub fn handle_midi_event(&self, message_type: ChannelMessageType, offset: MultiChannelOffset) {
        self.lock_model().handle_midi_event(message_type, offset);
    }

    pub fn handle_event(&self, event: Event) {
        self.lock_model().handle_event(event);
    }

    pub fn set_parameter(&self, parameter: LiveParameter, value: f64) {
        self.lock_model().set_parameter(parameter, value);
    }

    pub fn set_key_pressure(&self, id: SourceId, value: f64) {
        self.lock_model().set_key_pressure(id, value.as_u8());
    }

    pub fn toggle_tuning_mode(&self) {
        let mut model = self.lock_model();
        model.tuning_mode.toggle();
        model.retune();
    }

    pub fn toggle_envelope_type(&self) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.toggle_envelope_type();
        backend.send_status();
    }

    pub fn toggle_synth_mode(&self) {
        let mut model = self.lock_model();
        model.curr_backend += 1;
        model.curr_backend %= model.backends.len();
        model.backend_mut().send_status();
    }

    pub fn toggle_parameter(&self, parameter: LiveParameter) {
        self.lock_model().toggle_parameter(parameter);
    }

    pub fn inc_program(&self) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.program_change(Box::new(|p| p.saturating_add(1)));
        backend.send_status();
    }

    pub fn dec_program(&self) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.program_change(Box::new(|p| p.saturating_sub(1)));
        backend.send_status();
    }

    pub fn change_ref_note_by(&self, delta: i32) {
        let mut model = self.lock_model();
        let mut kbm_root = model.kbm.kbm_root();
        kbm_root = kbm_root.shift_ref_key_by(delta);
        model.kbm.set_kbm_root(kbm_root);
        model.retune();
    }

    pub fn change_root_offset_by(&self, delta: i32) {
        let mut model = self.lock_model();
        let mut kbm_root = model.kbm.kbm_root();
        kbm_root.root_offset += delta;
        model.kbm.set_kbm_root(kbm_root);
        model.retune();
    }

    /// Capture the state of the piano engine for screen rendering.
    /// By rendering the captured state the engine remains responsive even at low screen refresh rates.
    pub fn capture_state(&self, target: &mut PianoEngineState) {
        let mut model = self.lock_model();
        target.clone_from(&model);
        model.keys_updated = false;
        model.tuning_updated = false;
    }

    fn lock_model(&self) -> MutexGuard<PianoEngineModel> {
        self.model.lock().unwrap()
    }
}

impl PianoEngineModel {
    fn handle_midi_event(&mut self, message_type: ChannelMessageType, offset: MultiChannelOffset) {
        match message_type {
            // Forwarded to all backends.
            ChannelMessageType::NoteOff { key, velocity }
            | ChannelMessageType::NoteOn {
                key,
                velocity: velocity @ 0,
            } => {
                let piano_key = offset.get_piano_key(key);
                self.handle_event(Event::Released(SourceId::Midi(piano_key), velocity));
            }
            // Forwarded to current backend.
            ChannelMessageType::NoteOn { key, velocity } => {
                let piano_key = offset.get_piano_key(key);
                if let Some(degree) = self.kbm.scale_degree_of(piano_key) {
                    self.handle_event(Event::Pressed(
                        SourceId::Midi(piano_key),
                        Location::Degree(degree),
                        velocity,
                    ));
                }
            }
            // Forwarded to all backends.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                let piano_key = offset.get_piano_key(key);
                self.set_key_pressure(SourceId::Midi(piano_key), pressure);
            }
            // Forwarded to all backends.
            ChannelMessageType::ControlChange { controller, value } => {
                // Take a shortcut s.t. controller numbers are conserved
                for backend in &mut self.backends {
                    backend.control_change(controller, value);
                }
                for parameter in self.mapper.resolve_ccn(controller) {
                    self.set_parameter_without_backends_update(parameter, value.as_f64());
                }
            }
            // Forwarded to current backend.
            ChannelMessageType::ProgramChange { program } => {
                self.set_program(program);
            }
            // Forwarded to current backend.
            ChannelMessageType::ChannelPressure { pressure } => {
                self.set_parameter(LiveParameter::ChannelPressure, pressure);
            }
            // Forwarded to all backends
            ChannelMessageType::PitchBendChange { value } => self.pitch_bend(value),
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Pressed(id, location, velocity) => {
                let (degree, pitch) = self.degree_and_pitch(location);
                let curr_backend = self.curr_backend;
                for (backend_id, backend) in self.backends.iter_mut().enumerate() {
                    let is_curr_backend = backend_id == curr_backend;
                    if backend.note_input() == NoteInput::Background || is_curr_backend {
                        backend.start(id, degree, pitch, velocity);
                        self.state.pressed_keys.insert(
                            (id, backend_id),
                            PressedKey {
                                pitch,
                                shadow: !is_curr_backend,
                            },
                        );
                        self.state.keys_updated = true;
                    }
                }
            }
            Event::Moved(id, location) => {
                if self.storage.is_active(LiveParameter::Legato) {
                    let (degree, pitch) = self.degree_and_pitch(location);
                    for (backend_id, backend) in self.backends.iter_mut().enumerate() {
                        if let Some(pressed_key) =
                            self.state.pressed_keys.get_mut(&(id, backend_id))
                        {
                            backend.update_pitch(id, degree, pitch, 100);
                            if backend.has_legato() {
                                pressed_key.pitch = pitch;
                            }
                            self.state.keys_updated = true;
                        }
                    }
                }
            }
            Event::Released(id, velocity) => {
                for (backend_id, backend) in self.backends.iter_mut().enumerate() {
                    backend.stop(id, velocity);
                    self.state.pressed_keys.remove(&(id, backend_id));
                    self.state.keys_updated = true;
                }
            }
        }
    }

    fn degree_and_pitch(&self, location: Location) -> (i32, Pitch) {
        let tuning = (&self.scl, self.kbm.kbm_root());
        match location {
            Location::Pitch(pitch) => {
                let degree = tuning.find_by_pitch(pitch).approx_value;

                match self.tuning_mode {
                    TuningMode::Continuous => (degree, pitch),
                    TuningMode::Fixed => (degree, tuning.pitch_of(degree)),
                }
            }
            Location::Degree(degree) => (degree, tuning.pitch_of(degree)),
        }
    }

    fn set_program(&mut self, program: u8) {
        let backend = &mut self.backend_mut();
        backend.program_change(Box::new(move |_| usize::from(program)));
        backend.send_status();
    }

    fn toggle_parameter(&mut self, parameter: LiveParameter) {
        if self.storage.is_active(parameter) {
            self.set_parameter(parameter, 0.0);
        } else {
            self.set_parameter(parameter, 1.0);
        }
    }

    fn set_parameter(&mut self, parameter: LiveParameter, value: impl ParameterValue) {
        self.set_parameter_without_backends_update(parameter, value.as_f64());
        let value = value.as_u8();
        match parameter {
            LiveParameter::ChannelPressure => {
                for backend in &mut self.backends {
                    backend.channel_pressure(value);
                }
            }
            _ => {
                if let Some(ccn) = self.mapper.get_ccn(parameter) {
                    for backend in &mut self.backends {
                        backend.control_change(ccn, value);
                    }
                }
            }
        }
    }

    fn set_parameter_without_backends_update(&mut self, parameter: LiveParameter, value: f64) {
        self.storage.set_parameter(parameter, value);
        self.storage_updates.send(self.storage).unwrap();
    }

    fn set_key_pressure(&mut self, id: SourceId, pressure: u8) {
        for backend in &mut self.backends {
            backend.update_pressure(id, pressure);
        }
    }

    fn pitch_bend(&mut self, value: i16) {
        self.storage
            .set_parameter(LiveParameter::PitchBend, f64::from(value) / 8192.0);
        self.storage_updates.send(self.storage).unwrap();
        for backend in &mut self.backends {
            backend.pitch_bend(value);
        }
    }

    fn retune(&mut self) {
        for backend in &mut self.backends {
            match self.state.tuning_mode {
                TuningMode::Fixed => {
                    backend.set_tuning((&self.state.scl, self.state.kbm.kbm_root()))
                }
                TuningMode::Continuous => backend.set_no_tuning(),
            }
            self.state.tuning_updated = true;
        }
        self.backend_mut().send_status();
    }
}

pub enum Event {
    Pressed(SourceId, Location, u8),
    Moved(SourceId, Location),
    Released(SourceId, u8),
}

pub enum Location {
    Pitch(Pitch),
    Degree(i32),
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SourceId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Midi(PianoKey),
}

impl PianoEngineModel {
    pub fn backend_mut(&mut self) -> &mut dyn Backend<SourceId> {
        let curr_backend = self.curr_backend;
        self.backends[curr_backend].as_mut()
    }
}
