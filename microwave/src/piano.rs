#![allow(clippy::too_many_arguments)] // Valid lint but the error popped up too late s.t. this will be fixed in the future.

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex, MutexGuard},
};

use tune::{
    key::PianoKey,
    midi::ChannelMessageType,
    pitch::{Pitch, Ratio},
    scala::{Kbm, KbmRoot, Scl},
    tuning::Tuning,
};

use crate::{
    model::{EventId, EventPhase},
    synth::SynthControl,
};

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

/// A snapshot of the piano engine state to be used for screen rendering.
/// By rendering the snapshotted version the engine remains responsive even at low screen refresh rates.
#[derive(Clone)]
pub struct PianoEngineSnapshot {
    pub curr_backend: usize,
    pub legato: bool,
    pub continuous: bool,
    pub scl: Arc<Scl>,
    pub kbm: Arc<Kbm>,
    pub pressed_keys: HashMap<EventId, VirtualKey>, // TODO: Not required in model
}

#[derive(Clone, Debug)]
pub struct VirtualKey {
    pub pitch: Pitch,
}

struct PianoEngineModel {
    snapshot: PianoEngineSnapshot,
    backends: Vec<Box<dyn Backend<EventId>>>,
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
        backends: Vec<Box<dyn Backend<EventId>>>,
        program_number: u8,
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            curr_backend: 0,
            legato: true,
            continuous: false,
            scl: Arc::new(scl),
            kbm: Arc::new(kbm),
            pressed_keys: HashMap::new(),
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            backends,
        };

        model.set_program(program_number);
        model.retune();

        let engine = Self {
            model: Mutex::new(model),
        };

        (Arc::new(engine), snapshot)
    }

    pub fn handle_key_event(&self, id: EventId, offset: i32, phase: EventPhase) {
        self.lock_model().handle_key_event(id, offset, phase);
    }

    pub fn handle_pitch_event(&self, id: EventId, pitch: Pitch, phase: EventPhase) {
        self.lock_model().handle_pitch_event(id, pitch, phase);
    }

    pub fn handle_midi_event(&self, message_type: ChannelMessageType) {
        self.lock_model().handle_midi_event(message_type);
    }

    pub fn control(&self, control: SynthControl, value: f64) {
        // TODO: Send MouseY control change (use configurable mapping to MIDI)
    }

    pub fn toggle_legato(&self) {
        let mut model = self.lock_model();
        model.legato = !model.legato;
    }

    pub fn toggle_continuous(&self) {
        let mut model = self.lock_model();
        model.continuous = !model.continuous;
    }

    pub fn toggle_envelope_type(&self) {
        /*
        if let SynthMode::Waveform { envelope_type, .. } = self.lock_model().backend_mut() {
            *envelope_type = match *envelope_type {
                None => Some(EnvelopeType::Organ),
                Some(EnvelopeType::Organ) => Some(EnvelopeType::Piano),
                Some(EnvelopeType::Piano) => Some(EnvelopeType::Pad),
                Some(EnvelopeType::Pad) => Some(EnvelopeType::Bell),
                Some(EnvelopeType::Bell) => None,
            }
        }
        */
    }

    pub fn toggle_synth_mode(&self) {
        let mut model = self.lock_model();
        model.curr_backend += 1;
        model.curr_backend %= model.backends.len();
    }

    pub fn inc_program(&self) {
        self.lock_model()
            .backend_mut()
            .update_program(Box::new(|p| p + 1));
    }

    pub fn dec_program(&self) {
        self.lock_model()
            .backend_mut()
            .update_program(Box::new(|p| p - 1));
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
    fn handle_pitch_event(&mut self, id: EventId, mut pitch: Pitch, phase: EventPhase) {
        let tuning = self.tuning();
        let degree = tuning.find_by_pitch(pitch).approx_value;

        if !self.continuous {
            pitch = self.tuning().pitch_of(degree);
        }

        self.handle_event(id, degree, pitch, phase)
    }

    fn handle_midi_event(&mut self, message_type: ChannelMessageType) {
        match message_type {
            // Handled by the engine.
            ChannelMessageType::NoteOff { key, velocity } => {
                if let Some(degree) = self.kbm.scale_degree_of(PianoKey::from_midi_number(key)) {
                    self.handle_key_event(
                        EventId::Midi(key),
                        degree,
                        EventPhase::Released(velocity),
                    );
                }
            }
            // Handled by the engine.
            ChannelMessageType::NoteOn { key, velocity } => {
                // TODO: This should be called in every case. Should EventPhase::Pressed contain the degree?
                if let Some(degree) = self.kbm.scale_degree_of(PianoKey::from_midi_number(key)) {
                    self.handle_key_event(
                        EventId::Midi(key),
                        degree,
                        EventPhase::Pressed(velocity),
                    );
                }
            }
            // Forwarded to all synths.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                for backend in &mut self.backends {
                    backend.polyphonic_key_pressure(EventId::Midi(key), pressure);
                }
            }
            // Forwarded to all synths.
            ChannelMessageType::ControlChange { controller, value } => {
                for backend in &mut self.backends {
                    backend.control_change(controller, value);
                }
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
            // Forwarded to all channels of all synths.
            ChannelMessageType::PitchBendChange { value } => {
                /*
                self.waveform_control.pitch_bend(value);
                FluidMessage::Monophonic(message_type)
                */
            }
        }

        /*
        self.fluid_control.send(fluid_message.clone());
        if let Some(midi_out) = &mut self.midi_out {
            match fluid_message {
                FluidMessage::Polyphonic(message) => {
                    midi_out.send(&message.to_raw_message()).unwrap()
                }
                FluidMessage::Monophonic(message_type) => {
                    for channel in 0..16 {
                        midi_out
                            .send(&message_type.in_channel(channel).unwrap().to_raw_message())
                            .unwrap()
                    }
                }
                FluidMessage::Retune { .. } => unreachable!(),
            }
        }
        */
    }

    fn handle_key_event(&mut self, id: EventId, degree: i32, phase: EventPhase) {
        self.handle_event(id, degree, self.tuning().pitch_of(degree), phase);
    }

    fn handle_event(&mut self, id: EventId, degree: i32, pitch: Pitch, phase: EventPhase) {
        match phase {
            EventPhase::Pressed(velocity) => {
                self.backend_mut().start(id, degree, pitch, velocity);
                self.pressed_keys.insert(id, VirtualKey { pitch });
            }
            EventPhase::Moved => {
                if self.legato {
                    for backend in &mut self.backends {
                        // TODO: Velocity?
                        backend.update(id, degree, pitch);
                    }
                    if let Some(pressed_key) = self.pressed_keys.get_mut(&id) {
                        pressed_key.pitch = pitch;
                    }
                }
            }
            EventPhase::Released(velocity) => {
                for backend in &mut self.backends {
                    backend.stop(id, velocity);
                }
                self.pressed_keys.remove(&id);
            }
        }
    }

    fn set_program(&mut self, program: u8) {
        self.backend_mut()
            .update_program(Box::new(move |_| usize::from(program)))
    }

    fn retune(&mut self) {
        for backend in &mut self.backends {
            backend.set_tuning(self.snapshot.tuning());
        }
    }
}

pub trait Backend<E>: Send {
    fn start(&mut self, id: E, degree: i32, pitch: Pitch, velocity: u8);

    fn update(&mut self, id: E, degree: i32, pitch: Pitch);

    fn stop(&mut self, id: E, velocity: u8);

    fn update_program(&mut self, update_fn: Box<dyn FnMut(usize) -> usize + Send>);

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot));

    fn polyphonic_key_pressure(&mut self, id: E, pressure: u8);

    fn control_change(&mut self, controller: u8, value: u8);

    fn channel_pressure(&mut self, pressure: u8);
}

impl PianoEngineModel {
    pub fn backend_mut(&mut self) -> &mut dyn Backend<EventId> {
        let curr_backend = self.curr_backend;
        &mut *self.backends[curr_backend]
    }
}

impl PianoEngineSnapshot {
    pub fn tuning(&self) -> (&Scl, KbmRoot) {
        (&self.scl, self.kbm.kbm_root())
    }
}
