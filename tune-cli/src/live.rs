use std::{mem, sync::mpsc};

use midir::MidiInputConnection;
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    mts::ScaleOctaveTuningFormat,
    tuner::{AotMidiTuner, JitMidiTuner, MidiTarget, MidiTunerMessageHandler, PoolingMode},
    tuning::KeyboardMapping,
};

use crate::{midi, mts::DeviceIdArg, App, CliError, CliResult, ScaleCommand};

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
        /// Send tuning message as real-time message
        #[structopt(long = "rt")]
        realtime: bool,

        #[structopt(flatten)]
        device_id: DeviceIdArg,

        /// First tuning program to be used to store the tuning information per channel.
        #[structopt(long = "tun-pg", default_value = "0")]
        tuning_program: u8,

        #[structopt(subcommand)]
        scale: ScaleCommand,
    },
    /// Retune channels via Scale/Octave Tuning (1 byte format) messages. Each channel can handle at most one detuning per note letter.
    #[structopt(name = "octave-1")]
    Octave1 {
        /// Send tuning message as real-time message
        #[structopt(long = "rt")]
        realtime: bool,

        #[structopt(flatten)]
        device_id: DeviceIdArg,

        #[structopt[subcommand]]
        scale: ScaleCommand,
    },
    /// Retune channels via Scale/Octave Tuning (2 byte format) messages. Each channel can handle at most one detuning per note letter.
    #[structopt(name = "octave-2")]
    Octave2 {
        /// Send tuning message as real-time message
        #[structopt(long = "rt")]
        realtime: bool,

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
        let target = MidiTarget {
            handler: move |message| send.send(message).unwrap(),
            first_channel: self.out_channel,
            num_channels: self.num_out_channels,
        };

        let (in_device, in_connection) = match &self.mode {
            LiveMode::JustInTime(options) => options.run(app, self, target)?,
            LiveMode::AheadOfTime(options) => options.run(app, self, target)?,
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
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        match &self.method {
            TuningMethod::FullKeyboard {
                realtime,
                device_id,
                tuning_program,
                scale,
            } => {
                let tuner = JitMidiTuner::single_note_tuning_change(
                    target,
                    self.clash_mitigation,
                    *realtime,
                    device_id.device_id,
                    *tuning_program,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, options)
            }
            TuningMethod::Octave1 {
                realtime,
                device_id,
                scale,
            } => {
                let tuner = JitMidiTuner::scale_octave_tuning(
                    target,
                    self.clash_mitigation,
                    *realtime,
                    device_id.device_id,
                    ScaleOctaveTuningFormat::OneByte,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, options)
            }
            TuningMethod::Octave2 {
                realtime,
                device_id,
                scale,
            } => {
                let tuner = JitMidiTuner::scale_octave_tuning(
                    target,
                    self.clash_mitigation,
                    *realtime,
                    device_id.device_id,
                    ScaleOctaveTuningFormat::TwoByte,
                );
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, options)
            }
            TuningMethod::ChannelFineTuning { scale } => {
                let tuner = JitMidiTuner::channel_fine_tuning(target, self.clash_mitigation);
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, options)
            }
            TuningMethod::PitchBend { scale } => {
                let tuner = JitMidiTuner::pitch_bend(target, self.clash_mitigation);
                let tuning = scale.to_scale(app)?.tuning;
                self.run_internal(tuner, tuning, options)
            }
        }
    }

    fn run_internal<H: MidiTunerMessageHandler + Send + 'static>(
        &self,
        mut tuner: JitMidiTuner<u8, H>,
        tuning: Box<dyn KeyboardMapping<PianoKey> + Send>,
        options: &LiveOptions,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
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
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        app: &mut App,
        options: &LiveOptions,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        let tuner = match &self.method {
            TuningMethod::FullKeyboard {
                realtime,
                device_id,
                tuning_program,
                scale,
            } => {
                let scale = scale.to_scale(app)?;

                AotMidiTuner::single_note_tuning_change(
                    target,
                    &*scale.tuning,
                    scale.keys,
                    *realtime,
                    device_id.device_id,
                    *tuning_program,
                )
            }
            TuningMethod::Octave1 {
                realtime,
                device_id,
                scale,
            } => {
                let scale = scale.to_scale(app)?;

                AotMidiTuner::scale_octave_tuning(
                    target,
                    &*scale.tuning,
                    scale.keys,
                    *realtime,
                    device_id.device_id,
                    ScaleOctaveTuningFormat::OneByte,
                )
            }
            TuningMethod::Octave2 {
                realtime,
                device_id,
                scale,
            } => {
                let scale = scale.to_scale(app)?;

                AotMidiTuner::scale_octave_tuning(
                    target,
                    &*scale.tuning,
                    scale.keys,
                    *realtime,
                    device_id.device_id,
                    ScaleOctaveTuningFormat::TwoByte,
                )
            }
            TuningMethod::ChannelFineTuning { scale } => {
                let scale = scale.to_scale(app)?;

                AotMidiTuner::channel_fine_tuning(target, &*scale.tuning, scale.keys)
            }
            TuningMethod::PitchBend { scale } => {
                let scale = scale.to_scale(app)?;

                AotMidiTuner::pitch_bend(target, &*scale.tuning, scale.keys)
            }
        };

        match tuner {
            Err(num_required_channels) => Result::Err(CliError::CommandError(format!(
                "Tuning requires {} channels but only {} channels are available",
                num_required_channels, options.num_out_channels,
            ))),
            Ok(tuner) => self.run_internal(tuner, options),
        }
    }

    fn run_internal<H: MidiTunerMessageHandler + Send + 'static>(
        &self,
        mut tuner: AotMidiTuner<PianoKey, H>,
        options: &LiveOptions,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        connect_to_in_device(
            &options.midi_in_device,
            options.in_channel,
            move |message| match message.message_type() {
                ChannelMessageType::NoteOff { key, velocity } => {
                    tuner.note_off(PianoKey::from_midi_number(key), velocity);
                }
                ChannelMessageType::NoteOn { key, velocity } => {
                    tuner.note_on(PianoKey::from_midi_number(key), velocity);
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    tuner.key_pressure(PianoKey::from_midi_number(key), pressure);
                }
                message_type @ (ChannelMessageType::ControlChange { .. }
                | ChannelMessageType::ProgramChange { .. }
                | ChannelMessageType::ChannelPressure { .. }
                | ChannelMessageType::PitchBendChange { .. }) => {
                    tuner.send_monophonic_message(message_type);
                }
            },
        )
    }
}

fn connect_to_in_device(
    target_port: &str,
    in_channel: u8,
    mut callback: impl FnMut(ChannelMessage) + Send + 'static,
) -> CliResult<(String, MidiInputConnection<()>)> {
    Ok(midi::connect_to_in_device(
        target_port,
        move |raw_message| {
            if let Some(parsed_message) = ChannelMessage::from_raw_message(raw_message) {
                if parsed_message.channel() == in_channel {
                    callback(parsed_message)
                }
            }
        },
    )?)
}
