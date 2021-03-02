#![allow(clippy::too_many_arguments)] // Valid lint but the error popped up too late s.t. this will be fixed in the future.

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
    mts,
    note::Note,
    pitch::{Pitch, Pitched, Ratio},
    scala::{Kbm, KbmRoot, Scl},
    tuner::{ChannelTuner, FullKeyboardDetuning},
    tuning::{Scale, Tuning},
};

use crate::{
    fluid::FluidMessage,
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    magnetron::{
        envelope::EnvelopeType,
        waveform::{Waveform, WaveformSpec},
    },
    model::{EventId, EventPhase},
    synth::{ControlStorage, SynthControl, WaveformLifecycle, WaveformMessage},
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
    pub scl: Arc<Scl>,
    pub kbm: Arc<Kbm>,
    pub pressed_keys: HashMap<EventId, VirtualKey>,
}

#[derive(Clone)]
pub enum SynthMode {
    Waveform {
        curr_waveform: usize,
        waveforms: Arc<[WaveformSpec<SynthControl>]>, // Arc used here in order to prevent cloning of the inner Vec
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
    cc_numbers: ControlChangeNumbers,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
enum KeyLocation {
    FluidSynth((u8, u8)),
    MidiOutSynth((u8, u8)),
}

pub struct ControlChangeNumbers {
    pub modulation: u8,
    pub breath: u8,
    pub foot: u8,
    pub expression: u8,
    pub damper: u8,
    pub sostenuto: u8,
    pub soft: u8,
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
        available_synth_modes: Vec<SynthMode>,
        waveform_messages: Sender<WaveformMessage<EventId>>,
        cc_numbers: ControlChangeNumbers,
        fluid_messages: Sender<FluidMessage>,
        midi_out: Option<MidiOutputConnection>,
        program_number: u8,
    ) -> (Arc<Self>, PianoEngineSnapshot) {
        let snapshot = PianoEngineSnapshot {
            synth_modes: available_synth_modes,
            curr_synth_mode: 0,
            legato: true,
            scl: Arc::new(scl),
            kbm: Arc::new(kbm),
            pressed_keys: HashMap::new(),
        };

        let mut model = PianoEngineModel {
            snapshot: snapshot.clone(),
            keypress_tracker: KeypressTracker::new(),
            channel_tuner: ChannelTuner::empty(),
            fluid_messages,
            waveform_messages,
            midi_out,
            cc_numbers,
        };

        model.set_program(program_number);
        model.retune();

        let engine = Self {
            model: Mutex::new(model),
        };

        (Arc::new(engine), snapshot)
    }

    pub fn handle_key_offset_event(&self, id: EventId, offset: i32, phase: EventPhase) {
        self.lock_model().handle_key_event(id, offset, phase);
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

    pub fn control(&self, control: SynthControl, value: f64) {
        self.lock_model()
            .waveform_messages
            .send(WaveformMessage::Control { control, value })
            .unwrap()
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
                if let Some(degree) = self.kbm.scale_degree_of(PianoKey::from_midi_number(key)) {
                    self.handle_key_event(
                        EventId::Midi(key),
                        degree,
                        EventPhase::Released(velocity),
                    );
                }
                return;
            }
            // Intercepted by the engine.
            ChannelMessageType::NoteOn { key, velocity } => {
                if let Some(degree) = self.kbm.scale_degree_of(PianoKey::from_midi_number(key)) {
                    self.handle_key_event(
                        EventId::Midi(key),
                        degree,
                        EventPhase::Pressed(velocity),
                    );
                }
                return;
            }
            // Transformed and forwarded to all MIDI synths if possible. Should be intercepted in the future.
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                self.waveform_messages
                    .send(WaveformMessage::Lifecycle {
                        id: EventId::Midi(key),
                        action: WaveformLifecycle::UpdatePressure {
                            pressure: f64::from(pressure) / 127.0,
                        },
                    })
                    .unwrap();

                if let Some((channel, note)) = self
                    .kbm
                    .scale_degree_of(PianoKey::from_midi_number(key))
                    .and_then(|degree| self.channel_and_note_for_degree(degree))
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
                let value = f64::from(value) / 127.0;
                if controller == self.cc_numbers.modulation {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Modulation,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.breath {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Breath,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.foot {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Foot,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.expression {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Expression,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.damper {
                    self.waveform_messages
                        .send(WaveformMessage::DamperPedal { pressure: value })
                        .unwrap();
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Damper,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.sostenuto {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::Sostenuto,
                            value,
                        })
                        .unwrap();
                }
                if controller == self.cc_numbers.soft {
                    self.waveform_messages
                        .send(WaveformMessage::Control {
                            control: SynthControl::SoftPedal,
                            value,
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
            ChannelMessageType::ChannelPressure { pressure } => {
                self.waveform_messages
                    .send(WaveformMessage::Control {
                        control: SynthControl::ChannelPressure,
                        value: f64::from(pressure) / 127.0,
                    })
                    .unwrap();
                FluidMessage::Monophonic(message_type)
            }
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

        self.fluid_messages.send(fluid_message.clone()).unwrap();
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
    }

    fn handle_key_event(&mut self, id: EventId, degree: i32, phase: EventPhase) {
        self.handle_event(id, degree, self.tuning().pitch_of(degree), phase);
    }

    fn handle_event(&mut self, id: EventId, degree: i32, pitch: Pitch, phase: EventPhase) {
        match phase {
            EventPhase::Pressed(velocity) => match self.synth_mode() {
                SynthMode::Waveform {
                    curr_waveform,
                    waveforms,
                    envelope_type,
                    ..
                } => {
                    let waveform = waveforms[*curr_waveform].create_waveform(
                        pitch,
                        f64::from(velocity) / 127.0,
                        *envelope_type,
                    );
                    self.start_waveform(id, waveform);
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
                SynthMode::Fluid { .. } => {
                    if let Some(channel_and_note) = self.channel_and_note_for_degree(degree) {
                        self.start_note(id, KeyLocation::FluidSynth(channel_and_note), velocity);
                    }
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
                SynthMode::MidiOut { .. } => {
                    if let Some(channel_and_note) = self.channel_and_note_for_degree(degree) {
                        self.start_note(id, KeyLocation::MidiOutSynth(channel_and_note), velocity);
                    }
                    self.pressed_keys.insert(id, VirtualKey { pitch });
                }
            },
            EventPhase::Moved => {
                if self.legato {
                    self.update_waveform(id, pitch);
                    self.update_note(&id, degree, 100);
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
                let program = *curr_program;

                for channel in 0..16 {
                    let midi_message = ChannelMessageType::ProgramChange { program }
                        .in_channel(channel)
                        .unwrap();

                    self.midi_out
                        .as_mut()
                        .unwrap()
                        .send(&midi_message.to_raw_message())
                        .unwrap()
                }
            }
        }
    }

    fn retune(&mut self) {
        let tuning = self.snapshot.tuning();

        // FLUID
        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let (tuner, channel_tunings) = ChannelTuner::apply_full_keyboard_tuning(
            tuning.as_sorted_tuning().as_linear_mapping(),
            lowest_key..highest_key,
        );
        self.channel_tuner = tuner;

        assert!(
            channel_tunings.len() <= 16,
            "Cannot apply tuning: There are too many notes in one semitone"
        );

        self.fluid_messages
            .send(FluidMessage::Retune {
                channel_tunings: channel_tunings
                    .iter()
                    .map(FullKeyboardDetuning::to_fluid_format)
                    .collect(),
            })
            .unwrap();

        // MIDI-out
        if let Some(midi_out) = &mut self.midi_out {
            for channel in 0..16 {
                for message in &mts::tuning_program_change(channel, channel).unwrap() {
                    midi_out.send(&message.to_raw_message()).unwrap();
                }
            }

            for (channel_tuning, channel) in channel_tunings.iter().zip(0..16) {
                let tuning_message = channel_tuning
                    .to_mts_format(Default::default(), channel)
                    .unwrap();
                for sysex_call in tuning_message.sysex_bytes() {
                    midi_out.send(sysex_call).unwrap();
                }
            }
        }
    }

    fn start_waveform(&self, id: EventId, waveform: Waveform<ControlStorage>) {
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
                action: WaveformLifecycle::UpdatePitch { pitch },
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

    fn update_note(&mut self, id: &EventId, degree: i32, velocity: u8) {
        let location = match self.synth_mode() {
            SynthMode::Waveform { .. } => None,
            SynthMode::Fluid { .. } => self
                .channel_and_note_for_degree(degree)
                .map(KeyLocation::FluidSynth),
            SynthMode::MidiOut { .. } => self
                .channel_and_note_for_degree(degree)
                .map(KeyLocation::MidiOutSynth),
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

    fn channel_and_note_for_degree(&self, degree: i32) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.channel_tuner.get_channel_and_note_for_key(degree) {
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
            KeyLocation::MidiOutSynth((channel, note)) => {
                self.midi_out
                    .as_mut()
                    .unwrap()
                    .send(
                        &ChannelMessageType::NoteOn {
                            key: note,
                            velocity,
                        }
                        .in_channel(channel)
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
            KeyLocation::MidiOutSynth((channel, note)) => {
                self.midi_out
                    .as_mut()
                    .unwrap()
                    .send(
                        &ChannelMessageType::NoteOff {
                            key: note,
                            velocity,
                        }
                        .in_channel(channel)
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

    pub fn tuning(&self) -> (&Scl, KbmRoot) {
        (&self.scl, self.kbm.kbm_root())
    }
}
