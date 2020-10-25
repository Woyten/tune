use std::{
    collections::HashMap,
    convert::TryInto,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{mpsc::Sender, Arc, Mutex, MutexGuard},
};

use midir::MidiOutputConnection;
use tune::{
    key::PianoKey,
    midi::ChannelMessageType,
    mts::{self, SingleNoteTuningChangeMessage},
    note::{Note, NoteLetter},
    pitch::{Pitch, Pitched},
    scala::{Kbm, Scl},
    tuner::{ChannelTuner, ChannelTuning},
    tuning::Scale,
    tuning::Tuning,
};
use wave::EnvelopeType;

use crate::{
    fluid::FluidMessage,
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    model::{EventId, EventPhase},
    synth::{WaveformLifecycle, WaveformMessage},
    wave::{self, Patch, Waveform},
};

pub struct PianoEngine {
    model: Mutex<PianoEngineModel>,
}

/// A snapshot of the piano engine state to be used for screen rendering.
/// By rendering the snapshotted version the engine remains responsive even at low screen refresh rates.
#[derive(Clone)]
pub struct PianoEngineSnapshot {
    pub synth_modes: Vec<SynthMode>,
    pub curr_synth_mode: usize,
    pub legato: bool,
    pub scale: Arc<Scl>,
    pub root_note: Note,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
}

#[derive(Clone)]
pub enum SynthMode {
    Waveform {
        curr_waveform: usize,
        waveforms: Arc<Vec<Patch>>, // Arc used here in order to prevent cloning of the inner Vec
        envelope_type: Option<EnvelopeType>,
        continuous: bool,
    },
    Fluid {
        soundfont_file_location: PathBuf,
    },
    MidiOut {
        device: String,
        curr_program: u8,
    },
}

#[derive(Clone, Debug)]
pub struct VirtualKey {
    pub pitch: Pitch,
}

struct PianoEngineModel {
    snapshot: PianoEngineSnapshot,
    keypress_tracker: KeypressTracker<EventId, KeyLocation>,
    channel_tuner: ChannelTuner<i32>,
    fluid_messages: Sender<FluidMessage>,
    waveform_messages: Sender<WaveformMessage<EventId>>,
    midi_out: Option<MidiOutputConnection>,
    damper_controller: u8,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
enum KeyLocation {
    FluidSynth((u8, u8)),
    MidiOutSynth(u8),
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
        scale: Scl,
        available_synth_modes: Vec<SynthMode>,
        waveform_messages: Sender<WaveformMessage<EventId>>,
        damper_controller: u8,
        fluid_messages: Sender<FluidMessage>,
        midi_out: Option<MidiOutputConnection>,
        program_number: u8,
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            synth_modes: available_synth_modes,
            curr_synth_mode: 0,
            legato: true,
            scale: Arc::new(scale),
            root_note: NoteLetter::D.in_octave(4),
            pressed_keys: HashMap::new(),
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            keypress_tracker: KeypressTracker::new(),
            channel_tuner: ChannelTuner::new(),
            fluid_messages,
            waveform_messages,
            midi_out,
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
        if let SynthMode::Waveform { continuous, .. } = self.lock_model().synth_mode_mut() {
            *continuous = !*continuous;
        }
    }

    pub fn toggle_envelope_type(&self) {
        if let SynthMode::Waveform { envelope_type, .. } = self.lock_model().synth_mode_mut() {
            *envelope_type = match *envelope_type {
                None => Some(EnvelopeType::Organ),
                Some(EnvelopeType::Organ) => Some(EnvelopeType::Piano),
                Some(EnvelopeType::Piano) => Some(EnvelopeType::Pad),
                Some(EnvelopeType::Pad) => Some(EnvelopeType::Bell),
                Some(EnvelopeType::Bell) => None,
            }
        }
    }

    pub fn toggle_synth_mode(&self) {
        let mut model = self.lock_model();
        model.curr_synth_mode += 1;
        model.curr_synth_mode %= model.synth_modes.len();
    }

    pub fn inc_program(&self, curr_program: &mut u8) {
        let mut model = self.lock_model();
        let model = model.deref_mut();

        match model.snapshot.synth_mode_mut() {
            SynthMode::Waveform {
                curr_waveform,
                waveforms,
                ..
            } => {
                *curr_waveform += 1;
                *curr_waveform %= waveforms.len();
            }
            SynthMode::Fluid { .. } => {
                *curr_program += 1;
                *curr_program %= 128;
                model.set_program(*curr_program);
            }
            SynthMode::MidiOut { curr_program, .. } => {
                let new_program = *curr_program + 1;
                model.set_program(new_program);
            }
        }
    }

    pub fn dec_program(&self, curr_program: &mut u8) {
        let mut model = self.lock_model();
        let model = model.deref_mut();

        match model.snapshot.synth_mode_mut() {
            SynthMode::Waveform {
                curr_waveform,
                waveforms,
                ..
            } => {
                *curr_waveform += waveforms.len() - 1;
                *curr_waveform %= waveforms.len();
            }
            SynthMode::Fluid { .. } => {
                *curr_program += 128 - 1;
                *curr_program %= 128;
                model.set_program(*curr_program);
            }
            SynthMode::MidiOut { curr_program, .. } => {
                let new_program = *curr_program + 128 - 1;
                model.set_program(new_program);
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

        let should_quantize = match self.synth_mode() {
            SynthMode::Waveform { continuous, .. } => !*continuous,
            SynthMode::Fluid { .. } | SynthMode::MidiOut { .. } => true,
        };

        if should_quantize {
            pitch = tuning.pitch_of(key);
        }

        self.handle_event(id, key, pitch, phase)
    }

    fn handle_midi_event(&mut self, message_type: ChannelMessageType) {
        // We currently do not support multiple input channels, s.t. the channel is ignored.
        let fluid_message = match message_type {
            // Intercepted by the engine.
            ChannelMessageType::NoteOff { key, velocity } => {
                self.handle_key_event(
                    EventId::Midi(key),
                    PianoKey::from_midi_number(key),
                    EventPhase::Released(velocity),
                );
                return;
            }
            // Intercepted by the engine.
            ChannelMessageType::NoteOn { key, velocity } => {
                self.handle_key_event(
                    EventId::Midi(key),
                    PianoKey::from_midi_number(key),
                    EventPhase::Pressed(velocity),
                );
                return;
            }
            // Transformed and forwarded to all MIDI synths if possible. Should be intercepted in the future.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                if let Some((channel, note)) =
                    self.channel_and_note_for_key(PianoKey::from_midi_number(key))
                {
                    FluidMessage::Polyphonic(
                        ChannelMessageType::PolyphonicKeyPressure {
                            key: note,
                            pressure,
                        }
                        .in_channel(channel)
                        .unwrap(),
                    )
                } else {
                    return;
                }
            }
            // Forwarded to all channels of all synths.
            ChannelMessageType::ControlChange { controller, value } => {
                if controller == self.damper_controller {
                    self.waveform_messages
                        .send(WaveformMessage::DamperPedal {
                            pressure: f64::from(value) / 127.0,
                        })
                        .unwrap();
                }
                FluidMessage::Monophonic(message_type)
            }
            // Intercepted by the engine.
            ChannelMessageType::ProgramChange { program } => {
                self.set_program(program);
                return;
            }
            // Forwarded to all channels of all MIDI synths. Should be intercepted in the future.
            ChannelMessageType::ChannelPressure { .. } => FluidMessage::Monophonic(message_type),
            // Forwarded to all channels of all synths.
            ChannelMessageType::PitchBendChange { value } => {
                self.waveform_messages
                    .send(WaveformMessage::PitchBend {
                        bend_level: (f64::from(value) / f64::from(2 << 12)) - 1.0,
                    })
                    .unwrap();
                FluidMessage::Monophonic(message_type)
            }
        };

        self.fluid_messages.send(fluid_message).unwrap();
        if let Some(midi_out) = &mut self.midi_out {
            midi_out
                .send(&message_type.in_channel(0).unwrap().to_raw_message())
                .unwrap()
        }
    }

    fn handle_key_event(&mut self, id: EventId, key: PianoKey, phase: EventPhase) {
        let pitch = (&*self.scale, Kbm::root_at(self.root_note)).pitch_of(key);
        self.handle_event(id, key, pitch, phase);
    }

    fn handle_event(&mut self, id: EventId, key: PianoKey, pitch: Pitch, phase: EventPhase) {
        match phase {
            EventPhase::Pressed(velocity) => match self.synth_mode() {
                SynthMode::Waveform {
                    curr_waveform,
                    waveforms,
                    envelope_type,
                    ..
                } => {
                    let waveform = waveforms[*curr_waveform].new_waveform(
                        pitch,
                        f64::from(velocity) / 127.0,
                        *envelope_type,
                    );
                    self.start_waveform(id, waveform);
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
                SynthMode::Fluid { .. } => {
                    if let Some(channel_and_note) = self.channel_and_note_for_key(key) {
                        self.start_note(id, KeyLocation::FluidSynth(channel_and_note), velocity);
                    }
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
                SynthMode::MidiOut { .. } => {
                    if let Some(note) = key.checked_midi_number() {
                        self.start_note(id, KeyLocation::MidiOutSynth(note), velocity);
                    }
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
            },
            EventPhase::Moved => {
                if self.legato {
                    self.update_waveform(id, pitch);
                    self.update_note(&id, key, 100);
                    if let Some(pressed_key) = self.pressed_keys.get_mut(&id) {
                        pressed_key.pitch = pitch;
                    }
                }
            }
            EventPhase::Released(velocity) => {
                self.stop_waveform(id);
                self.stop_note(&id, velocity);
                self.pressed_keys.remove(&id);
            }
        }
    }

    fn set_program(&mut self, program: u8) {
        match self.synth_mode_mut() {
            SynthMode::Waveform {
                curr_waveform,
                waveforms,
                ..
            } => *curr_waveform = usize::from(program) % waveforms.len(),
            SynthMode::Fluid { .. } => {
                self.fluid_messages
                    .send(FluidMessage::Monophonic(
                        ChannelMessageType::ProgramChange {
                            program: program % 128,
                        },
                    ))
                    .unwrap();
            }
            SynthMode::MidiOut { curr_program, .. } => {
                *curr_program = program % 128;

                let midi_message = ChannelMessageType::ProgramChange {
                    program: *curr_program,
                }
                .in_channel(0)
                .unwrap();

                self.midi_out
                    .as_mut()
                    .unwrap()
                    .send(&midi_message.to_raw_message())
                    .unwrap()
            }
        }
    }

    fn retune(&mut self) {
        let tuning = &(&*self.snapshot.scale, Kbm::root_at(self.root_note));

        // FLUID
        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(0).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let channel_tunings = self
            .channel_tuner
            .apply_full_keyboard_tuning(tuning.as_sorted_tuning(), lowest_key - 1..highest_key + 1);

        assert!(
            channel_tunings.len() <= 16,
            "Cannot apply tuning: There are too many notes in one semitone"
        );

        self.fluid_messages
            .send(FluidMessage::Retune {
                channel_tunings: channel_tunings
                    .iter()
                    .map(ChannelTuning::to_fluid_format)
                    .collect(),
            })
            .unwrap();

        // MIDI-out
        if let Some(midi_out) = &mut self.midi_out {
            for message in &mts::tuning_program_change(0, 0).unwrap() {
                midi_out.send(&message.to_raw_message()).unwrap();
            }

            let sntcm =
                SingleNoteTuningChangeMessage::from_scale(&tuning, Default::default(), 0).unwrap();
            midi_out.send(&sntcm.sysex_bytes()).unwrap();
        }
    }

    fn start_waveform(&self, id: EventId, waveform: Waveform) {
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

    fn start_note(&mut self, id: EventId, location: KeyLocation, velocity: u8) {
        match self.keypress_tracker.place_finger_at(id, location) {
            Ok(PlaceAction::KeyPressed) | Ok(PlaceAction::KeyAlreadyPressed) => {
                self.send_note_on(location, velocity)
            }
            Err(id) => eprintln!(
                "[WARNING] location {:?} with ID {:?} released before pressed",
                location, id
            ),
        }
    }

    fn update_note(&mut self, id: &EventId, key: PianoKey, velocity: u8) {
        let location = match self.synth_mode() {
            SynthMode::Waveform { .. } => None,
            SynthMode::Fluid { .. } => self
                .channel_and_note_for_key(key)
                .map(KeyLocation::FluidSynth),
            SynthMode::MidiOut { .. } => key.checked_midi_number().map(KeyLocation::MidiOutSynth),
        };

        if let Some(location) = location {
            match self.keypress_tracker.move_finger_to(id, location) {
                Ok((LiftAction::KeyReleased(released), _)) => {
                    self.send_note_off(released, velocity);
                    self.send_note_on(location, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    self.send_note_on(location, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {
                    // Occurs when mouse moved
                }
            }
        }
    }

    fn stop_note(&mut self, id: &EventId, velocity: u8) {
        match self.keypress_tracker.lift_finger(id) {
            Ok(LiftAction::KeyReleased(location)) => self.send_note_off(location, velocity),
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {
                // Occurs when in waveform mode
            }
        }
    }

    fn channel_and_note_for_key(&self, key: PianoKey) -> Option<(u8, u8)> {
        let scale_degree = self.root_note.as_piano_key().num_keys_before(key);
        if let Some((channel, note)) = self
            .channel_tuner
            .get_channel_and_note_for_key(scale_degree)
        {
            if let Some(key) = note.checked_midi_number() {
                return Some((channel.try_into().unwrap(), key));
            }
        }
        None
    }

    fn send_note_on(&mut self, location: KeyLocation, velocity: u8) {
        match location {
            KeyLocation::FluidSynth((channel, note)) => {
                self.fluid_messages
                    .send(FluidMessage::Polyphonic(
                        ChannelMessageType::NoteOn {
                            key: note,
                            velocity,
                        }
                        .in_channel(channel)
                        .unwrap(),
                    ))
                    .unwrap();
            }
            KeyLocation::MidiOutSynth(key) => {
                self.midi_out
                    .as_mut()
                    .unwrap()
                    .send(
                        &ChannelMessageType::NoteOn { key, velocity }
                            .in_channel(0)
                            .unwrap()
                            .to_raw_message(),
                    )
                    .unwrap();
            }
        }
    }

    fn send_note_off(&mut self, location: KeyLocation, velocity: u8) {
        match location {
            KeyLocation::FluidSynth((channel, note)) => {
                self.fluid_messages
                    .send(FluidMessage::Polyphonic(
                        ChannelMessageType::NoteOff {
                            key: note,
                            velocity,
                        }
                        .in_channel(channel)
                        .unwrap(),
                    ))
                    .unwrap();
            }
            KeyLocation::MidiOutSynth(key) => {
                self.midi_out
                    .as_mut()
                    .unwrap()
                    .send(
                        &ChannelMessageType::NoteOff { key, velocity }
                            .in_channel(0)
                            .unwrap()
                            .to_raw_message(),
                    )
                    .unwrap();
            }
        }
    }
}

impl PianoEngineSnapshot {
    pub fn synth_mode_mut(&mut self) -> &mut SynthMode {
        &mut self.synth_modes[self.curr_synth_mode]
    }

    pub fn synth_mode(&self) -> &SynthMode {
        &self.synth_modes[self.curr_synth_mode]
    }
}
