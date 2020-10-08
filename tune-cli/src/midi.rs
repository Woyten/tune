use std::error::Error;

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use tune::midi;

#[derive(Clone, Debug)]
pub enum MidiError {
    MidiDeviceNotFound(usize),
    Other(String),
}

impl<T: Error> From<T> for MidiError {
    fn from(error: T) -> Self {
        MidiError::Other(error.to_string())
    }
}

pub fn connect_to_out_device(
    target_port: usize,
) -> Result<(String, MidiOutputConnection), MidiError> {
    let midi_output = MidiOutput::new("tune-cli")?;
    match midi_output.ports().get(target_port) {
        Some(port) => Ok((
            midi_output.port_name(port)?,
            midi_output.connect(port, "Output Connection")?,
        )),
        None => Err(MidiError::MidiDeviceNotFound(target_port)),
    }
}

pub fn connect_to_in_device(
    target_port: usize,
    mut callback: impl FnMut(&[u8]) + Send + 'static,
) -> Result<(String, MidiInputConnection<()>), MidiError> {
    let midi_input = MidiInput::new("tune-cli")?;
    match midi_input.ports().get(target_port) {
        Some(port) => Ok((
            midi_input.port_name(port)?,
            midi_input.connect(
                port,
                "Input Connection",
                move |_, message, _| callback(message),
                (),
            )?,
        )),
        None => Err(MidiError::MidiDeviceNotFound(target_port)),
    }
}

pub fn rpn_message(
    channel: u8,
    parameter_number_msb: u8,
    parameter_number_lsb: u8,
    value: u8,
) -> [[u8; 3]; 3] {
    let control_change = channel_message(midi::CONTROL_CHANGE, channel);
    [
        [control_change, 0x65, parameter_number_msb],
        [control_change, 0x64, parameter_number_lsb],
        [control_change, 0x06, value],
    ]
}

fn channel_message(prefix: u8, channel_nr: u8) -> u8 {
    prefix << 4 | channel_nr
}
