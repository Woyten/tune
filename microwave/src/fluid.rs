use fluidlite_lib as _;

use std::{
    convert::TryInto,
    path::PathBuf,
    sync::mpsc::{self, Sender},
};

use fluidlite::{IsPreset, Settings, Synth};
use mpsc::Receiver;
use nannou_audio::Buffer;
use tune::midi::{ChannelMessage, ChannelMessageType};

use crate::model::SelectedProgram;

pub struct FluidSynth {
    synth: Synth,
    messages: Receiver<FluidMessage>,
    message_sender: Sender<FluidMessage>,
    program_updates: Sender<SelectedProgram>,
}

pub enum FluidMessage {
    Polyphonic(ChannelMessage),
    Monophonic(ChannelMessageType),
    Retune { channel_tunings: Vec<[f64; 128]> },
}

impl FluidSynth {
    pub fn new(
        soundfont_file_location: &Option<PathBuf>,
        program_updates: Sender<SelectedProgram>,
    ) -> Self {
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

        let (sender, receiver) = mpsc::channel();

        Self {
            synth,
            messages: receiver,
            message_sender: sender,
            program_updates,
        }
    }

    pub fn messages(&self) -> Sender<FluidMessage> {
        self.message_sender.clone()
    }

    pub fn write(&mut self, buffer: &mut Buffer) {
        for message in self.messages.try_iter() {
            self.process_message(message)
        }
        self.synth.write(&mut buffer[..]).unwrap();
    }

    fn process_message(&self, message: FluidMessage) {
        match message {
            FluidMessage::Polyphonic(channel_message) => self.process_message_type(
                channel_message.channel().into(),
                channel_message.message_type(),
            ),
            FluidMessage::Monophonic(message_type) => {
                for channel in 0..16 {
                    self.process_message_type(channel, message_type)
                }
                if let ChannelMessageType::ProgramChange { program } = message_type {
                    self.program_updates
                        .send(SelectedProgram {
                            program_number: program,
                            program_name: self
                                .synth
                                .get_channel_preset(0)
                                .and_then(|preset| preset.get_name().map(str::to_owned)),
                        })
                        .unwrap();
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
        }
    }

    fn process_message_type(&self, channel: u32, message_type: ChannelMessageType) {
        match message_type {
            ChannelMessageType::NoteOff { key, .. } => {
                // Ignore result since errors can occur when note_off is sent twice
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
            ChannelMessageType::PitchBendChange { value } => {
                self.synth.pitch_bend(channel, value.into()).unwrap()
            }
        }
    }
}
