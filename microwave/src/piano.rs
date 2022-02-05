use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{mpsc::Sender, Arc, Mutex, MutexGuard},
};

use tune::{
    midi::ChannelMessageType,
    pitch::{Pitch, Ratio},
    scala::{Kbm, KbmRoot, Scl},
    tuning::Tuning,
};
use tune_cli::shared::midi::MultiChannelOffset;

use crate::model::{Event, Location, SourceId};

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

/// A snapshot of the piano engine state to be used for screen rendering.
/// By rendering the snapshot version the engine remains responsive even at low screen refresh rates.
#[derive(Clone)]
pub struct PianoEngineSnapshot {
    pub curr_backend: usize,
    pub legato: bool,
    pub tuning_mode: TuningMode,
    pub kbm: Arc<Kbm>,
    pub pressed_keys: HashMap<SourceId, PressedKey>,
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
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            curr_backend: 0,
            legato: true,
            tuning_mode: TuningMode::Fixed,
            kbm: Arc::new(kbm),
            pressed_keys: HashMap::new(),
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            backends,
            scl,
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

    pub fn control_change(&self, controller: u8, value: f64) {
        self.lock_model()
            .control_change(controller, (value * 127.0).round() as u8)
    }

    pub fn toggle_legato(&self) {
        let mut model = self.lock_model();
        model.legato = !model.legato;
    }

    pub fn toggle_tuning_mode(&self) {
        let mut model = self.lock_model();
        model.tuning_mode.toggle();
        model.retune();
    }

    pub fn toggle_envelope_type(&self) {
        self.lock_model().backend_mut().toggle_envelope_type();
    }

    pub fn toggle_synth_mode(&self) {
        let mut model = self.lock_model();
        model.curr_backend += 1;
        model.curr_backend %= model.backends.len();
        model.backend_mut().send_status();
    }

    pub fn inc_program(&self) {
        self.lock_model()
            .backend_mut()
            .program_change(Box::new(|p| p + 1));
    }

    pub fn dec_program(&self) {
        self.lock_model()
            .backend_mut()
            .program_change(Box::new(|p| p - 1));
    }

    pub fn change_ref_note_by(&self, delta: i32) {
        let mut model = self.lock_model();
        let mut kbm_root = model.kbm.kbm_root();
        kbm_root.origin = kbm_root.origin.plus_steps(delta);
        kbm_root.ref_pitch = kbm_root.ref_pitch * Ratio::from_semitones(delta);
        Arc::make_mut(&mut model.kbm).set_kbm_root(kbm_root);
        model.retune();
    }

    pub fn change_root_offset_by(&self, delta: i32) {
        let mut model = self.lock_model();
        let mut kbm_root = model.kbm.kbm_root();
        kbm_root.ref_degree -= delta;
        Arc::make_mut(&mut model.kbm).set_kbm_root(kbm_root);
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
            // Handled by the engine.
            ChannelMessageType::NoteOff { key, velocity }
            | ChannelMessageType::NoteOn {
                key,
                velocity: velocity @ 0,
            } => {
                let piano_key = offset.get_piano_key(key);
                self.handle_event(Event::Released(SourceId::Midi(piano_key), velocity));
            }
            // Handled by the engine.
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
            // Forwarded to all synths.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                let piano_key = offset.get_piano_key(key);
                for backend in &mut self.backends {
                    backend.update_pressure(SourceId::Midi(piano_key), pressure);
                }
            }
            // Handled by the engine.
            ChannelMessageType::ControlChange { controller, value } => {
                self.control_change(controller, value);
            }
            // Handled by the engine.
            ChannelMessageType::ProgramChange { program } => {
                self.set_program(program);
            }
            // Forwarded to all synths.
            ChannelMessageType::ChannelPressure { pressure } => {
                for backend in &mut self.backends {
                    backend.channel_pressure(pressure);
                }
            }
            // Forwarded to all synths.
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
                if self.legato {
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

    fn control_change(&mut self, controller: u8, value: u8) {
        for backend in &mut self.backends {
            backend.control_change(controller, value);
        }
    }

    fn set_program(&mut self, program: u8) {
        self.backend_mut()
            .program_change(Box::new(move |_| usize::from(program)))
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
    }
}

pub trait Backend<S>: Send {
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot));

    fn set_no_tuning(&mut self);

    fn send_status(&self);

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
        &mut *self.backends[curr_backend]
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

    fn send_status(&self) {
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
