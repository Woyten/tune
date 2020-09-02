use midir::MidiOutput;
use std::{env, thread, time::Duration};

fn main() {
    let mut args = env::args();
    args.next();
    let index = args.next().unwrap().parse::<usize>().unwrap();
    let midi_output = MidiOutput::new("tune-cli-example").unwrap();
    let port = &midi_output.ports()[index];
    let mut connection = midi_output.connect(&port, "example-connection").unwrap();

    loop {
        connection.send(&note_on(0, 62, 100)).unwrap();
        thread::sleep(Duration::from_secs(1));
        connection.send(&note_on(0, 66, 100)).unwrap();
        thread::sleep(Duration::from_secs(1));
    }
}

fn note_on(channel: u8, note: u8, velocity: u8) -> [u8; 3] {
    [channel_msg(0b1001, channel), note, velocity]
}

fn channel_msg(prefix: u8, channel_nr: u8) -> u8 {
    prefix << 4 | channel_nr
}
