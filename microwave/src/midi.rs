use std::{
    fmt::Debug,
    hash::Hash,
    io::Write,
    sync::{mpsc::Sender, Arc},
};

use midir::{MidiInputConnection, MidiOutputConnection};
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    mts::{self, ScaleOctaveTuningFormat, ScaleOctaveTuningOptions, SingleNoteTuningChangeOptions},
    note::Note,
    pitch::{Pitch, Pitched},
    scala::{KbmRoot, Scl},
    tuner::AotTuner,
    tuning::{Scale, Tuning},
};
use tune_cli::{
    shared::{self, MidiResult},
    CliResult,
};

use crate::{
    keypress::{IllegalState, KeypressTracker, LiftAction, PlaceAction},
    piano::{Backend, PianoEngine},
};

pub fn create<I, E: Eq + Hash + Debug>(
    info_sender: Sender<I>,
    target_port: &str,
    tuning_method: TuningMethod,
) -> CliResult<MidiOutBackend<I, E>> {
    let (device, midi_out) = shared::connect_to_out_device("microwave", target_port)?;

    Ok(MidiOutBackend {
        info_sender,
        device,
        tuning_method,
        curr_program: 0,
        tuner: AotTuner::empty(),
        keypress_tracker: KeypressTracker::new(),
        midi_out,
    })
}

pub struct MidiOutBackend<I, E> {
    info_sender: Sender<I>,
    device: String,
    tuning_method: TuningMethod,
    curr_program: u8,
    tuner: AotTuner<i32>,
    keypress_tracker: KeypressTracker<E, (u8, u8)>,
    midi_out: MidiOutputConnection,
}

pub enum TuningMethod {
    FullKeyboard(bool),
    Octave1(bool),
    Octave2(bool),
    ChannelFineTuning,
    PitchBend,
}

impl<I: From<MidiInfo> + Send, S: Eq + Hash + Debug + Send> Backend<S> for MidiOutBackend<I, S> {
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let tuning = tuning.as_sorted_tuning().as_linear_mapping();
        let keys = lowest_key..highest_key;

        fn zip_with_channel<T>(detunings: Vec<T>) -> impl Iterator<Item = (T, u8)> {
            let zip_limit = if detunings.len() > 16 {
                println!("[WARNING] Cannot apply tuning. More than 16 channels are required.");
                0
            } else {
                16
            };

            detunings.into_iter().zip(0..zip_limit)
        }

        self.tuner = match self.tuning_method {
            TuningMethod::FullKeyboard(realtime) => {
                let (tuner, detunings) = AotTuner::apply_full_keyboard_tuning(tuning, keys);
                for (detuning, channel) in zip_with_channel(detunings) {
                    for message in &mts::tuning_program_change(channel, channel).unwrap() {
                        self.midi_out.send(&message.to_raw_message()).unwrap();
                    }
                    let options = SingleNoteTuningChangeOptions {
                        realtime,
                        tuning_program: channel,
                        ..Default::default()
                    };
                    let tuning_message = detuning.to_mts_format(&options).unwrap();
                    for sysex_call in tuning_message.sysex_bytes() {
                        self.midi_out.send(sysex_call).unwrap();
                    }
                }
                tuner
            }
            TuningMethod::Octave1(realtime) => {
                let (tuner, detunings) = AotTuner::apply_octave_based_tuning(tuning, keys);
                for (detuning, channel) in zip_with_channel(detunings) {
                    let options = ScaleOctaveTuningOptions {
                        realtime,
                        channels: channel.into(),
                        format: ScaleOctaveTuningFormat::OneByte,
                        ..Default::default()
                    };
                    let tuning_message = detuning.to_mts_format(&options).unwrap();
                    self.midi_out.send(tuning_message.sysex_bytes()).unwrap();
                }
                tuner
            }
            TuningMethod::Octave2(realtime) => {
                let (tuner, detunings) = AotTuner::apply_octave_based_tuning(tuning, keys);
                for (detuning, channel) in zip_with_channel(detunings) {
                    let options = ScaleOctaveTuningOptions {
                        realtime,
                        channels: channel.into(),
                        format: ScaleOctaveTuningFormat::TwoByte,
                        ..Default::default()
                    };
                    let tuning_message = detuning.to_mts_format(&options).unwrap();
                    self.midi_out.send(tuning_message.sysex_bytes()).unwrap();
                }
                tuner
            }
            TuningMethod::ChannelFineTuning => {
                let (tuner, detunings) = AotTuner::apply_channel_based_tuning(tuning, keys);
                for (detuning, channel) in zip_with_channel(detunings) {
                    for message in &mts::channel_fine_tuning(channel, detuning).unwrap() {
                        self.midi_out.send(&message.to_raw_message()).unwrap();
                    }
                }
                tuner
            }
            TuningMethod::PitchBend => {
                let (tuner, detunings) = AotTuner::apply_channel_based_tuning(tuning, keys);
                for (detuning, channel) in zip_with_channel(detunings) {
                    self.midi_out
                        .send(
                            &ChannelMessageType::PitchBendChange {
                                value: (detuning.as_semitones() / 2.0 * 8192.0) as i16,
                            }
                            .in_channel(channel)
                            .unwrap()
                            .to_raw_message(),
                        )
                        .unwrap();
                }
                tuner
            }
        };
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

    fn start(&mut self, id: S, degree: i32, _pitch: Pitch, velocity: u8) {
        if let Some(location) = self.channel_and_note_for_degree(degree) {
            match self.keypress_tracker.place_finger_at(id, location) {
                Ok(PlaceAction::KeyPressed | PlaceAction::KeyAlreadyPressed) => {
                    self.send_note_on(location, velocity);
                }
                Err(id) => eprintln!(
                    "[WARNING] location {:?} with ID {:?} released before pressed",
                    location, id
                ),
            }
        }
    }

    fn update_pitch(&mut self, id: S, degree: i32, _pitch: Pitch) {
        if let Some(location) = self.channel_and_note_for_degree(degree) {
            match self.keypress_tracker.move_finger_to(&id, location) {
                Ok((LiftAction::KeyReleased(released), _)) => {
                    self.send_note_off(released, 100);
                    self.send_note_on(location, 100);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    self.send_note_on(location, 100);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {}
            }
        }
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        if let Some(&(channel, note)) = self.keypress_tracker.location_of(&id) {
            self.send_polyphonic(
                channel,
                ChannelMessageType::PolyphonicKeyPressure {
                    key: note,
                    pressure,
                },
            );
        }
    }

    fn stop(&mut self, id: S, velocity: u8) {
        match self.keypress_tracker.lift_finger(&id) {
            Ok(LiftAction::KeyReleased(location)) => self.send_note_off(location, velocity),
            Ok(LiftAction::KeyRemainsPressed) => {}
            Err(IllegalState) => {}
        }
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

    fn has_legato(&self) -> bool {
        true
    }
}

impl<I, E: Eq + Hash + Debug> MidiOutBackend<I, E> {
    fn channel_and_note_for_degree(&self, degree: i32) -> Option<(u8, u8)> {
        if let Some((channel, note)) = self.tuner.get_channel_and_note_for_key(degree) {
            if let Some(key) = note.checked_midi_number() {
                return Some((u8::try_from(channel).unwrap(), key));
            }
        }
        None
    }

    fn send_note_on(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.send_polyphonic(
            channel,
            ChannelMessageType::NoteOn {
                key: note,
                velocity,
            },
        );
    }

    fn send_note_off(&mut self, (channel, note): (u8, u8), velocity: u8) {
        self.send_polyphonic(
            channel,
            ChannelMessageType::NoteOff {
                key: note,
                velocity,
            },
        );
    }

    fn send_monophonic(&mut self, message_type: ChannelMessageType) {
        for channel in 0..16 {
            self.midi_out
                .send(&message_type.in_channel(channel).unwrap().to_raw_message())
                .unwrap()
        }
    }

    fn send_polyphonic(&mut self, channel: u8, message_type: ChannelMessageType) {
        self.midi_out
            .send(&message_type.in_channel(channel).unwrap().to_raw_message())
            .unwrap();
    }
}

pub struct MidiInfo {
    pub device: String,
    pub program_number: u8,
}

pub fn connect_to_midi_device(
    target_port: &str,
    mut engine: Arc<PianoEngine>,
    midi_channel: u8,
    midi_logging: bool,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    shared::connect_to_in_device("microwave", target_port, move |message| {
        process_midi_event(message, &mut engine, midi_channel, midi_logging)
    })
}

fn process_midi_event(
    message: &[u8],
    engine: &mut Arc<PianoEngine>,
    input_channel: u8,
    midi_logging: bool,
) {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
        if midi_logging {
            writeln!(stderr, "[DEBUG] MIDI message received:").unwrap();
            writeln!(stderr, "{:#?}", channel_message).unwrap();
            writeln!(stderr,).unwrap();
        }
        if channel_message.channel() == input_channel {
            engine.handle_midi_event(channel_message.message_type());
        }
    } else {
        writeln!(stderr, "[WARNING] Unsupported MIDI message received:").unwrap();
        for i in message {
            writeln!(stderr, "{:08b}", i).unwrap();
        }
        writeln!(stderr).unwrap();
    }
}
