use std::{
    collections::HashMap,
    hash::Hash,
    iter, mem,
    sync::mpsc::{self, Sender},
};

use midir::MidiInputConnection;
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    mts::{
        self, ScaleOctaveTuning, ScaleOctaveTuningMessage, ScaleOctaveTuningOptions,
        SingleNoteTuningChange, SingleNoteTuningChangeMessage, SingleNoteTuningChangeOptions,
    },
    note::Note,
    pitch::{Pitched, Ratio},
    tuner::{
        AccessKeyResult, ChannelTuner, GroupBy, GroupByChannel, GroupByNote, GroupByNoteLetter,
        JitTuner, PoolingMode, RegisterKeyResult,
    },
    tuning::KeyboardMapping,
};

use crate::{midi, mts::DeviceIdArg, shared::MidiError, App, CliResult, ScaleCommand};

#[derive(StructOpt)]
pub(crate) struct LiveOptions {
    /// MIDI input device
    #[structopt(long = "midi-in")]
    midi_in_device: String,

    /// MIDI output device
    #[structopt(long = "midi-out")]
    midi_out_device: String,

    /// MIDI channel to listen to
    #[structopt(long = "in-chan", default_value = "0")]
    in_channel: u8,

    /// Lowest MIDI channel to send the modified MIDI events to
    #[structopt(long = "out-chan", default_value = "0")]
    out_channel: u8,

    #[structopt(subcommand)]
    mode: LiveMode,
}

#[derive(StructOpt)]
enum LiveMode {
    /// Just-in-time: Tracks which notes are active and injects tuning messages into the stream of MIDI events.
    /// This mode uses a dynamic key-to-channel mapping to avoid tuning clashes.
    /// The number of output channels can be selected by the user and can be set to a small number.
    /// When tuning clashes occur several mitigation strategies can be applied.
    #[structopt(name = "jit")]
    JustInTime(JustInTimeOptions),

    /// Ahead-of-time: Sends all necessary tuning messages at startup.
    /// The key-to-channel mapping is fixed and eliminates tuning clashes s.t. this mode offers the highest degree of musical freedom.
    /// On the downside, the number of output channels cannot be changed by the user and might be a large number.
    #[structopt(name = "aot")]
    AheadOfTime(AheadOfTimeOptions),
}

#[derive(StructOpt)]
struct JustInTimeOptions {
    /// Number of MIDI output channels that should be retuned.
    /// A reasonable number for the octave-based tuning method is 3.
    /// This means each note letter (e.g. D) can be played in 3 different manifestations simultaneously without clashes.
    #[structopt(long = "out-chans", default_value = "3")]
    num_out_channels: u8,

    /// Describes what to do when a note is triggered that cannot be handled by any channel without tuning clashes.
    /// [block] Do not accept the new note. It will remain silent.
    /// [stop] Stop an old note and accept the new note.
    /// [ignore] Neither block nor stop. Accept that an old note receives an arbitrary tuning update.
    #[structopt(long = "clash", default_value = "stop", parse(try_from_str = parse_mitigation))]
    clash_mitigation: PoolingMode,

    #[structopt[subcommand]]
    method: TuningMethod,
}

fn parse_mitigation(src: &str) -> Result<PoolingMode, &'static str> {
    Ok(match &*src.to_lowercase() {
        "block" => PoolingMode::Block,
        "stop" => PoolingMode::Stop,
        "ignore" => PoolingMode::Ignore,
        _ => return Err("Invalid mode. Should be `veto`, `stop` or `ignore`"),
    })
}

#[derive(StructOpt)]
struct AheadOfTimeOptions {
    #[structopt[subcommand]]
    method: TuningMethod,
}

#[derive(StructOpt)]
enum TuningMethod {
    /// Retune channels via Single Note Tuning Change messages. Each channel can handle at most one detuning per note.
    #[structopt(name = "full")]
    FullKeyboard {
        #[structopt(flatten)]
        device_id: DeviceIdArg,

        /// Lowest tuning program to be used to store the tuning information per channel. Each note is detuned by 50c at most.
        #[structopt(long = "tun-pg", default_value = "0")]
        tuning_program: u8,

        #[structopt(subcommand)]
        scale: ScaleCommand,
    },
    /// Retune channels via Scale/Octave Tuning (1 byte format) messages. Each channel can handle at most one detuning per note letter.
    #[structopt(name = "octave")]
    Octave {
        #[structopt(flatten)]
        device_id: DeviceIdArg,

        #[structopt[subcommand]]
        scale: ScaleCommand,
    },
    /// Retune channels via Channel Fine Tuning messages. Each channel can handle at most one detuning.
    #[structopt(name = "channel")]
    ChannelFineTuning {
        #[structopt[subcommand]]
        scale: ScaleCommand,
    },
    /// Retune channels via pitch-bend messages. Each channel can handle at most one detuning.
    #[structopt(name = "pitch-bend")]
    PitchBend {
        #[structopt[subcommand]]
        scale: ScaleCommand,
    },
}

impl LiveOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let (send, recv) = mpsc::channel();

        let (num_channels, (in_device, in_connection)) = match &self.mode {
            LiveMode::JustInTime(options) => options.run(app, self, send)?,
            LiveMode::AheadOfTime(options) => options.run(app, self, send)?,
        };

        let (out_device, mut out_connection) = midi::connect_to_out_device(&self.midi_out_device)?;

        app.writeln(format_args!("Receiving MIDI data from {}", in_device))?;
        app.writeln(format_args!("Sending MIDI data to {}", out_device))?;
        app.writeln(format_args!(
            "in-channel {} -> out-channels [{}..{})",
            self.in_channel,
            self.out_channel,
            usize::from(self.out_channel) + num_channels
        ))?;

        for message in recv {
            match message {
                Message::Generic(channel) => out_connection.send(&channel.to_raw_message()),
                Message::FullKeyboardTuning(channel, tuning_program, tuning) => {
                    mts::tuning_program_change(channel, tuning_program)
                        .iter()
                        .flatten()
                        .try_for_each(|message| out_connection.send(&message.to_raw_message()))
                        .and_then(|_| {
                            tuning
                                .sysex_bytes()
                                .try_for_each(|message| out_connection.send(message))
                        })
                }
                Message::OctaveBasedTuning(tuning) => out_connection.send(tuning.sysex_bytes()),
                Message::ChannelBasedTuning(channel, detune) => {
                    mts::channel_fine_tuning(channel, detune)
                        .unwrap()
                        .iter()
                        .try_for_each(|message| out_connection.send(&message.to_raw_message()))
                }
                Message::PitchBend(channel, detune) => out_connection.send(
                    &ChannelMessageType::PitchBendChange {
                        value: (detune.as_semitones() / 2.0 * 8192.0) as i16,
                    }
                    .in_channel(channel)
                    .unwrap()
                    .to_raw_message(),
                ),
            }
            .unwrap()
        }

        mem::drop(in_connection);

        Ok(())
    }

    fn validate_channels(&self, num_channels: usize) -> CliResult<()> {
        Err(if self.in_channel >= 16 {
            "Input channel is not in the range [0..16)".to_owned()
        } else if self.out_channel >= 16 {
            "Output channel is not in the range [0..16)".to_owned()
        } else if usize::from(self.out_channel).saturating_add(num_channels) > 16 {
            format!(
                "The tuning method requires {} output channels but the number of available channels is {}. Try lowering the output channel number.",
                num_channels,
                16 - self.out_channel
            )
        } else {
            return Ok(());
        }
        .into())
    }
}

impl JustInTimeOptions {
    fn run(
        &self,
        app: &mut App,
        options: &LiveOptions,
        messages: Sender<Message>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        let num_channels = usize::from(self.num_out_channels);

        match &self.method {
            TuningMethod::FullKeyboard {
                device_id,
                tuning_program,
                scale,
            } => {
                let jit_tuner = JitTuner::new(GroupByNote, self.clash_mitigation, num_channels);
                let tuning = scale.to_scale(app)?.tuning;
                let to_tuning_message = ToSingleNoteTuningMessage {
                    device_id: device_id.device_id,
                    tuning_program_start: *tuning_program,
                };

                self.run_internal(
                    options,
                    messages,
                    true,
                    jit_tuner,
                    tuning,
                    to_tuning_message,
                )
            }
            TuningMethod::Octave { device_id, scale } => {
                let jit_tuner =
                    JitTuner::new(GroupByNoteLetter, self.clash_mitigation, num_channels);
                let tuning = scale.to_scale(app)?.tuning;
                let to_tuning_message = ToScaleOctaveTuningMessage {
                    device_id: device_id.device_id,
                    octave_tunings: HashMap::new(),
                };

                self.run_internal(
                    options,
                    messages,
                    true,
                    jit_tuner,
                    tuning,
                    to_tuning_message,
                )
            }
            TuningMethod::ChannelFineTuning { scale } => {
                let jit_tuner = JitTuner::new(GroupByChannel, self.clash_mitigation, num_channels);
                let tuning = scale.to_scale(app)?.tuning;
                let to_tuning_message = ToChannelFineTuningMessage {};

                self.run_internal(
                    options,
                    messages,
                    true,
                    jit_tuner,
                    tuning,
                    to_tuning_message,
                )
            }
            TuningMethod::PitchBend { scale } => {
                let jit_tuner = JitTuner::new(GroupByChannel, self.clash_mitigation, num_channels);
                let tuning = scale.to_scale(app)?.tuning;
                let to_tuning_message = ToPitchBendMessage {};

                self.run_internal(
                    options,
                    messages,
                    false,
                    jit_tuner,
                    tuning,
                    to_tuning_message,
                )
            }
        }
    }

    fn run_internal<G>(
        &self,
        options: &LiveOptions,
        messages: Sender<Message>,
        accept_pitch_bend_messages: bool,
        mut jit_tuner: JitTuner<u8, G>,
        tuning: Box<dyn KeyboardMapping<PianoKey> + Send>,
        mut to_tuning_message: impl ToTuningMessage + Send + 'static,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))>
    where
        G: GroupBy + Send + 'static,
        G::Group: Eq + Hash + Copy + Send,
    {
        options.validate_channels(usize::from(self.num_out_channels))?;

        let out_channel = options.out_channel;
        let channel_range = options.out_channel..options.out_channel + self.num_out_channels;

        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
            accept_pitch_bend_messages,
            move |original_message| match original_message.message_type() {
                ChannelMessageType::NoteOff { key, velocity } => {
                    match jit_tuner.deregister_key(&key) {
                        AccessKeyResult::Found {
                            channel,
                            found_note,
                        } => {
                            let channel = u8::try_from(channel).unwrap() + out_channel;
                            if let Some(found_note) = found_note.checked_midi_number() {
                                messages
                                    .send(Message::Generic(
                                        ChannelMessageType::NoteOff {
                                            key: found_note,
                                            velocity,
                                        }
                                        .in_channel(channel)
                                        .unwrap(),
                                    ))
                                    .unwrap();
                            }
                        }
                        AccessKeyResult::NotFound => {}
                    }
                }
                ChannelMessageType::NoteOn { key, velocity } => {
                    if let Some(pitch) = tuning.maybe_pitch_of(PianoKey::from_midi_number(key)) {
                        match jit_tuner.register_key(key, pitch) {
                            RegisterKeyResult::Accepted {
                                channel,
                                stopped_note,
                                started_note,
                                detuning,
                            } => {
                                let channel = u8::try_from(channel).unwrap() + out_channel;
                                if let Some(stopped_note) =
                                    stopped_note.and_then(Note::checked_midi_number)
                                {
                                    messages
                                        .send(Message::Generic(
                                            ChannelMessageType::NoteOff {
                                                key: stopped_note,
                                                velocity,
                                            }
                                            .in_channel(channel)
                                            .unwrap(),
                                        ))
                                        .unwrap();
                                }
                                if let Some(started_note) = started_note.checked_midi_number() {
                                    messages
                                        .send(to_tuning_message.create_tuning_message(
                                            channel,
                                            started_note,
                                            detuning,
                                        ))
                                        .unwrap();
                                    messages
                                        .send(Message::Generic(
                                            ChannelMessageType::NoteOn {
                                                key: started_note,
                                                velocity,
                                            }
                                            .in_channel(channel)
                                            .unwrap(),
                                        ))
                                        .unwrap();
                                }
                            }
                            RegisterKeyResult::Rejected => {}
                        }
                    }
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    match jit_tuner.access_key(&key) {
                        AccessKeyResult::Found {
                            channel,
                            found_note,
                        } => {
                            let channel = u8::try_from(channel).unwrap() + out_channel;
                            if let Some(found_note) = found_note.checked_midi_number() {
                                messages
                                    .send(Message::Generic(
                                        ChannelMessageType::PolyphonicKeyPressure {
                                            key: found_note,
                                            pressure,
                                        }
                                        .in_channel(channel)
                                        .unwrap(),
                                    ))
                                    .unwrap();
                            }
                        }
                        AccessKeyResult::NotFound => {}
                    }
                }
                ChannelMessageType::ControlChange { .. }
                | ChannelMessageType::ProgramChange { .. }
                | ChannelMessageType::ChannelPressure { .. }
                | ChannelMessageType::PitchBendChange { .. } => {
                    for channel in channel_range.clone() {
                        messages
                            .send(Message::Generic(
                                original_message.message_type().in_channel(channel).unwrap(),
                            ))
                            .unwrap();
                    }
                }
            },
        )
        .map(|result| (usize::from(self.num_out_channels), result))
        .map_err(Into::into)
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        app: &mut App,
        options: &LiveOptions,
        messages: Sender<Message>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        match &self.method {
            TuningMethod::FullKeyboard {
                device_id,
                tuning_program,
                scale,
            } => {
                let scale = scale.to_scale(app)?;
                self.run_internal(
                    options,
                    messages,
                    true,
                    ChannelTuner::apply_full_keyboard_tuning(&*scale.tuning, scale.keys),
                    |channel, channel_tuning| {
                        let options = SingleNoteTuningChangeOptions {
                            device_id: device_id.device_id,
                            tuning_program: (channel + tuning_program) % 128,
                            ..Default::default()
                        };
                        channel_tuning
                            .to_mts_format(&options)
                            .map(|tuning_message| {
                                Message::FullKeyboardTuning(
                                    channel,
                                    (channel + tuning_program) % 128,
                                    tuning_message,
                                )
                            })
                            .map_err(|err| {
                                format!("Could not apply full keyboard tuning ({:?})", err)
                            })
                    },
                )
            }
            TuningMethod::Octave { device_id, scale } => {
                let scale = scale.to_scale(app)?;
                self.run_internal(
                    options,
                    messages,
                    true,
                    ChannelTuner::apply_octave_based_tuning(&*scale.tuning, scale.keys),
                    |channel, channel_tuning| {
                        let options = ScaleOctaveTuningOptions {
                            device_id: device_id.device_id,
                            channels: channel.into(),
                            ..Default::default()
                        };
                        channel_tuning
                            .to_mts_format(&options)
                            .map(Message::OctaveBasedTuning)
                            .map_err(|err| {
                                format!("Could not apply octave based tuning ({:?})", err)
                            })
                    },
                )
            }
            TuningMethod::ChannelFineTuning { scale } => {
                let scale = scale.to_scale(app)?;
                self.run_internal(
                    options,
                    messages,
                    true,
                    ChannelTuner::apply_channel_based_tuning(&*scale.tuning, scale.keys),
                    |channel, &ratio| Ok(Message::ChannelBasedTuning(channel, ratio)),
                )
            }
            TuningMethod::PitchBend { scale } => {
                let scale = scale.to_scale(app)?;
                self.run_internal(
                    options,
                    messages,
                    false,
                    ChannelTuner::apply_channel_based_tuning(&*scale.tuning, scale.keys),
                    |channel, &ratio| Ok(Message::PitchBend(channel, ratio)),
                )
            }
        }
    }

    fn run_internal<T>(
        &self,
        options: &LiveOptions,
        messages: Sender<Message>,
        accept_pitch_bend_messages: bool,
        (tuner, channel_tunings): (ChannelTuner<PianoKey>, Vec<T>),
        mut to_tuning_message: impl FnMut(u8, &T) -> Result<Message, String>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        options.validate_channels(channel_tunings.len())?;

        for (channel_tuning, channel) in channel_tunings.iter().zip(0..) {
            messages
                .send(to_tuning_message(channel, channel_tuning)?)
                .unwrap();
        }

        let out_channel = options.out_channel;
        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
            accept_pitch_bend_messages,
            move |original_message| {
                for message in original_message
                    .message_type()
                    .distribute(&tuner, out_channel)
                {
                    messages.send(Message::Generic(message)).unwrap();
                }
            },
        )
        .map(|result| (channel_tunings.len(), result))
        .map_err(Into::into)
    }
}

trait ToTuningMessage {
    fn create_tuning_message(&mut self, channel: u8, note: u8, deviation: Ratio) -> Message;
}

struct ToSingleNoteTuningMessage {
    device_id: u8,
    tuning_program_start: u8,
}

impl ToTuningMessage for ToSingleNoteTuningMessage {
    fn create_tuning_message(&mut self, channel: u8, note: u8, deviation: Ratio) -> Message {
        let tuning_program = (channel + self.tuning_program_start) % 128;
        let options = SingleNoteTuningChangeOptions {
            device_id: self.device_id,
            tuning_program,
            ..Default::default()
        };
        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            &options,
            iter::once(SingleNoteTuningChange {
                key: PianoKey::from_midi_number(note),
                target_pitch: Note::from_midi_number(note).pitch() * deviation,
            }),
        )
        .unwrap();

        Message::FullKeyboardTuning(channel, tuning_program, tuning_message)
    }
}

struct ToScaleOctaveTuningMessage {
    device_id: u8,
    octave_tunings: HashMap<usize, ScaleOctaveTuning>,
}

impl ToTuningMessage for ToScaleOctaveTuningMessage {
    fn create_tuning_message(&mut self, channel: u8, note: u8, deviation: Ratio) -> Message {
        let letter = Note::from_midi_number(note).letter_and_octave().0;
        let octave_tuning = self.octave_tunings.entry(usize::from(channel)).or_default();
        *octave_tuning.as_mut(letter) = deviation;
        let options = ScaleOctaveTuningOptions {
            device_id: self.device_id,
            channels: channel.into(),
            ..Default::default()
        };
        let tuning_message =
            ScaleOctaveTuningMessage::from_octave_tuning(&options, octave_tuning).unwrap();

        Message::OctaveBasedTuning(tuning_message)
    }
}

struct ToChannelFineTuningMessage;

impl ToTuningMessage for ToChannelFineTuningMessage {
    fn create_tuning_message(&mut self, channel: u8, _note: u8, deviation: Ratio) -> Message {
        Message::ChannelBasedTuning(channel, deviation)
    }
}

struct ToPitchBendMessage {}

impl ToTuningMessage for ToPitchBendMessage {
    fn create_tuning_message(&mut self, channel: u8, _note: u8, deviation: Ratio) -> Message {
        Message::PitchBend(channel, deviation)
    }
}

#[derive(Debug)]
enum Message {
    Generic(ChannelMessage),
    FullKeyboardTuning(u8, u8, SingleNoteTuningChangeMessage),
    OctaveBasedTuning(ScaleOctaveTuningMessage),
    ChannelBasedTuning(u8, Ratio),
    PitchBend(u8, Ratio),
}

fn connect_to_in_device(
    target_port: &str,
    in_channel: u8,
    accept_pitch_bend_messages: bool,
    mut callback: impl FnMut(ChannelMessage) + Send + 'static,
) -> Result<(String, MidiInputConnection<()>), MidiError> {
    midi::connect_to_in_device(target_port, move |raw_message| {
        if let Some(parsed_message) = ChannelMessage::from_raw_message(raw_message) {
            if parsed_message.channel() == in_channel
                && (accept_pitch_bend_messages
                    || !matches!(
                        parsed_message.message_type(),
                        ChannelMessageType::PitchBendChange { .. }
                    ))
            {
                callback(parsed_message)
            }
        }
    })
}
