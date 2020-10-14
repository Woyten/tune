use midir::{MidiInputConnection, MidiOutputConnection};

use crate::shared::{self, MidiError};

pub fn connect_to_in_device(
    target_port: usize,
    callback: impl FnMut(&[u8]) + Send + 'static,
) -> Result<(String, MidiInputConnection<()>), MidiError> {
    shared::connect_to_in_device("tune-cli", target_port, callback)
}

pub fn connect_to_out_device(
    target_port: usize,
) -> Result<(String, MidiOutputConnection), MidiError> {
    shared::connect_to_out_device("tune-cli", target_port)
}
