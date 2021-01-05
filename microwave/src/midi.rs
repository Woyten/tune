use std::{
    convert::TryFrom,
    fmt::Debug,
    hash::Hash,
    io::Write,
    sync::{mpsc::Sender, Arc},
};

use midir::{MidiInputConnection, MidiOutputConnection};
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    mts,
    pitch::Pitch,
    scala::{KbmRoot, Scl},
    tuner::ChannelTuner,
};
use tune_cli::{
    shared::{self, MidiResult},
    CliResult,
};

use crate::{
    keypress::KeypressTracker,
    piano::{Backend, PianoEngine},
    tools::{MidiBackendHelper, PolyphonicSender},
};

pub fn create<I, E: Eq + Hash + Debug>(
    info_sender: Sender<I>,
    target_port: usize,
) -> CliResult<MidiOutBackend<I, E>> {
    let (device, midi_out) = shared::connect_to_out_device("microwave", target_port)?;
    Ok(MidiOutBackend {
        info_sender,
        device,
        curr_program: 0,
        tuner: ChannelTuner::empty(),
        keypress_tracker: KeypressTracker::new(),
        midi_out,
    })
}

pub struct MidiOutBackend<I, E> {
    info_sender: Sender<I>,
    device: String,
    curr_program: u8,
    tuner: ChannelTuner<i32>,
    keypress_tracker: KeypressTracker<E, (u8, u8)>,
    midi_out: MidiOutputConnection,
}

impl<I: From<MidiInfo> + Send, E: Eq + Hash + Debug + Send> Backend<E> for MidiOutBackend<I, E> {
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let channel_tunings = self.helper().set_tuning(tuning);

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

    fn send_status(&self) {
        self.info_sender
            .send(
                MidiInfo {
                    device: self.device.clone(),
                    program_number: self.curr_program,
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: E, degree: i32, _pitch: Pitch, velocity: u8) {
        self.helper().start(id, degree, velocity);
    }

    fn update_pitch(&mut self, id: E, degree: i32, _pitch: Pitch) {
        self.helper().update(id, degree);
    }

    fn update_pressure(&mut self, id: E, pressure: u8) {
        self.helper().update_pressure(id, pressure);
    }

    fn stop(&mut self, id: E, velocity: u8) {
        self.helper().stop(id, velocity);
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_program =
            u8::try_from(update_fn(usize::from(self.curr_program) + 128) % 128).unwrap();
        self.send_monophonic(ChannelMessageType::ProgramChange {
            program: self.curr_program,
        });
        self.send_status();
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.send_monophonic(ChannelMessageType::ControlChange { controller, value });
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.send_monophonic(ChannelMessageType::ChannelPressure { pressure });
    }

    fn pitch_bend(&mut self, value: i16) {
        self.send_monophonic(ChannelMessageType::PitchBendChange { value });
    }

    fn toggle_envelope_type(&mut self) {}
}

impl<I, E: Eq + Hash + Debug> MidiOutBackend<I, E> {
    fn helper(&mut self) -> MidiBackendHelper<'_, E, &mut MidiOutputConnection> {
        MidiBackendHelper::new(
            &mut self.tuner,
            &mut self.keypress_tracker,
            &mut self.midi_out,
        )
    }

    fn send_monophonic(&mut self, message_type: ChannelMessageType) {
        for channel in 0..16 {
            self.midi_out
                .send(&message_type.in_channel(channel).unwrap().to_raw_message())
                .unwrap()
        }
    }
}

impl PolyphonicSender for &mut MidiOutputConnection {
    fn send(&mut self, message: ChannelMessage) {
        MidiOutputConnection::send(self, &message.to_raw_message()).unwrap();
    }
}

pub struct MidiInfo {
    pub device: String,
    pub program_number: u8,
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
