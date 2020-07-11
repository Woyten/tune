use crate::model::PianoEngine;
use midir::{MidiInput, MidiInputConnection};
use std::sync::Arc;

pub fn connect_to_midi_device(
    target_device: usize,
    engine: Arc<PianoEngine>,
) -> MidiInputConnection<Arc<PianoEngine>> {
    let midi_input = MidiInput::new("microwave").unwrap();
    let port = &midi_input.ports()[target_device];

    midi_input
        .connect(
            &port,
            "microwave-input-connection",
            process_midi_event,
            engine,
        )
        .unwrap()
}

fn process_midi_event(_: u64, message: &[u8], engine: &mut Arc<PianoEngine>) {
    match message[0] & 0b1111_0000 {
        0b1000_0000 => engine.midi_off(message[1]),
        0b1001_0000 => engine.midi_on(message[1]),
        _ => {}
    }
}
