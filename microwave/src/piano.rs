use crate::{
    fluid::{FluidGlobalMessage, FluidMessage, FluidPolyphonicMessage},
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    model::{EventId, EventPhase},
    synth::{WaveformLifecycle, WaveformMessage},
    wave::{self, Patch},
};
use std::{
    collections::HashMap,
    convert::TryInto,
    ops::{Deref, DerefMut},
    sync::{mpsc::Sender, Arc, Mutex, MutexGuard},
};
use tune::{
    key::PianoKey,
    midi::ChannelMessageType,
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    scala::{Kbm, Scl},
    tuner::{ChannelTuner, ChannelTuning},
    tuning::Tuning,
};
use wave::EnvelopeType;

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

/// A snapshot of the piano engine state to be used for screen rendering.
/// By rendering the snapshotted version the engine remains responsive even at low screen refresh rates.
#[derive(Clone)]
pub struct PianoEngineSnapshot {
    pub synth_mode: SynthMode,
    pub continuous: bool,
    pub legato: bool,
    pub scale: Arc<Scl>,
    pub root_note: Note,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
    pub waveform_number: usize,
    pub waveforms: Arc<Vec<Patch>>, // Arc used here in order to prevent cloning of the inner Vec
    pub envelope_type: Option<EnvelopeType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SynthMode {
    OnlyWaveform,
    Waveform,
    Fluid,
}

#[derive(Clone, Debug)]
pub struct VirtualKey {
    pub pitch: Pitch,
    synth_type: SynthType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SynthType {
    Waveform,
    Fluid,
}

struct PianoEngineModel {
    snapshot: PianoEngineSnapshot,
    keypress_tracker: KeypressTracker<EventId, (u8, u8)>,
    channel_tuner: ChannelTuner,
    fluid_messages: std::sync::mpsc::Sender<FluidMessage>,
    waveform_messages: Sender<WaveformMessage<EventId>>,
    damper_controller: u8,
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
        synth_mode: SynthMode,
        scale: Scl,
        program_number: u8,
        fluid_messages: Sender<FluidMessage>,
        waveform_messages: Sender<WaveformMessage<EventId>>,
        damper_controller: u8,
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            synth_mode,
            continuous: false,
            legato: true,
            scale: Arc::new(scale),
            root_note: NoteLetter::D.in_octave(4),
            pressed_keys: HashMap::new(),
            waveform_number: 0,
            waveforms: Arc::new(wave::all_waveforms()),
            envelope_type: None,
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            keypress_tracker: KeypressTracker::new(),
            channel_tuner: ChannelTuner::new(),
            fluid_messages,
            waveform_messages,
            damper_controller,
        };

        model.set_program(program_number);
        model.retune();

        let engine = Self {
            model: Mutex::new(model),
        };

        (Arc::new(engine), snapshot)
    }

    pub fn handle_key_offset_event(&self, id: EventId, offset: i32, phase: EventPhase) {
        self.lock_model().handle_key_offset_event(id, offset, phase);
    }

    pub fn handle_pitch_event(&self, id: EventId, pitch: Pitch, phase: EventPhase) {
        self.lock_model().handle_pitch_event(id, pitch, phase);
    }

    pub fn handle_midi_event(&self, message_type: ChannelMessageType) {
        self.lock_model().handle_midi_event(message_type);
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
        let mut model = self.lock_model();
        model.envelope_type = match model.envelope_type {
            None => Some(EnvelopeType::Organ),
            Some(EnvelopeType::Organ) => Some(EnvelopeType::Piano),
            Some(EnvelopeType::Piano) => Some(EnvelopeType::Pad),
            Some(EnvelopeType::Pad) => Some(EnvelopeType::Bell),
            Some(EnvelopeType::Bell) => None,
        }
    }

    pub fn toggle_synth_mode(&self) {
        let mut model = self.lock_model();
        model.synth_mode = match model.synth_mode {
            SynthMode::OnlyWaveform => SynthMode::OnlyWaveform,
            SynthMode::Waveform => SynthMode::Fluid,
            SynthMode::Fluid => SynthMode::Waveform,
        };
    }

    pub fn inc_program(&self, curr_program: &mut u8) {
        let mut model = self.lock_model();
        match model.synth_mode {
            SynthMode::OnlyWaveform | SynthMode::Waveform => {
                model.waveform_number = (model.waveform_number + 1) % model.waveforms.len();
            }
            SynthMode::Fluid => {
                *curr_program = (*curr_program + 1) % 128;
                model.set_program(*curr_program);
            }
        }
    }

    pub fn dec_program(&self, curr_program: &mut u8) {
        let mut model = self.lock_model();
        match model.synth_mode {
            SynthMode::OnlyWaveform | SynthMode::Waveform => {
                model.waveform_number =
                    (model.waveform_number + model.waveforms.len() - 1) % model.waveforms.len();
            }
            SynthMode::Fluid => {
                *curr_program = (*curr_program + 128 - 1) % 128;
                model.set_program(*curr_program);
            }
        }
    }

    pub fn inc_root_note(&self) {
        let mut model = self.lock_model();
        model.root_note = model.root_note.plus_semitones(1);
        model.retune();
    }

    pub fn dec_root_note(&self) {
        let mut model = self.lock_model();
        model.root_note = model.root_note.plus_semitones(-1);
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
    fn handle_key_offset_event(&mut self, id: EventId, offset: i32, phase: EventPhase) {
        let key = self.root_note.as_piano_key().plus_steps(offset);
        self.handle_key_event(id, key, phase);
    }

    fn handle_pitch_event(&mut self, id: EventId, mut pitch: Pitch, phase: EventPhase) {
        let tuning = (&*self.scale, Kbm::root_at(self.root_note));
        let key = tuning.find_by_pitch(pitch).approx_value;

        let pitch_is_quantized = match self.pressed_keys.get(&id) {
            Some(pressed_key) => pressed_key.synth_type == SynthType::Fluid,
            None => self.synth_mode == SynthMode::Fluid,
        };

        if pitch_is_quantized || !self.continuous {
            pitch = tuning.pitch_of(key);
        }

        self.handle_event(id, key, pitch, phase)
    }

    fn handle_midi_event(&mut self, message_type: ChannelMessageType) {
        // We currently do not support multiple input channels, s.t. the channel is ignored
        let message = match message_type {
            ChannelMessageType::NoteOff {
                key,
                velocity: _, // FluidLite cannot handle release velocities
            } => {
                self.handle_key_event(
                    EventId::Midi(key),
                    PianoKey::from_midi_number(key),
                    EventPhase::Released,
                );
                return;
            }
            ChannelMessageType::NoteOn { key, velocity } => {
                self.handle_key_event(
                    EventId::Midi(key),
                    PianoKey::from_midi_number(key),
                    EventPhase::Pressed(velocity),
                );
                return;
            }
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                if let Some((channel, note)) =
                    self.channel_and_note_for_key(PianoKey::from_midi_number(key))
                {
                    FluidMessage::Polyphonic {
                        channel,
                        note,
                        event: FluidPolyphonicMessage::KeyPressure { pressure },
                    }
                } else {
                    return;
                }
            }
            ChannelMessageType::ControlChange { controller, value } => {
                if controller == self.damper_controller {
                    self.waveform_messages
                        .send(WaveformMessage::DamperPedal {
                            pressure: f64::from(value) / 127.0,
                        })
                        .unwrap();
                }
                FluidMessage::Global {
                    event: FluidGlobalMessage::ControlChange { controller, value },
                }
            }
            ChannelMessageType::ProgramChange { program } => FluidMessage::Global {
                event: FluidGlobalMessage::ProgramChange { program },
            },
            ChannelMessageType::ChannelPressure { pressure } => FluidMessage::Global {
                event: FluidGlobalMessage::ChannelPressure { pressure },
            },
            ChannelMessageType::PitchBendChange { value } => {
                self.waveform_messages
                    .send(WaveformMessage::PitchBend {
                        bend_level: (f64::from(value) / f64::from(2 << 12)) - 1.0,
                    })
                    .unwrap();
                FluidMessage::Global {
                    event: FluidGlobalMessage::PitchBendChange { value },
                }
            }
        };
        self.fluid_messages.send(message).unwrap();
    }

    fn handle_key_event(&mut self, id: EventId, key: PianoKey, phase: EventPhase) {
        let pitch = (&*self.scale, Kbm::root_at(self.root_note)).pitch_of(key);
        self.handle_event(id, key, pitch, phase);
    }

    fn handle_event(&mut self, id: EventId, key: PianoKey, pitch: Pitch, phase: EventPhase) {
        match phase {
            EventPhase::Pressed(velocity) => match self.synth_mode {
                SynthMode::OnlyWaveform | SynthMode::Waveform => {
                    self.start_waveform(id, pitch, f64::from(velocity) / 127.0);
                    self.pressed_keys.insert(
                        id,
                        VirtualKey {
                            pitch,
                            synth_type: SynthType::Waveform,
                        },
                    );
                }
                SynthMode::Fluid => {
                    self.start_fluid_note(id, key, velocity);
                    self.pressed_keys.insert(
                        id,
                        VirtualKey {
                            pitch,
                            synth_type: SynthType::Fluid,
                        },
                    );
                }
            },
            EventPhase::Moved if self.legato => {
                self.update_waveform(id, pitch);
                self.update_fluid_note(&id, key, 100);
                if let Some(pressed_key) = self.pressed_keys.get_mut(&id) {
                    pressed_key.pitch = pitch;
                }
            }
            EventPhase::Released => {
                self.stop_waveform(id);
                self.stop_fluid_note(&id);
                self.pressed_keys.remove(&id);
            }
            _ => {}
        }
    }

    fn set_program(&self, program_number: u8) {
        self.fluid_messages
            .send(FluidMessage::Global {
                event: FluidGlobalMessage::ProgramChange {
                    program: program_number,
                },
            })
            .unwrap();
    }

    fn retune(&mut self) {
        let padding: i32 = self
            .snapshot
            .scale
            .size()
            .try_into()
            .expect("Scale too big");

        let tuning = (&*self.snapshot.scale, Kbm::root_at(self.root_note));

        let lowest_key: PianoKey = tuning
            .find_by_pitch(Note::from_midi_number(0).pitch())
            .approx_value;

        let highest_key: PianoKey = tuning
            .find_by_pitch(Note::from_midi_number(128).pitch())
            .approx_value;

        let channel_tunings = self.channel_tuner.apply_full_keyboard_tuning(
            &tuning,
            lowest_key.plus_steps(-padding),
            highest_key.plus_steps(padding),
        );

        assert!(
            channel_tunings.len() <= 16,
            "Cannot apply tuning: There are too many notes in one semitone"
        );

        self.fluid_messages
            .send(FluidMessage::Retune {
                channel_tunings: channel_tunings
                    .iter()
                    .map(ChannelTuning::to_fluidlite_format)
                    .collect(),
            })
            .unwrap();
    }

    fn start_waveform(&self, id: EventId, pitch: Pitch, velocity: f64) {
        let waveform =
            self.waveforms[self.waveform_number].new_waveform(pitch, velocity, self.envelope_type);
        self.waveform_messages
            .send(WaveformMessage::Lifecycle {
                id,
                action: WaveformLifecycle::Start { waveform },
            })
            .unwrap();
    }

    fn update_waveform(&self, id: EventId, pitch: Pitch) {
        self.waveform_messages
            .send(WaveformMessage::Lifecycle {
                id,
                action: WaveformLifecycle::Update { pitch },
            })
            .unwrap();
    }

    fn stop_waveform(&self, id: EventId) {
        self.waveform_messages
            .send(WaveformMessage::Lifecycle {
                id,
                action: WaveformLifecycle::Stop,
            })
            .unwrap();
    }

    fn start_fluid_note(&mut self, id: EventId, key: PianoKey, velocity: u8) {
        if let Some(channel_and_note) = self.channel_and_note_for_key(key) {
            match self.keypress_tracker.place_finger_at(id, channel_and_note) {
                Ok(PlaceAction::KeyPressed) | Ok(PlaceAction::KeyAlreadyPressed) => {
                    self.send_fluid_note_on(channel_and_note, velocity)
                }
                Err(id) => eprintln!(
                    "[WARNING] key {:?} with ID {:?} pressed before released",
                    key, id
                ),
            }
        }
    }

    fn update_fluid_note(&mut self, id: &EventId, key: PianoKey, velocity: u8) {
        if let Some(channel_and_note) = self.channel_and_note_for_key(key) {
            match self.keypress_tracker.move_finger_to(id, channel_and_note) {
                Ok((LiftAction::KeyReleased(released_key), _)) => {
                    self.send_fluid_note_off(released_key);
                    self.send_fluid_note_on(channel_and_note, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    self.send_fluid_note_on(channel_and_note, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {
                    // Occurs when mouse moved
                }
            }
        }
    }

    fn stop_fluid_note(&mut self, id: &EventId) {
        match self.keypress_tracker.lift_finger(id) {
            Ok(LiftAction::KeyReleased(channel_and_note)) => {
                self.send_fluid_note_off(channel_and_note)
            }
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {
                // Occurs when in waveform mode
            }
        }
    }

    fn send_fluid_note_on(&self, (channel, note): (u8, u8), velocity: u8) {
        self.fluid_messages
            .send(FluidMessage::Polyphonic {
                channel,
                note,
                event: FluidPolyphonicMessage::NoteOn { velocity },
            })
            .unwrap();
    }

    fn send_fluid_note_off(&self, (channel, note): (u8, u8)) {
        self.fluid_messages
            .send(FluidMessage::Polyphonic {
                channel,
                note,
                event: FluidPolyphonicMessage::NoteOff,
            })
            .unwrap();
    }

    fn channel_and_note_for_key(&self, key: PianoKey) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.channel_tuner.get_channel_and_note_for_key(key) {
            if let Ok(key) = note.midi_number().try_into() {
                if key < 128 {
                    return Some((channel.try_into().unwrap(), key));
                }
            }
        }
        None
    }
}
