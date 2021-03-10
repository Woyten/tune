use std::{convert::TryFrom, io::Write, sync::Arc};

use midir::{MidiInputConnection, MidiOutputConnection};
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    mts,
    pitch::Pitch,
    scala::{KbmRoot, Scl},
    tuner::{ChannelTuner, FullKeyboardDetuning},
};
use tune_cli::{
    shared::{self, MidiResult},
    CliResult,
};

use crate::{
    keypress::KeypressTracker,
    piano::{Backend, PianoEngine},
};

pub fn create<E>(target_port: usize) -> CliResult<MidiOutBackend<E>> {
    let (device, midi_out) = shared::connect_to_out_device("microwave", target_port)?;
    Ok(MidiOutBackend {
        device,
        curr_program: 0,
        channel_tuner: ChannelTuner::empty(),
        keypress_tracker: KeypressTracker::new(),
        midi_out,
    })
}

pub struct MidiOutBackend<E> {
    device: String,
    curr_program: u8,
    channel_tuner: ChannelTuner<i32>,
    keypress_tracker: KeypressTracker<E, (u8, u8)>,
    midi_out: MidiOutputConnection,
}

impl<E: Send> Backend<E> for MidiOutBackend<E> {
    fn start(&mut self, id: E, degree: i32, pitch: Pitch, velocity: u8) {
        todo!("Copy from fluid");
    }

    fn update(&mut self, id: E, degree: i32, pitch: Pitch) {
        todo!("Copy from fluid");
    }

    fn stop(&mut self, id: E, velocity: u8) {
        todo!("Copy from fluid");
    }

    fn update_program(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_program =
            u8::try_from(update_fn(usize::from(self.curr_program) + 128) % 128).unwrap();
    }

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let channel_tunings: Vec<FullKeyboardDetuning> = todo!("Copy from fluid");

        for channel in 0..16 {
            for message in &mts::tuning_program_change(channel, channel).unwrap() {
                self.midi_out.send(&message.to_raw_message()).unwrap();
            }
        }

        for (channel_tuning, channel) in channel_tunings.iter().zip(0..16) {
            let tuning_message = channel_tuning
                .to_mts_format(Default::default(), channel)
                .unwrap();
            for sysex_call in tuning_message.sysex_bytes() {
                self.midi_out.send(sysex_call).unwrap();
            }
        }
    }

    fn polyphonic_key_pressure(&mut self, id: E, pressure: u8) {
        todo!("Copy from fluid");
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        todo!("Copy from fluid");
    }

    fn channel_pressure(&mut self, pressure: u8) {
        todo!("Copy from fluid");
    }
}

impl<E> MidiOutBackend<E> {
    fn send_note_on(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.midi_out
            .send(
                &ChannelMessageType::NoteOn {
                    key: note,
                    velocity,
                }
                .in_channel(channel)
                .unwrap()
                .to_raw_message(),
            )
            .unwrap();
    }

    fn send_note_off(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.midi_out
            .send(
                &ChannelMessageType::NoteOff {
                    key: note,
                    velocity,
                }
                .in_channel(channel)
                .unwrap()
                .to_raw_message(),
            )
            .unwrap();
    }

    fn send_program_change(&mut self, program: u8) {
        for channel in 0..16 {
            let midi_message = ChannelMessageType::ProgramChange { program }
                .in_channel(channel)
                .unwrap();

            self.midi_out.send(&midi_message.to_raw_message()).unwrap()
        }
    }
}

pub fn connect_to_midi_device(
    target_device: usize,
    mut engine: Arc<PianoEngine>,
    midi_channel: u8,
    midi_logging: bool,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    shared::connect_to_in_device("microwave", target_device, move |message| {
        process_midi_event(message, &mut engine, midi_channel, midi_logging)
    })
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
        if channel_message.channel() == input_channel {
            engine.handle_midi_event(channel_message.message_type());
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
