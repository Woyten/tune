use crate::model::SelectedProgram;
use fluidlite::{IsPreset, Settings, Synth};
use mpsc::Receiver;
use nannou_audio::Buffer;
use std::{
    convert::TryInto,
    path::PathBuf,
    sync::mpsc::{self, Sender},
};

pub struct FluidSynth {
    synth: Synth,
    messages: Receiver<FluidMessage>,
    message_sender: Sender<FluidMessage>,
    program_updates: Sender<SelectedProgram>,
}

pub enum FluidMessage {
    Polyphonic {
        channel: u8,
        note: u8,
        event: FluidPolyphonicMessage,
    },
    Global {
        event: FluidGlobalMessage,
    },
    Retune {
        channel_tunings: Vec<[f64; 128]>,
    },
}

pub enum FluidPolyphonicMessage {
    NoteOn { velocity: u8 },
    NoteOff,
    KeyPressure { pressure: u8 },
}

pub enum FluidGlobalMessage {
    ControlChange { controller: u8, value: u8 },
    ProgramChange { program: u8 },
    ChannelPressure { pressure: u8 },
    PitchBendChange { value: u16 },
}

impl FluidSynth {
    pub fn new(
        soundfont_file_location: Option<PathBuf>,
        program_updates: Sender<SelectedProgram>,
    ) -> Self {
        let settings = Settings::new().unwrap();
        let synth = Synth::new(settings).unwrap();

        if let Some(soundfont_file_location) = soundfont_file_location {
            synth.sfload(soundfont_file_location, false).unwrap();
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
            FluidMessage::Polyphonic {
                channel,
                note,
                event,
            } => {
                let channel = channel.into();
                match event {
                    FluidPolyphonicMessage::NoteOn { velocity } => self
                        .synth
                        .note_on(channel, note.into(), velocity.into())
                        .unwrap(),
                    FluidPolyphonicMessage::NoteOff => {
                        let _ = self.synth.note_off(channel, note.into());
                    }
                    FluidPolyphonicMessage::KeyPressure { pressure } => self
                        .synth
                        .key_pressure(channel, note.into(), pressure.into())
                        .unwrap(),
                }
            }
            FluidMessage::Global { event } => {
                for channel in 0..16 {
                    match event {
                        FluidGlobalMessage::ControlChange { controller, value } => {
                            self.synth
                                .cc(channel, controller.into(), value.into())
                                .unwrap();
                        }
                        FluidGlobalMessage::ProgramChange { program } => {
                            self.synth.bank_select(channel, 0).unwrap();
                            self.synth.program_change(channel, program.into()).unwrap();
                            self.program_updates
                                .send(SelectedProgram {
                                    program_number: program,
                                    program_name: self
                                        .synth
                                        .get_channel_preset(channel)
                                        .and_then(|preset| preset.get_name().map(str::to_owned)),
                                })
                                .unwrap();
                        }
                        FluidGlobalMessage::ChannelPressure { pressure } => self
                            .synth
                            .channel_pressure(channel, pressure.into())
                            .unwrap(),
                        FluidGlobalMessage::PitchBendChange { value } => {
                            self.synth.pitch_bend(channel, value.into()).unwrap()
                        }
                    }
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
}
