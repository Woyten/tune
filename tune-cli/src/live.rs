use std::{
    hash::Hash,
    mem,
    sync::mpsc::{self, Sender},
};

use midir::MidiInputConnection;
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    mts::{
        ScaleOctaveTuningMessage, ScaleOctaveTuningOptions, SingleNoteTuningChangeMessage,
        SingleNoteTuningChangeOptions,
    },
    pitch::Ratio,
    tuner::{AotTuner, Group, JitMidiTuner, MidiTunerMessageHandler, PoolingMode},
    tuning::KeyboardMapping,
};

use crate::{midi, mts::DeviceIdArg, App, CliResult, ScaleCommand};

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

    /// First MIDI channel to send the modified MIDI events to
    #[structopt(long = "out-chan", default_value = "0")]
    out_channel: u8,

    /// Number of MIDI output channels that should be retuned.
    /// Wraps around at zero-based channel number 15.
    /// For example --out-chan=10 and --out-chans=15 uses all channels but the drum channel.
    #[structopt(long = "out-chans", default_value = "9")]
    num_out_channels: u8,

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
        _ => return Err("Invalid mode. Should be `block`, `stop` or `ignore`"),
    })
}

#[derive(StructOpt)]
struct AheadOfTimeOptions {
    #[structopt(subcommand)]
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
        self.validate_channels()?;

        let (send, recv) = mpsc::channel();
        let handler = move |message| send.send(message).unwrap();

        let (in_device, in_connection) = match &self.mode {
            LiveMode::JustInTime(options) => options.run(app, self, handler)?,
            LiveMode::AheadOfTime(options) => options.run(app, self, todo!())?,
        };

        let (out_device, mut out_connection) = midi::connect_to_out_device(&self.midi_out_device)?;

        app.writeln(format_args!("Receiving MIDI data from {}", in_device))?;
        app.writeln(format_args!("Sending MIDI data to {}", out_device))?;
        app.writeln(format_args!(
            "in-channel {} -> out-channels {{{}}}",
            self.in_channel,
            (0..self.num_out_channels)
                .map(|c| (self.out_channel + c) % 16)
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

        for message in recv {
            message.send_to(|message| out_connection.send(message).unwrap());
        }

        mem::drop(in_connection);

        Ok(())
    }

    fn validate_channels(&self) -> CliResult<()> {
        Err(if self.num_out_channels > 16 {
            "Cannot use more than 16 channels"
        } else if self.in_channel >= 16 {
            "Input channel is not in the range [0..16)"
        } else if self.out_channel >= 16 {
            "Output channel is not in the range [0..16)"
        } else {
            return Ok(());
        }
        .to_owned()
        .into())
    }
}

impl JustInTimeOptions {
    fn run(
        &self,
        app: &mut App,
        options: &LiveOptions,
        handler: impl MidiTunerMessageHandler + Send + 'static,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        match &self.method {
            TuningMethod::FullKeyboard {
                device_id,
                tuning_program,
                scale,
            } => {
                let tuner = JitMidiTuner::single_note_tuning_change(
                    handler,
                    options.out_channel,
                    options.num_out_channels,
                    self.clash_mitigation,
                    device_id.device_id,
                    *tuning_program,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, true, options)
            }
            TuningMethod::Octave { device_id, scale } => {
                let tuner = JitMidiTuner::scale_octave_tuning(
                    handler,
                    options.out_channel,
                    options.num_out_channels,
                    self.clash_mitigation,
                    device_id.device_id,
                    tune::mts::ScaleOctaveTuningFormat::OneByte,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, true, options)
            }
            TuningMethod::ChannelFineTuning { scale } => {
                let tuner = JitMidiTuner::channel_fine_tuning(
                    handler,
                    options.out_channel,
                    options.num_out_channels,
                    self.clash_mitigation,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, true, options)
            }
            TuningMethod::PitchBend { scale } => {
                let tuner = JitMidiTuner::pitch_bend(
                    handler,
                    options.out_channel,
                    options.num_out_channels,
                    self.clash_mitigation,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, false, options)
            }
        }
    }

    fn run_internal<
        G: Group + Copy + Eq + Hash + Send + 'static,
        H: MidiTunerMessageHandler + Send + 'static,
    >(
        &self,
        mut tuner: JitMidiTuner<u8, G, H>,
        tuning: Box<dyn KeyboardMapping<PianoKey> + Send>,
        accept_pitch_bend_messages: bool,
        options: &LiveOptions,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
            accept_pitch_bend_messages,
            move |message| match message.message_type() {
                ChannelMessageType::NoteOff { key, velocity } => {
                    tuner.note_off(&key, velocity);
                }
                ChannelMessageType::NoteOn { key, velocity } => {
                    if let Some(pitch) = tuning.maybe_pitch_of(PianoKey::from_midi_number(key)) {
                        tuner.note_on(key, pitch, velocity);
                    }
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    tuner.key_pressure(&key, pressure);
                }
                message_type @ (ChannelMessageType::ControlChange { .. }
                | ChannelMessageType::ProgramChange { .. }
                | ChannelMessageType::ChannelPressure { .. }
                | ChannelMessageType::PitchBendChange { .. }) => {
                    tuner.send_monophonic_message(message_type);
                }
            },
        )
        .map_err(Into::into)
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        app: &mut App,
        options: &LiveOptions,
        messages: Sender<Message>,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
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
                    AotTuner::apply_full_keyboard_tuning(&*scale.tuning, scale.keys),
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
                    AotTuner::apply_octave_based_tuning(&*scale.tuning, scale.keys),
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
                    AotTuner::apply_channel_based_tuning(&*scale.tuning, scale.keys),
                    |channel, &ratio| Ok(Message::ChannelBasedTuning(channel, ratio)),
                )
            }
            TuningMethod::PitchBend { scale } => {
                let scale = scale.to_scale(app)?;
                self.run_internal(
                    options,
                    messages,
                    false,
                    AotTuner::apply_channel_based_tuning(&*scale.tuning, scale.keys),
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
        (tuner, channel_tunings): (AotTuner<PianoKey>, Vec<T>),
        mut to_tuning_message: impl FnMut(u8, &T) -> Result<Message, String>,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        println!(
            "This tuning requires {} output channels",
            tuner.num_channels()
        );

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
) -> CliResult<(String, MidiInputConnection<()>)> {
    Ok(midi::connect_to_in_device(
        target_port,
        move |raw_message| {
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
        },
    )?)
}
