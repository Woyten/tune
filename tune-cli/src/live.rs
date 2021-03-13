#![allow(clippy::too_many_arguments)] // Valid lint but the error popped up too late s.t. this will be fixed in the future.

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
    midi::{ChannelMessage, ChannelMessageType, TransformResult},
    mts::{
        self, ScaleOctaveTuning, ScaleOctaveTuningMessage, SingleNoteTuningChange,
        SingleNoteTuningChangeMessage,
    },
    note::{Note, PitchedNote},
    pitch::Ratio,
    tuner::ChannelTuner,
};

use crate::{
    midi,
    mts::DeviceIdArg,
    pool::{Pool, PoolingMode},
    shared::MidiError,
    App, CliResult, ScaleCommand,
};

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
    /// This means each note letter (e.g. D) can be played in 3 different manifestations simultaneuously without clashes.
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
            LiveMode::JustInTime(options) => options.run(&self, app, send)?,
            LiveMode::AheadOfTime(options) => options.run(&self, app, send)?,
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
}

impl JustInTimeOptions {
    fn run(
        &self,
        options: &LiveOptions,
        app: &mut App,
        messages: Sender<Message>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        match &self.method {
            TuningMethod::FullKeyboard {
                device_id,
                tuning_program,
                scale,
            } => {
                let device_id = device_id.get()?;
                let tuning_program = *tuning_program;
                self.run_internal(
                    options,
                    app,
                    scale,
                    messages,
                    true,
                    move |channel, note, deviation| {
                        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
                            iter::once(SingleNoteTuningChange::new(
                                PianoKey::from_midi_number(note),
                                Note::from_midi_number(note).alter_pitch_by(deviation),
                            )),
                            device_id,
                            (channel + tuning_program) % 128,
                        )
                        .unwrap();
                        Message::FullKeyboardTuning(
                            channel,
                            (channel + tuning_program) % 128,
                            tuning_message,
                        )
                    },
                    |note| note,
                )
            }
            TuningMethod::Octave { device_id, scale } => {
                let mut octave_tunings = HashMap::<_, ScaleOctaveTuning>::new();
                let device_id = device_id.get()?;
                self.run_internal(
                    options,
                    app,
                    scale,
                    messages,
                    true,
                    move |channel, note, deviation| {
                        let letter = Note::from_midi_number(note).letter_and_octave().0;
                        let octave_tuning = octave_tunings.entry(usize::from(channel)).or_default();
                        *octave_tuning.as_mut(letter) = deviation;
                        let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                            octave_tuning,
                            channel,
                            device_id,
                        )
                        .unwrap();
                        Message::OctaveBasedTuning(tuning_message)
                    },
                    |note| Note::from_midi_number(note).letter_and_octave().0,
                )
            }
            TuningMethod::ChannelFineTuning { scale } => self.run_internal(
                options,
                app,
                scale,
                messages,
                true,
                |channel, _, deviation| Message::ChannelBasedTuning(channel, deviation),
                |_| (),
            ),
            TuningMethod::PitchBend { scale } => self.run_internal(
                options,
                app,
                scale,
                messages,
                false,
                |channel, _, deviation| Message::PitchBend(channel, deviation),
                |_| (),
            ),
        }
    }

    fn run_internal<N: Eq + Hash + Copy + Send + 'static>(
        &self,
        options: &LiveOptions,
        app: &mut App,
        scale: &ScaleCommand,
        messages: Sender<Message>,
        accept_pitch_bend_messages: bool,
        mut tuning_message: impl FnMut(u8, u8, Ratio) -> Message + Send + 'static,
        mut group: impl FnMut(u8) -> N + Send + 'static,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        validate_channels(options, usize::from(self.num_out_channels))?;

        let tuning = scale.to_scale(app)?.tuning;
        let channel_range = options.out_channel
            ..options
                .out_channel
                .saturating_add(self.num_out_channels)
                .min(16);

        let mut pools = HashMap::new();
        let pooling_mode = self.clash_mitigation;

        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
            accept_pitch_bend_messages,
            move |original_message| match original_message.message_type().transform(&*tuning) {
                TransformResult::Transformed {
                    message_type,
                    orig_key,
                    mapped_note,
                    deviation,
                } => {
                    let pool = pools
                        .entry(group(mapped_note))
                        .or_insert_with(|| Pool::new(pooling_mode, channel_range.clone()));
                    let channel_to_use = match message_type {
                        ChannelMessageType::NoteOn { velocity, .. } => {
                            let result = pool.key_pressed(orig_key, mapped_note);

                            if let Some((free_channel, note_to_stop)) = result {
                                if let Some(note_to_stop) = note_to_stop {
                                    let note_off_message = ChannelMessageType::NoteOff {
                                        key: note_to_stop,
                                        velocity,
                                    }
                                    .in_channel(free_channel)
                                    .unwrap();

                                    messages
                                        .send(Message::Generic(note_off_message))
                                        .unwrap_or_default();
                                }
                                messages
                                    .send(tuning_message(free_channel, mapped_note, deviation))
                                    .unwrap();
                            }

                            result.map(|(channel, _)| channel)
                        }
                        ChannelMessageType::NoteOff { .. } => pool.key_released(&orig_key),
                        ChannelMessageType::PolyphonicKeyPressure { .. } => {
                            pool.channel_for_key(&orig_key)
                        }
                        _ => None,
                    };

                    if let Some(channel_to_use) = channel_to_use {
                        let message_with_correct_channel =
                            message_type.in_channel(channel_to_use).unwrap();

                        messages
                            .send(Message::Generic(message_with_correct_channel))
                            .unwrap();
                    }
                }
                TransformResult::NotKeyBased => {
                    for channel in channel_range.clone() {
                        messages
                            .send(Message::Generic(
                                original_message.message_type().in_channel(channel).unwrap(),
                            ))
                            .unwrap();
                    }
                }
                TransformResult::NoteOutOfRange => {}
            },
        )
        .map(|result| (usize::from(self.num_out_channels), result))
        .map_err(Into::into)
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        options: &LiveOptions,
        app: &mut App,
        messages: Sender<Message>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        match &self.method {
            TuningMethod::FullKeyboard {
                device_id,
                tuning_program,
                scale,
            } => {
                let scale = scale.to_scale(app)?;
                let device_id = device_id.get()?;
                self.run_internal(
                    options,
                    messages,
                    true,
                    ChannelTuner::apply_full_keyboard_tuning(&*scale.tuning, scale.keys),
                    |channel, channel_tuning| {
                        channel_tuning
                            .to_mts_format(device_id, (channel + tuning_program) % 128)
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
                let device_id = device_id.get()?;
                self.run_internal(
                    options,
                    messages,
                    true,
                    ChannelTuner::apply_octave_based_tuning(&*scale.tuning, scale.keys),
                    |channel, channel_tuning| {
                        channel_tuning
                            .to_mts_format(device_id, channel)
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
        mut map_message: impl FnMut(u8, &T) -> Result<Message, String>,
    ) -> CliResult<(usize, (String, MidiInputConnection<()>))> {
        validate_channels(options, channel_tunings.len())?;

        for (channel_tuning, channel) in channel_tunings.iter().zip(0..16) {
            messages
                .send(map_message(channel, channel_tuning)?)
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

fn validate_channels(options: &LiveOptions, num_channels: usize) -> CliResult<()> {
    Err(if options.in_channel >= 16 {
        "Input channel is not in the range [0..16)".to_owned()
    } else if options.out_channel >= 16 {
        "Output channel is not in the range [0..16)".to_owned()
    } else if num_channels + usize::from(options.out_channel) > 16 {
        format!(
            "The tuning method requires {} output channels but the number of available channels is {}. Try lowering the output channel number.",
            num_channels,
            16 - options.out_channel
        )
    } else {
        return Ok(());
    }
    .into())
}
