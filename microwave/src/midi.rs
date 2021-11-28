use std::{
    fmt::Debug,
    hash::Hash,
    io::Write,
    mem,
    sync::{mpsc::Sender, Arc},
};

use midir::{MidiInputConnection, MidiOutputConnection};
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    mts::ScaleOctaveTuningFormat,
    note::Note,
    pitch::{Pitch, Pitched},
    scala::{KbmRoot, Scl},
    tuner::{
        AotMidiTuner, JitMidiTuner, MidiTarget, MidiTunerMessage, MidiTunerMessageHandler,
        PoolingMode,
    },
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

pub struct MidiOutBackend<I, S> {
    info_sender: Sender<I>,
    device: String,
    tuning_method: TuningMethod,
    curr_program: u8,
    tuner: MidiTuner<S>,
}

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    target_port: &str,
    tuning_method: TuningMethod,
) -> CliResult<MidiOutBackend<I, S>> {
    let (device, midi_out) = shared::connect_to_out_device("microwave", target_port)?;

    Ok(MidiOutBackend {
        info_sender,
        device,
        tuning_method,
        curr_program: 0,
        tuner: MidiTuner::None {
            target: MidiTarget {
                handler: MidiOutHandler { midi_out },
                first_channel: 0,
                num_channels: 9,
            },
        },
    })
}

enum MidiTuner<S> {
    Destroyed,
    None {
        target: MidiTarget<MidiOutHandler>,
    },
    Jit {
        jit_tuner: JitMidiTuner<S, MidiOutHandler>,
    },
    Aot {
        aot_tuner: AotMidiTuner<i32, MidiOutHandler>,
        keypress_tracker: KeypressTracker<S, i32>,
    },
}

pub enum TuningMethod {
    FullKeyboard(bool),
    Octave1(bool),
    Octave2(bool),
    ChannelFineTuning,
    PitchBend,
}

struct MidiOutHandler {
    midi_out: MidiOutputConnection,
}

impl MidiTunerMessageHandler for MidiOutHandler {
    fn handle(&mut self, message: MidiTunerMessage) {
        message.send_to(|m| self.midi_out.send(m).unwrap());
    }
}

impl<I: From<MidiInfo> + Send, S: Copy + Eq + Hash + Debug + Send> Backend<S>
    for MidiOutBackend<I, S>
{
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        let target = self.destroy_tuning();

        let lowest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(-1).pitch())
            .approx_value;

        let highest_key = tuning
            .find_by_pitch_sorted(Note::from_midi_number(128).pitch())
            .approx_value;

        let tuning = tuning.as_sorted_tuning().as_linear_mapping();
        let keys = lowest_key..highest_key;

        let device_id = 0x7f;
        let first_tuning_program = 0;
        let aot_tuner = match self.tuning_method {
            TuningMethod::FullKeyboard(realtime) => AotMidiTuner::single_note_tuning_change(
                target,
                tuning,
                keys,
                realtime,
                device_id,
                first_tuning_program,
            ),
            TuningMethod::Octave1(realtime) => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                realtime,
                device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2(realtime) => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                realtime,
                device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                AotMidiTuner::channel_fine_tuning(target, tuning, keys)
            }
            TuningMethod::PitchBend => AotMidiTuner::pitch_bend(target, tuning, keys),
        };

        self.tuner = match aot_tuner {
            Ok(aot_tuner) => MidiTuner::Aot {
                aot_tuner,
                keypress_tracker: KeypressTracker::new(),
            },
            Err((target, num_required_channels)) => {
                eprintln!(
                    "[WARNING] Cannot apply tuning. The tuning requires {} channels",
                    num_required_channels
                );
                MidiTuner::None { target }
            }
        }
    }

    fn set_no_tuning(&mut self) {
        let target = self.destroy_tuning();

        let pooling_mode = PoolingMode::Stop;
        let device_id = 0x7f;
        let first_tuning_program = 0;

        let jit_tuner = match self.tuning_method {
            TuningMethod::FullKeyboard(realtime) => JitMidiTuner::single_note_tuning_change(
                target,
                pooling_mode,
                realtime,
                device_id,
                first_tuning_program,
            ),
            TuningMethod::Octave1(realtime) => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                realtime,
                device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2(realtime) => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                realtime,
                device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                JitMidiTuner::channel_fine_tuning(target, pooling_mode)
            }
            TuningMethod::PitchBend => JitMidiTuner::pitch_bend(target, pooling_mode),
        };

        self.tuner = MidiTuner::Jit { jit_tuner }
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

    fn start(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        match &mut self.tuner {
            MidiTuner::Destroyed | MidiTuner::None { .. } => {}
            MidiTuner::Jit { jit_tuner } => {
                jit_tuner.note_on(id, pitch, velocity);
            }
            MidiTuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.place_finger_at(id, degree) {
                Ok(PlaceAction::KeyPressed) => {
                    aot_tuner.note_on(degree, velocity);
                }
                Ok(PlaceAction::KeyAlreadyPressed) => {
                    aot_tuner.note_off(degree, velocity);
                    aot_tuner.note_on(degree, velocity);
                }
                Err(id) => {
                    eprintln!(
                        "[WARNING] Key with ID {:?} not lifted before pressed again",
                        id,
                    );
                }
            },
        }
    }

    fn update_pitch(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        match &mut self.tuner {
            MidiTuner::Destroyed | MidiTuner::None { .. } => {}
            MidiTuner::Jit { jit_tuner } => {
                jit_tuner.update_pitch(&id, pitch);
            }
            MidiTuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.move_finger_to(&id, degree) {
                Ok((LiftAction::KeyReleased(released), _)) => {
                    aot_tuner.note_off(released, velocity);
                    aot_tuner.note_on(degree, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyPressed)) => {
                    aot_tuner.note_on(degree, velocity);
                }
                Ok((LiftAction::KeyRemainsPressed, PlaceAction::KeyAlreadyPressed)) => {}
                Err(IllegalState) => {}
            },
        }
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        match &mut self.tuner {
            MidiTuner::Destroyed | MidiTuner::None { .. } => {}
            MidiTuner::Jit { jit_tuner } => {
                jit_tuner.key_pressure(&id, pressure);
            }
            MidiTuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => {
                if let Some(&location) = keypress_tracker.location_of(&id) {
                    aot_tuner.key_pressure(location, pressure);
                }
            }
        }
    }

    fn stop(&mut self, id: S, velocity: u8) {
        match &mut self.tuner {
            MidiTuner::Destroyed | MidiTuner::None { .. } => {}
            MidiTuner::Jit { jit_tuner } => {
                jit_tuner.note_off(&id, velocity);
            }
            MidiTuner::Aot {
                keypress_tracker,
                aot_tuner,
            } => match keypress_tracker.lift_finger(&id) {
                Ok(LiftAction::KeyReleased(location)) => aot_tuner.note_off(location, velocity),
                Ok(LiftAction::KeyRemainsPressed) => {}
                Err(IllegalState) => {}
            },
        }
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_program =
            u8::try_from(update_fn(usize::from(self.curr_program) + 128) % 128).unwrap();

        self.send_monophonic_message(ChannelMessageType::ProgramChange {
            program: self.curr_program,
        });

        self.send_status();
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.send_monophonic_message(ChannelMessageType::ControlChange { controller, value });
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.send_monophonic_message(ChannelMessageType::ChannelPressure { pressure });
    }

    fn pitch_bend(&mut self, value: i16) {
        self.send_monophonic_message(ChannelMessageType::PitchBendChange { value });
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        true
    }
}

impl<I, S: Copy + Eq + Hash> MidiOutBackend<I, S> {
    fn destroy_tuning(&mut self) -> MidiTarget<MidiOutHandler> {
        let mut tuner = MidiTuner::Destroyed;
        mem::swap(&mut tuner, &mut self.tuner);

        match tuner {
            MidiTuner::None { target } => target,
            MidiTuner::Jit { jit_tuner } => jit_tuner.destroy(),
            MidiTuner::Aot {
                mut aot_tuner,
                keypress_tracker,
            } => {
                for pressed_key in keypress_tracker.pressed_locations() {
                    aot_tuner.note_off(pressed_key, 0);
                }
                aot_tuner.destroy()
            }
            MidiTuner::Destroyed => unreachable!("Tuning already destroyed"),
        }
    }

    fn send_monophonic_message(&mut self, message_type: ChannelMessageType) {
        match &mut self.tuner {
            MidiTuner::None { .. } => {}
            MidiTuner::Jit { jit_tuner } => {
                jit_tuner.send_monophonic_message(message_type);
            }
            MidiTuner::Aot { aot_tuner, .. } => {
                aot_tuner.send_monophonic_message(message_type);
            }
            MidiTuner::Destroyed => {}
        }
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
