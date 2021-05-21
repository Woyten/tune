use std::{
    convert::{TryFrom, TryInto},
    fmt::Debug,
    hash::Hash,
    path::Path,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
};

use fluidlite::{IsPreset, Settings, Synth};
use mpsc::Receiver;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    pitch::Pitch,
    scala::{KbmRoot, Scl},
    tuner::{ChannelTuner, FullKeyboardDetuning},
};

use crate::{
    keypress::KeypressTracker,
    piano::Backend,
    tools::{MidiBackendHelper, PolyphonicSender},
};

pub fn create<I, S>(
    info_sender: Sender<I>,
    soundfont_file_location: Option<&Path>,
) -> (FluidBackend<S>, FluidSynth<I>) {
    let settings = Settings::new().unwrap();
    let synth = Synth::new(settings).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        synth.sfload(soundfont_file_location, false).unwrap();
    }

    for channel in 0..16 {
        // Initialize the bank s.t. channel 9 will not have a drum kit loaded
        synth.bank_select(channel, 0).unwrap();

        // Initilize the program s.t. fluidsynth will not error on note_on
        synth.program_change(channel, 0).unwrap();
    }

    let (send, recv) = mpsc::channel();

    (
        FluidBackend {
            sender: send,
            tuner: ChannelTuner::empty(),
            keypress_tracker: KeypressTracker::new(),
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
    tuner: ChannelTuner<i32>,
    keypress_tracker: KeypressTracker<S, (u8, u8)>,
}

impl<S: Send + Eq + Hash + Debug> Backend<S> for FluidBackend<S> {
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let channel_tunings = self
            .helper()
            .set_tuning(tuning, ChannelTuner::apply_full_keyboard_tuning);

        self.send(FluidMessage::Retune {
            channel_tunings: channel_tunings
                .iter()
                .map(FullKeyboardDetuning::to_fluid_format)
                .collect(),
        });
    }

    fn send_status(&self) {
        self.send(FluidMessage::SendStatus);
    }

    fn start(&mut self, id: S, degree: i32, _pitch: Pitch, velocity: u8) {
        self.helper().start(id, degree, velocity);
    }

    fn update_pitch(&mut self, id: S, degree: i32, _pitch: Pitch) {
        self.helper().update(id, degree);
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.helper().update_pressure(id, pressure);
    }

    fn stop(&mut self, id: S, velocity: u8) {
        self.helper().stop(id, velocity);
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
}

impl<S: Eq + Hash + Debug> FluidBackend<S> {
    fn helper(&mut self) -> MidiBackendHelper<'_, S, &Sender<FluidMessage>> {
        MidiBackendHelper::new(&mut self.tuner, &mut self.keypress_tracker, &self.sender)
    }

    fn send(&self, message: FluidMessage) {
        self.sender.send(message).unwrap();
    }
}

impl PolyphonicSender for &Sender<FluidMessage> {
    fn send(&mut self, message: ChannelMessage) {
        Sender::send(self, FluidMessage::Polyphonic(message)).unwrap();
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
            FluidMessage::Retune { channel_tunings } => {
                for (channel, channel_tuning) in channel_tunings.iter().enumerate() {
                    let channel = channel.try_into().unwrap();
                    self.synth
                        .create_key_tuning(0, channel, "microwave-dynamic-tuning", &channel_tuning)
                        .unwrap();
                    self.synth
                        .activate_tuning(channel, 0, channel, true)
                        .unwrap();
                }
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
    Retune {
        channel_tunings: Vec<[f64; 128]>,
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
