use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use flume::Sender;
use tune::key::PianoKey;
use tune::midi::ChannelMessageType;
use tune::pitch::Pitch;
use tune::tuning::Tuning;
use tune_cli::shared::midi::MultiChannelOffset;

use crate::backend::Backends;
use crate::backend::BankSelect;
use crate::backend::DynBackend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::control::LiveParameter;
use crate::control::LiveParameterMapper;
use crate::control::LiveParameterStorage;
use crate::control::ParameterValue;
use crate::lumatone::LumatoneLayout;
use crate::toggle::Toggle;
use crate::tuning_layout::TuningLayout;

#[derive(Clone)]
pub struct PianoEngine {
    model: Arc<Mutex<PianoEngineModel>>,
}

#[derive(Clone)]
pub struct PianoEngineState {
    pub curr_tuning_layout: TuningLayout,
    pub scale_index: usize,
    pub num_scales: usize,
    pub tuning_mode: TuningMode,
    pub mapper: LiveParameterMapper,
    pub storage: LiveParameterStorage,
    pub pressed_keys: PressedKeys,
    pub keys_version: u64,
    pub layout_version: u64,
}

pub type PressedKeys = HashMap<(SourceId, usize), Option<KeyInfo>>;

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
        };
    }
}

#[derive(Clone, Debug)]
pub struct KeyInfo {
    pub pitch: Pitch,
}

struct PianoEngineModel {
    backends: Toggle<DynBackend<SourceId>>,
    storage_updates: Sender<LiveParameterStorage>,
    tuning_layouts: Toggle<TuningLayout>,
    lumatone_sender: Option<Sender<LumatoneLayout>>,
    tuning_mode: TuningMode,
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
            tuning_mode: TuningMode::Fixed,
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

    pub fn handle_midi_event(
        &self,
        message_type: ChannelMessageType,
        offset: MultiChannelOffset,
        lumatone_mode: bool,
    ) {
        self.lock_model()
            .handle_midi_event(message_type, offset, lumatone_mode);
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
        backend.request_status();
    }

    pub fn inc_backend(&self) {
        let mut model = self.lock_model();
        model.backends.inc();
        model.backend_mut().request_status();
    }

    pub fn dec_backend(&self) {
        let mut model = self.lock_model();
        model.backends.dec();
        model.backend_mut().request_status();
    }

    pub fn inc_tuning(&self) {
        let mut model = self.lock_model();
        model.tuning_layouts.inc();
        model.retune();
        model.send_lumatone_layout();
    }

    pub fn dec_tuning(&self) {
        let mut model = self.lock_model();
        model.tuning_layouts.dec();
        model.retune();
        model.send_lumatone_layout();
    }

    pub fn toggle_layout(&self) {
        let mut model = self.lock_model();
        model.tuning_layouts.curr_option_mut().layout.toggle_next();
        model.send_lumatone_layout();
    }

    pub fn toggle_compression(&self) {
        let mut model = self.lock_model();
        model
            .tuning_layouts
            .curr_option_mut()
            .compression
            .toggle_next();
        model.send_lumatone_layout();
    }

    pub fn toggle_scale(&self) {
        let mut model = self.lock_model();
        model.tuning_layouts.curr_option_mut().scale.toggle_next();
        model.send_lumatone_layout();
    }

    pub fn toggle_parameter(&self, parameter: LiveParameter) {
        self.lock_model().toggle_parameter(parameter);
    }

    pub fn bank_select(&self, bank_select: BankSelect) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.bank_select(bank_select);
        backend.request_status();
    }

    pub fn program_change(&self, program_change: ProgramChange) {
        let mut model = self.lock_model();
        let backend = model.backend_mut();
        backend.program_change(program_change);
        backend.request_status();
    }

    pub fn change_ref_note_by(&self, delta: i32) {
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

    pub fn change_root_offset_by(&self, delta: i32) {
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
            tuning_mode: model.tuning_mode,
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
    fn handle_midi_event(
        &mut self,
        message_type: ChannelMessageType,
        offset: MultiChannelOffset,
        lumatone_mode: bool,
    ) {
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
                let degree = match lumatone_mode {
                    false => self
                        .tuning_layouts
                        .curr_option()
                        .kbm
                        .scale_degree_of(piano_key),
                    true => Some(piano_key.midi_number()),
                };
                if let Some(degree) = degree {
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
                let curr_backend = self.backends.curr_index();
                for (backend_id, backend) in self.backends.into_iter().enumerate() {
                    let is_curr_backend = backend_id == curr_backend;
                    if backend.note_input() == NoteInput::Background || is_curr_backend {
                        backend.start(id, degree, pitch, velocity);
                        self.pressed_keys.insert(
                            (id, backend_id),
                            is_curr_backend.then_some(KeyInfo { pitch }),
                        );
                    }
                }
                self.keys_version += 1;
            }
            Event::Moved(id, location) => {
                if self.storage.is_active(LiveParameter::Legato) {
                    let (degree, pitch) = self.degree_and_pitch(location);
                    for (backend_id, backend) in self.backends.into_iter().enumerate() {
                        if let Some(key_info) = self.pressed_keys.get_mut(&(id, backend_id)) {
                            backend.update_pitch(id, degree, pitch, 100);
                            if let (true, Some(key_info)) = (backend.has_legato(), key_info) {
                                key_info.pitch = pitch;
                            }
                        }
                    }
                    self.keys_version += 1;
                }
            }
            Event::Released(id, velocity) => {
                for (backend_id, backend) in self.backends.into_iter().enumerate() {
                    backend.stop(id, velocity);
                    self.pressed_keys.remove(&(id, backend_id));
                }
                self.keys_version += 1;
            }
        }
    }

    fn degree_and_pitch(&self, location: Location) -> (i32, Pitch) {
        let tuning_layout = self.tuning_layouts.curr_option();
        let tuning = (&tuning_layout.scl, tuning_layout.kbm.kbm_root());
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
            match self.tuning_mode {
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
    pub fn backend_mut(&mut self) -> &mut DynBackend<SourceId> {
        self.backends.curr_option_mut()
    }
}
