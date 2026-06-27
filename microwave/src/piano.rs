use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use bevy::prelude::*;
use flume::Sender;
use tune::key::PianoKey;
use tune::midi::ChannelMessageType;
use tune::pitch::Pitch;
use tune::tuning::Tuning;

use crate::backend::Backends;
use crate::backend::DynBackend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::control::LiveParameter;
use crate::control::LiveParameterMapper;
use crate::control::LiveParameterStorage;
use crate::control::ParameterValue;
use crate::lumatone::LumatoneLayout;
use crate::toggle::Direction;
use crate::toggle::Toggle;
use crate::tuning_layout::TuningLayout;

#[derive(Clone, Resource)]
pub struct PianoEngine {
    model: Arc<Mutex<PianoEngineModel>>,
}

#[derive(Clone, Resource)]
pub struct PianoEngineState {
    pub curr_tuning_layout: TuningLayout,
    pub scale_index: usize,
    pub num_scales: usize,
    pub tuning_mode: Toggle<TuningMode>,
    pub mapper: LiveParameterMapper,
    pub storage: LiveParameterStorage,
    pub pressed_keys: PressedKeys,
    pub keys_version: u64,
    pub layout_version: u64,
}

pub type PressedKeys = HashMap<(SourceId, usize), Option<Pitch>>;

#[derive(Clone, Debug)]
pub enum TuningMode {
    Fixed,
    Continuous,
}

struct PianoEngineModel {
    backends: Toggle<DynBackend<SourceId>>,
    storage_updates: Sender<LiveParameterStorage>,
    tuning_layouts: Toggle<TuningLayout>,
    lumatone_sender: Option<Sender<LumatoneLayout>>,
    tuning_mode: Toggle<TuningMode>,
    mapper: LiveParameterMapper,
    storage: LiveParameterStorage,
    pressed_keys: PressedKeys,
    keys_version: u64,
    layout_version: u64,
}

impl PianoEngine {
    pub fn new(
        tuning_layouts: Toggle<TuningLayout>,
        backends: Backends<SourceId>,
        mapper: LiveParameterMapper,
        storage: LiveParameterStorage,
        storage_updates: Sender<LiveParameterStorage>,
        lumatone_sender: Option<Sender<LumatoneLayout>>,
    ) -> Self {
        let mut model = PianoEngineModel {
            backends: backends.into(),
            storage_updates,
            tuning_layouts,
            lumatone_sender,
            tuning_mode: vec![TuningMode::Fixed, TuningMode::Continuous].into(),
            storage,
            mapper,
            pressed_keys: HashMap::new(),
            keys_version: 0,
            layout_version: 0,
        };

        model.retune();
        model.send_lumatone_layout();

        Self {
            model: Arc::new(Mutex::new(model)),
        }
    }

    pub fn handle_midi(
        &self,
        message_type: ChannelMessageType,
        map_midi_key: impl Fn(u8) -> (SourceId, InputLocation),
    ) {
        self.lock_model().handle_midi(message_type, map_midi_key);
    }

    pub fn handle_input(&self, event: InputEvent) {
        self.lock_model().handle_input(event);
    }

    pub fn set_parameter(&self, parameter: LiveParameter, value: f64) {
        self.lock_model().set_parameter(parameter, value);
    }

    pub fn set_key_pressure(&self, id: SourceId, value: f64) {
        self.lock_model().set_key_pressure(id, value.as_u8());
    }

    pub fn switch_tuning_mode(&self, direction: Direction) {
        let mut model = self.lock_model();
        model.tuning_mode.switch(direction);
        model.retune();
    }

    pub fn switch_envelope_type(&self, direction: Direction) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.switch_envelope_type(direction);
        backend.request_status();
    }

    pub fn switch_backend(&self, direction: Direction) {
        let mut model = self.lock_model();
        model.backends.switch(direction);
        model.backend_mut().request_status();
    }

    pub fn switch_tuning(&self, direction: Direction) {
        let mut model = self.lock_model();
        model.tuning_layouts.switch(direction);
        model.retune();
        model.send_lumatone_layout();
    }

    pub fn switch_layout(&self, direction: Direction) {
        let mut model = self.lock_model();
        model
            .tuning_layouts
            .curr_option_mut()
            .layout
            .switch(direction);
        model.send_lumatone_layout();
    }

    pub fn switch_compression(&self, direction: Direction) {
        let mut model = self.lock_model();
        model
            .tuning_layouts
            .curr_option_mut()
            .compression
            .switch(direction);
        model.send_lumatone_layout();
    }

    pub fn switch_scale(&self, direction: Direction) {
        let mut model = self.lock_model();
        model
            .tuning_layouts
            .curr_option_mut()
            .scale
            .switch(direction);
        model.send_lumatone_layout();
    }

    pub fn toggle_parameter(&self, parameter: LiveParameter) {
        self.lock_model().toggle_parameter(parameter);
    }

    pub fn switch_bank(&self, direction: Direction) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.switch_bank(direction);
        backend.request_status();
    }

    pub fn switch_program(&self, direction: Direction) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.program_change(ProgramChange::Directional(direction));
        backend.request_status();
    }

    pub fn switch_ref_note(&self, direction: Direction) {
        let delta = match direction {
            Direction::Forward => 1,
            Direction::Backward => -1,
        };
        let mut model = self.lock_model();
        let mut kbm_root = model.tuning_layouts.curr_option().kbm.kbm_root();
        kbm_root = kbm_root.shift_ref_key_by(delta);
        model
            .tuning_layouts
            .curr_option_mut()
            .kbm
            .set_kbm_root(kbm_root);
        model.retune();
    }

    pub fn switch_root_offset(&self, direction: Direction) {
        let delta = match direction {
            Direction::Forward => 1,
            Direction::Backward => -1,
        };
        let mut model = self.lock_model();
        let mut kbm_root = model.tuning_layouts.curr_option().kbm.kbm_root();
        kbm_root.root_offset += delta;
        model
            .tuning_layouts
            .curr_option_mut()
            .kbm
            .set_kbm_root(kbm_root);
        model.retune();
    }

    pub fn capture_state(&self) -> PianoEngineState {
        let model = self.lock_model();
        PianoEngineState {
            curr_tuning_layout: model.tuning_layouts.curr_option().clone(),
            scale_index: model.tuning_layouts.curr_index(),
            num_scales: model.tuning_layouts.num_options(),
            tuning_mode: model.tuning_mode.clone(),
            mapper: model.mapper.clone(),
            storage: model.storage.clone(),
            pressed_keys: model.pressed_keys.clone(),
            keys_version: model.keys_version,
            layout_version: model.layout_version,
        }
    }

    /// Capture the state of the piano engine for screen rendering.
    /// By rendering the captured state the engine remains responsive even at low screen refresh rates.
    pub fn capture_state_into(&self, target: &mut PianoEngineState) {
        let model = self.lock_model();
        target
            .curr_tuning_layout
            .clone_from(model.tuning_layouts.curr_option());
        target
            .scale_index
            .clone_from(&model.tuning_layouts.curr_index());
        target
            .num_scales
            .clone_from(&model.tuning_layouts.num_options());
        target.tuning_mode.clone_from(&model.tuning_mode);
        target.mapper.clone_from(&model.mapper);
        target.storage.clone_from(&model.storage);
        target.pressed_keys.clone_from(&model.pressed_keys);
        target.keys_version.clone_from(&model.keys_version);
        target.layout_version.clone_from(&model.layout_version);
    }

    fn lock_model(&self) -> MutexGuard<'_, PianoEngineModel> {
        self.model.lock().unwrap()
    }
}

impl PianoEngineModel {
    fn handle_midi(
        &mut self,
        message_type: ChannelMessageType,
        map_midi_key: impl Fn(u8) -> (SourceId, InputLocation),
    ) {
        match message_type {
            // Forwarded to all backends.
            ChannelMessageType::NoteOff { key, velocity }
            | ChannelMessageType::NoteOn {
                key,
                velocity: velocity @ 0,
            } => {
                let (id, _) = map_midi_key(key);
                self.handle_input(InputEvent::Released(id, velocity));
            }
            // Forwarded to current backend.
            ChannelMessageType::NoteOn { key, velocity } => {
                let (id, location) = map_midi_key(key);
                self.handle_input(InputEvent::Pressed(id, location, velocity));
            }
            // Forwarded to all backends.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                let (id, _) = map_midi_key(key);
                self.set_key_pressure(id, pressure);
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

    fn handle_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::Pressed(id, location, velocity) => {
                if let Some((degree, pitch)) = self.degree_and_pitch(location) {
                    let curr_backend = self.backends.curr_index();
                    for (backend_id, backend) in self.backends.into_iter().enumerate() {
                        let is_curr_backend = backend_id == curr_backend;
                        if backend.note_input() == NoteInput::Background || is_curr_backend {
                            backend.start(id, degree, pitch, velocity);
                            self.pressed_keys
                                .insert((id, backend_id), is_curr_backend.then_some(pitch));
                            self.keys_version += 1;
                        }
                    }
                }
            }
            InputEvent::Moved(id, location) => {
                if self.storage.is_active(LiveParameter::Legato)
                    && let Some((degree, new_pitch)) = self.degree_and_pitch(location)
                {
                    for (backend_id, backend) in self.backends.into_iter().enumerate() {
                        if let Some(key_info) = self.pressed_keys.get_mut(&(id, backend_id)) {
                            backend.update_pitch(id, degree, new_pitch, 100);
                            if let (true, Some(pitch)) = (backend.has_legato(), key_info) {
                                *pitch = new_pitch;
                                self.keys_version += 1;
                            }
                        }
                    }
                }
            }
            InputEvent::Released(id, velocity) => {
                for (backend_id, backend) in self.backends.into_iter().enumerate() {
                    backend.stop(id, velocity);
                    self.pressed_keys.remove(&(id, backend_id));
                    self.keys_version += 1;
                }
            }
        }
    }

    fn degree_and_pitch(&self, location: InputLocation) -> Option<(i32, Pitch)> {
        let tuning_layout = self.tuning_layouts.curr_option();
        let tuning = (&tuning_layout.scl, tuning_layout.kbm.kbm_root());
        match location {
            InputLocation::Isomorphic(p, s) => {
                let degree = tuning_layout.get_key(p, s);
                Some((degree, tuning.pitch_of(degree)))
            }
            InputLocation::Piano(piano_key) => tuning_layout
                .kbm
                .scale_degree_of(piano_key)
                .map(|degree| (degree, tuning.pitch_of(degree))),
            InputLocation::Pitch(pitch) => {
                let degree = tuning.find_by_pitch(pitch).approx_value;
                match self.tuning_mode.curr_option() {
                    TuningMode::Continuous => Some((degree, pitch)),
                    TuningMode::Fixed => Some((degree, tuning.pitch_of(degree))),
                }
            }
        }
    }

    fn set_program(&mut self, program: u8) {
        let backend = &mut self.backend_mut();
        backend.program_change(ProgramChange::ProgramId(program));
        backend.request_status();
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
        self.storage_updates.send(self.storage.clone()).unwrap();
    }

    fn set_key_pressure(&mut self, id: SourceId, pressure: u8) {
        for backend in &mut self.backends {
            backend.update_pressure(id, pressure);
        }
    }

    fn pitch_bend(&mut self, value: i16) {
        self.storage
            .set_parameter(LiveParameter::PitchBend, f64::from(value) / 8192.0);
        self.storage_updates.send(self.storage.clone()).unwrap();
        for backend in &mut self.backends {
            backend.pitch_bend(value);
        }
    }

    fn retune(&mut self) {
        let scl = self.tuning_layouts.curr_option().scl.clone();
        let kbm_root = self.tuning_layouts.curr_option().kbm.kbm_root();
        for backend in &mut self.backends {
            match self.tuning_mode.curr_option() {
                TuningMode::Fixed => backend.set_tuning((&scl, kbm_root)),
                TuningMode::Continuous => backend.set_no_tuning(),
            }
        }
        self.backend_mut().request_status();
        self.layout_version += 1;
    }

    fn send_lumatone_layout(&mut self) {
        if let Some(lumatone_send) = &self.lumatone_sender {
            lumatone_send
                .send(LumatoneLayout::from_tuning_layout(
                    self.tuning_layouts.curr_option(),
                ))
                .unwrap();
        }
        self.layout_version += 1;
    }

    pub fn backend_mut(&mut self) -> &mut DynBackend<SourceId> {
        self.backends.curr_option_mut()
    }
}

pub enum InputEvent {
    Pressed(SourceId, InputLocation, u8),
    Moved(SourceId, InputLocation),
    Released(SourceId, u8),
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SourceId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Piano(u8, u8),
    Lumatone(u8, u8),
}

pub enum InputLocation {
    Pitch(Pitch),
    Isomorphic(i16, i16),
    Piano(PianoKey),
}
