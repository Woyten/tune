use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{mpsc::Sender, Arc, Mutex, MutexGuard},
};

use tune::{
    midi::ChannelMessageType,
    pitch::Pitch,
    scala::{Kbm, KbmRoot, Scl},
    tuning::Tuning,
};
use tune_cli::shared::midi::MultiChannelOffset;

use crate::{
    control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue},
    model::{Event, Location, SourceId},
};

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

/// A snapshot of the piano engine state to be used for screen rendering.
/// By rendering the snapshot version the engine remains responsive even at low screen refresh rates.
#[derive(Clone)]
pub struct PianoEngineSnapshot {
    pub curr_backend: usize,
    pub tuning_mode: TuningMode,
    pub kbm: Kbm,
    pub pressed_keys: HashMap<SourceId, PressedKey>,
    pub mapper: LiveParameterMapper,
    pub storage: LiveParameterStorage,
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
    pub backend: usize,
    pub pitch: Pitch,
}

struct PianoEngineModel {
    snapshot: PianoEngineSnapshot,
    backends: Vec<Box<dyn Backend<SourceId>>>,
    scl: Scl,
    storage_updates: Sender<LiveParameterStorage>,
}

impl Deref for PianoEngineModel {
    type Target = PianoEngineSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

impl DerefMut for PianoEngineModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.snapshot
    }
}

impl PianoEngine {
    pub fn new(
        scl: Scl,
        kbm: Kbm,
        backends: Vec<Box<dyn Backend<SourceId>>>,
        program_number: u8,
        mapper: LiveParameterMapper,
        storage: LiveParameterStorage,
        storage_updates: Sender<LiveParameterStorage>,
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            curr_backend: 0,
            tuning_mode: TuningMode::Fixed,
            kbm,
            pressed_keys: HashMap::new(),
            storage,
            mapper,
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            backends,
            scl,
            storage_updates,
        };

        model.retune();
        model.set_program(program_number);

        let engine = Self {
            model: Mutex::new(model),
        };

        (Arc::new(engine), snapshot)
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
        let backend = &mut model.backend_mut();
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
        let backend = &mut model.backend_mut();
        backend.program_change(Box::new(|p| p.saturating_add(1)));
        backend.send_status();
    }

    pub fn dec_program(&self) {
        let mut model = self.lock_model();
        let backend = &mut model.backend_mut();
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

    pub fn take_snapshot(&self, target: &mut PianoEngineSnapshot) {
        target.clone_from(&self.lock_model())
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
            ChannelMessageType::PitchBendChange { value } => {
                for backend in &mut self.backends {
                    backend.pitch_bend(value);
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Pressed(id, location, velocity) => {
                let (degree, pitch) = self.degree_and_pitch(location);
                self.backend_mut().start(id, degree, pitch, velocity);
                let backend = self.curr_backend;
                self.pressed_keys.insert(id, PressedKey { backend, pitch });
            }
            Event::Moved(id, location) => {
                if self.storage.is_active(LiveParameter::Legato) {
                    let (degree, pitch) = self.degree_and_pitch(location);
                    let (pressed_keys, backends) =
                        (&mut self.snapshot.pressed_keys, &mut self.backends);
                    if let Some(pressed_key) = pressed_keys.get_mut(&id) {
                        let backend = &mut backends[pressed_key.backend];
                        backend.update_pitch(id, degree, pitch, 100);
                        if backend.has_legato() {
                            pressed_key.pitch = pitch;
                        }
                    }
                }
            }
            Event::Released(id, velocity) => {
                for backend in &mut self.backends {
                    backend.stop(id, velocity);
                }
                self.pressed_keys.remove(&id);
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

    fn retune(&mut self) {
        let kbm_root = self.kbm.kbm_root();
        let tuning_mode = self.tuning_mode;

        for backend in &mut self.backends {
            match tuning_mode {
                TuningMode::Fixed => backend.set_tuning((&self.scl, kbm_root)),
                TuningMode::Continuous => backend.set_no_tuning(),
            }
        }
        self.backend_mut().send_status();
    }
}

pub trait Backend<S>: Send {
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

impl PianoEngineModel {
    pub fn backend_mut(&mut self) -> &mut dyn Backend<SourceId> {
        let curr_backend = self.curr_backend;
        self.backends[curr_backend].as_mut()
    }
}

pub struct NoAudio<I> {
    info_sender: Sender<I>,
}

impl<I> NoAudio<I> {
    pub fn new(info_sender: Sender<I>) -> Self {
        Self { info_sender }
    }
}

impl<E, I: From<()> + Send> Backend<E> for NoAudio<I> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        self.info_sender.send(().into()).unwrap();
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
