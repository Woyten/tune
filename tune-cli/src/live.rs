use std::{mem, sync::mpsc};

use clap::Parser;
use midir::MidiInputConnection;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    tuner::{MidiTarget, MidiTunerMessageHandler, PoolingMode},
};

use crate::{
    shared::midi::{self, MidiInArgs, MidiOutArgs, MidiSource, MultiChannelOffset, TuningMethod},
    App, CliResult, ScaleCommand,
};

#[derive(Parser)]
pub(crate) struct LiveOptions {
    /// MIDI input device
    #[clap(long = "midi-in")]
    midi_in_device: String,

    #[clap(flatten)]
    midi_in_args: MidiInArgs,

    /// MIDI output device
    #[clap(long = "midi-out")]
    midi_out_device: String,

    #[clap(flatten)]
    midi_out_args: MidiOutArgs,

    #[clap(subcommand)]
    mode: LiveMode,
}

#[derive(Parser)]
enum LiveMode {
    /// Just-in-time: Tracks which notes are active and injects tuning messages into the stream of MIDI events.
    /// This mode uses a dynamic key-to-channel mapping to avoid tuning clashes.
    /// The number of output channels can be selected by the user and can be set to a small number.
    /// When tuning clashes occur several mitigation strategies can be applied.
    #[clap(name = "jit")]
    JustInTime(JustInTimeOptions),

    /// Ahead-of-time: Sends all necessary tuning messages at startup.
    /// The key-to-channel mapping is fixed and eliminates tuning clashes s.t. this mode offers the highest degree of musical freedom.
    /// On the downside, the number of output channels cannot be changed by the user and might be a large number.
    #[clap(name = "aot")]
    AheadOfTime(AheadOfTimeOptions),
}

#[derive(Parser)]
struct JustInTimeOptions {
    /// Describes what to do when a note is triggered that cannot be handled by any channel without tuning clashes.
    /// [block] Do not accept the new note. It will remain silent.
    /// [stop] Stop an old note and accept the new note.
    /// [ignore] Neither block nor stop. Accept that an old note receives an arbitrary tuning update.
    #[clap(long = "clash", default_value = "stop", parse(try_from_str = parse_mitigation))]
    clash_mitigation: PoolingMode,

    /// MIDI-out tuning method
    #[clap(arg_enum)]
    method: TuningMethod,

    #[clap(subcommand)]
    scale: ScaleCommand,
}

fn parse_mitigation(src: &str) -> Result<PoolingMode, &'static str> {
    Ok(match &*src.to_lowercase() {
        "block" => PoolingMode::Block,
        "stop" => PoolingMode::Stop,
        "ignore" => PoolingMode::Ignore,
        _ => return Err("Invalid mode. Should be `block`, `stop` or `ignore`"),
    })
}

#[derive(Parser)]
struct AheadOfTimeOptions {
    /// MIDI-out tuning method
    #[clap(arg_enum)]
    method: TuningMethod,

    #[clap(subcommand)]
    scale: ScaleCommand,
}

impl LiveOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let (send, recv) = mpsc::channel();
        let handler = move |message| send.send(message).unwrap();

        let source = self.midi_in_args.get_midi_source()?;
        let target = self.midi_out_args.get_midi_target(handler)?;

        let in_chans = source.channels.clone();
        let out_chans = target.channels.clone();

        let (in_device, in_connection) = match &self.mode {
            LiveMode::JustInTime(options) => options.run(app, source, target, self)?,
            LiveMode::AheadOfTime(options) => options.run(app, source, target, self)?,
        };

        let (out_device, mut out_connection) =
            midi::connect_to_out_device("tune-cli", &self.midi_out_device)?;

        app.writeln(format_args!("Receiving MIDI data from {}", in_device))?;
        app.writeln(format_args!("Sending MIDI data to {}", out_device))?;
        app.writeln(format_args!(
            "in-channels {{{}}} -> out-channels {{{}}}",
            in_chans
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            out_chans
                .iter()
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
}

impl JustInTimeOptions {
    fn run(
        &self,
        app: &mut App,
        source: MidiSource,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
        options: &LiveOptions,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        let tuning = self.scale.to_scale(app)?.tuning;

        let mut tuner =
            options
                .midi_out_args
                .create_jit_tuner(target, self.method, self.clash_mitigation);

        connect_to_in_device(
            &options.midi_in_device,
            source,
            move |message_type, offset| match message_type {
                ChannelMessageType::NoteOff { key, velocity }
                | ChannelMessageType::NoteOn {
                    key,
                    velocity: velocity @ 0,
                } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.note_off(&piano_key, velocity);
                }
                ChannelMessageType::NoteOn { key, velocity } => {
                    let piano_key = offset.get_piano_key(key);
                    if let Some(pitch) = tuning.maybe_pitch_of(piano_key) {
                        tuner.note_on(piano_key, pitch, velocity);
                    }
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.key_pressure(&piano_key, pressure);
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
        source: MidiSource,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
        options: &LiveOptions,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        let scale = self.scale.to_scale(app)?;

        let mut tuner = options
            .midi_out_args
            .create_aot_tuner(target, self.method, &*scale.tuning, scale.keys)
            .map_err(|(_, num_required_channels)| {
                format!(
                    "Tuning requires {} MIDI channels but only {} MIDI channels are available",
                    num_required_channels, options.midi_out_args.num_out_channels,
                )
            })?;

        println!("Tuning requires {} MIDI channels", tuner.num_channels());

        connect_to_in_device(
            &options.midi_in_device,
            source,
            move |message_type, offset| match message_type {
                ChannelMessageType::NoteOff { key, velocity }
                | ChannelMessageType::NoteOn {
                    key,
                    velocity: velocity @ 0,
                } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.note_off(piano_key, velocity);
                }
                ChannelMessageType::NoteOn { key, velocity } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.note_on(piano_key, velocity);
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.key_pressure(piano_key, pressure);
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
    port_name: &str,
    source: MidiSource,
    mut callback: impl FnMut(ChannelMessageType, MultiChannelOffset) + Send + 'static,
) -> CliResult<(String, MidiInputConnection<()>)> {
    Ok(midi::connect_to_in_device(
        "tune-cli",
        port_name,
        move |raw_message| {
            if let Some(parsed_message) = ChannelMessage::from_raw_message(raw_message) {
                if source.channels.contains(&parsed_message.channel()) {
                    callback(
                        parsed_message.message_type(),
                        source.get_offset(parsed_message.channel()),
                    );
                }
            }
        },
    )?)
}
