use std::{
    convert::TryFrom,
    fmt::Debug,
    hash::Hash,
    path::Path,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
};

use fluidlite::{IsPreset, IsSettings, Settings, Synth};
use mpsc::Receiver;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    note::Note,
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
    tuner::{AccessKeyResult, GroupByNote, JitTuner, PoolingMode, RegisterKeyResult},
};

use crate::piano::Backend;

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    soundfont_file_location: Option<&Path>,
) -> (FluidBackend<S>, FluidSynth<I>) {
    let settings = Settings::new().unwrap();
    settings
        .str_("synth.drums-channel.active")
        .unwrap()
        .set("no");

    let synth = Synth::new(settings).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        synth.sfload(soundfont_file_location, false).unwrap();
    }

    for channel in 0..16 {
        synth
            .create_key_tuning(0, channel, "microwave-dynamic-tuning", &[0.0; 128])
            .unwrap();
        synth.activate_tuning(channel, 0, channel, true).unwrap();
    }

    let (send, recv) = mpsc::channel();

    (
        FluidBackend {
            sender: send,
            tuner: JitTuner::new(GroupByNote, PoolingMode::Stop, 16),
        },
        FluidSynth {
            synth,
            soundfont_file_location: soundfont_file_location
                .and_then(Path::to_str)
                .map(|l| l.to_owned().into()),
            messages: recv,
            info_sender,
        },
    )
}

pub struct FluidBackend<S> {
    sender: Sender<FluidMessage>,
    tuner: JitTuner<S, GroupByNote>,
}

impl<S: Copy + Eq + Hash + Send + Debug> Backend<S> for FluidBackend<S> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn send_status(&self) {
        self.send(FluidMessage::SendStatus);
    }

    fn start(&mut self, id: S, _degree: i32, pitch: Pitch, velocity: u8) {
        match self.tuner.register_key(id, pitch) {
            RegisterKeyResult::Accepted {
                channel,
                stopped_note,
                started_note,
                detuning,
            } => {
                if let Some(stopped_note) = stopped_note.and_then(Note::checked_midi_number) {
                    self.send(FluidMessage::Polyphonic(
                        ChannelMessageType::NoteOff {
                            key: stopped_note,
                            velocity,
                        }
                        .in_channel(u8::try_from(channel).unwrap())
                        .unwrap(),
                    ))
                }
                if let Some(started_note) = started_note.checked_midi_number() {
                    self.send(FluidMessage::Tune {
                        channel: u32::try_from(channel).unwrap(),
                        note: started_note,
                        detuning,
                    });
                    self.send(FluidMessage::Polyphonic(
                        ChannelMessageType::NoteOn {
                            key: started_note,
                            velocity,
                        }
                        .in_channel(u8::try_from(channel).unwrap())
                        .unwrap(),
                    ));
                }
            }
            RegisterKeyResult::Rejected => {}
        }
    }

    fn update_pitch(&mut self, id: S, _degree: i32, pitch: Pitch) {
        // This has no effect. Fluidlite does not update sounding notes.
        match self.tuner.access_key(&id) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    let detuning =
                        Ratio::between_pitches(Note::from_midi_number(found_note), pitch);
                    self.send(FluidMessage::Tune {
                        channel: u32::try_from(channel).unwrap(),
                        note: found_note,
                        detuning,
                    });
                }
            }
            AccessKeyResult::NotFound => {}
        }
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        match self.tuner.access_key(&id) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.send(FluidMessage::Polyphonic(
                        ChannelMessageType::PolyphonicKeyPressure {
                            key: found_note,
                            pressure,
                        }
                        .in_channel(u8::try_from(channel).unwrap())
                        .unwrap(),
                    ));
                }
            }
            AccessKeyResult::NotFound => todo!(),
        }
    }

    fn stop(&mut self, id: S, velocity: u8) {
        match self.tuner.deregister_key(&id) {
            AccessKeyResult::Found {
                channel,
                found_note,
            } => {
                if let Some(found_note) = found_note.checked_midi_number() {
                    self.send(FluidMessage::Polyphonic(
                        ChannelMessageType::NoteOff {
                            key: found_note,
                            velocity,
                        }
                        .in_channel(u8::try_from(channel).unwrap())
                        .unwrap(),
                    ))
                }
            }
            AccessKeyResult::NotFound => {}
        }
    }

    fn program_change(&mut self, update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.send(FluidMessage::UpdateProgram { update_fn });
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.send(FluidMessage::Monophonic(
            ChannelMessageType::ControlChange { controller, value },
        ));
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.send(FluidMessage::Monophonic(
            ChannelMessageType::ChannelPressure { pressure },
        ));
    }

    fn pitch_bend(&mut self, value: i16) {
        self.send(FluidMessage::Monophonic(
            ChannelMessageType::PitchBendChange { value },
        ));
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        false
    }
}

impl<S> FluidBackend<S> {
    fn send(&self, message: FluidMessage) {
        self.sender.send(message).unwrap();
    }
}

pub struct FluidSynth<I> {
    synth: Synth,
    soundfont_file_location: Option<Arc<str>>,
    messages: Receiver<FluidMessage>,
    info_sender: Sender<I>,
}

impl<I: From<FluidInfo>> FluidSynth<I> {
    pub fn write(&mut self, buffer: &mut [f32]) {
        for message in self.messages.try_iter() {
            self.process_message(message)
        }
        self.synth.write(buffer).unwrap();
    }

    fn process_message(&self, message: FluidMessage) {
        match message {
            FluidMessage::SendStatus => {
                let preset = self.synth.get_channel_preset(0);
                let program = preset
                    .as_ref()
                    .and_then(IsPreset::get_num)
                    .and_then(|p| u8::try_from(p).ok());
                let program_name = preset
                    .as_ref()
                    .and_then(IsPreset::get_name)
                    .map(str::to_owned);
                self.info_sender
                    .send(
                        FluidInfo {
                            soundfont_file_location: self.soundfont_file_location.clone(),
                            program,
                            program_name,
                        }
                        .into(),
                    )
                    .unwrap();
            }
            FluidMessage::Polyphonic(channel_message) => self.process_message_type(
                channel_message.channel().into(),
                channel_message.message_type(),
            ),
            FluidMessage::Monophonic(message_type) => {
                for channel in 0..16 {
                    self.process_message_type(channel, message_type)
                }
                if let ChannelMessageType::ProgramChange { .. } = message_type {
                    self.process_message(FluidMessage::SendStatus);
                }
            }
            FluidMessage::Tune {
                channel,
                note,
                detuning,
            } => {
                let detuning_in_fluid_format =
                    (Ratio::from_semitones(note).stretched_by(detuning)).as_cents();
                self.synth
                    .tune_notes(
                        0,
                        channel,
                        [u32::from(note)],
                        [detuning_in_fluid_format],
                        true,
                    )
                    .unwrap();
            }
            FluidMessage::UpdateProgram { mut update_fn } => {
                let curr_program = usize::try_from(self.synth.get_program(0).unwrap().2).unwrap();
                let updated_program = u8::try_from(update_fn(curr_program + 128) % 128).unwrap();
                self.process_message(FluidMessage::Monophonic(
                    ChannelMessageType::ProgramChange {
                        program: updated_program,
                    },
                ))
            }
        }
    }

    fn process_message_type(&self, channel: u32, message_type: ChannelMessageType) {
        match message_type {
            ChannelMessageType::NoteOff { key, .. } => {
                // When note_on is called for a note that is not supported by the current sound program FluidLite ignores that call.
                // When note_off is sent for the same note afterwards FluidLite reports an error since the note is considered off.
                // This error cannot be anticipated so we just ignore it.
                let _ = self.synth.note_off(channel, key.into());
            }
            ChannelMessageType::NoteOn { key, velocity } => self
                .synth
                .note_on(channel, key.into(), velocity.into())
                .unwrap(),
            ChannelMessageType::PolyphonicKeyPressure { key, pressure } => self
                .synth
                .key_pressure(channel, key.into(), pressure.into())
                .unwrap(),
            ChannelMessageType::ControlChange { controller, value } => {
                self.synth
                    .cc(channel, controller.into(), value.into())
                    .unwrap();
            }
            ChannelMessageType::ProgramChange { program } => {
                self.synth.program_change(channel, program.into()).unwrap();
            }
            ChannelMessageType::ChannelPressure { pressure } => {
                self.synth
                    .channel_pressure(channel, pressure.into())
                    .unwrap();
            }
            ChannelMessageType::PitchBendChange { value } => self
                .synth
                .pitch_bend(channel, (value + 8192) as u32)
                .unwrap(),
        }
    }
}

enum FluidMessage {
    SendStatus,
    Polyphonic(ChannelMessage),
    Monophonic(ChannelMessageType),
    Tune {
        channel: u32,
        note: u8,
        detuning: Ratio,
    },
    UpdateProgram {
        update_fn: Box<dyn FnMut(usize) -> usize + Send>,
    },
}

pub struct FluidInfo {
    pub soundfont_file_location: Option<Arc<str>>,
    pub program: Option<u8>,
    pub program_name: Option<String>,
}
