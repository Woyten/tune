use crate::piano::PianoEngine;
use midir::{MidiInput, MidiInputConnection};
use std::io::Write;
use std::sync::Arc;
use tune::midi::ChannelMessage;

pub fn connect_to_midi_device(
    target_device: usize,
    mut engine: Arc<PianoEngine>,
    midi_channel: u8,
    midi_logging: bool,
) -> MidiInputConnection<()> {
    let midi_input = MidiInput::new("microwave").unwrap();
    let port = &midi_input.ports()[target_device];

    midi_input
        .connect(
            &port,
            "microwave-input-connection",
            move |_, message, _| {
                process_midi_event(message, &mut engine, midi_channel, midi_logging)
            },
            (),
        )
        .unwrap()
}

fn process_midi_event(
    message: &[u8],
    engine: &mut Arc<PianoEngine>,
    input_channel: u8,
    midi_logging: bool,
) {
    if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
        let stderr = std::io::stderr();
        let mut stderr = stderr.lock();
        if midi_logging {
            writeln!(stderr, "[DEBUG] MIDI message received:").unwrap();
            writeln!(stderr, "{:#?}", channel_message).unwrap();
            writeln!(stderr,).unwrap();
        }
        if channel_message.channel == input_channel {
            engine.handle_midi_event(channel_message.message_type);
        }
    } else {
        let stderr = std::io::stderr();
        let mut stderr = stderr.lock();
        writeln!(stderr, "[WARNING] Unsupported MIDI message received:").unwrap();
        for i in message {
            writeln!(stderr, "{:08b}", i).unwrap();
        }
        writeln!(stderr).unwrap();
    }
}
