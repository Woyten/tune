use std::{
    path::PathBuf,
    sync::mpsc::{self, Sender},
};

use mpsc::Receiver;
use tune::midi::{ChannelMessage, ChannelMessageType};

use crate::model::SelectedProgram;

pub struct FluidSynth {
    messages: Receiver<FluidMessage>,
    message_sender: Sender<FluidMessage>,
    program_updates: Sender<SelectedProgram>,
}

#[derive(Clone, Debug)]
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
        let (sender, receiver) = mpsc::channel();

        Self {
            messages: receiver,
            message_sender: sender,
            program_updates,
        }
    }

    pub fn messages(&self) -> Sender<FluidMessage> {
        self.message_sender.clone()
    }

    pub fn write(&mut self, buffer: &mut [f32]) {
        buffer.iter_mut().for_each(|sample| *sample = 0.0);
    }
}
