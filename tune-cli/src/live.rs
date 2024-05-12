use clap::Parser;
use flume::Sender;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    tuner::{AotTuner, JitTuner, MidiTarget, MidiTunerMessageHandler, PoolingMode},
};

use crate::{
    error::ResultExt,
    midi::{self, MidiInArgs, MidiOutArgs, MidiSource, MultiChannelOffset, TuningMethod},
    App, CliError, CliResult, ScaleCommand,
};

#[derive(Parser)]
pub(crate) struct LiveOptions {
    /// MIDI input device
    #[arg(long = "midi-in")]
    midi_in_device: String,

    #[command(flatten)]
    midi_in_args: MidiInArgs,

    /// MIDI output device
    #[arg(long = "midi-out")]
    midi_out_device: String,

    #[command(flatten)]
    midi_out_args: MidiOutArgs,

    #[command(subcommand)]
    mode: LiveMode,
}

#[derive(Parser)]
enum LiveMode {
    /// Just-in-time: Tracks which notes are active and injects tuning messages into the stream of MIDI events.
    /// This mode uses a dynamic key-to-channel mapping to avoid tuning clashes.
    /// The number of output channels can be selected by the user and can be set to a small number.
    /// When tuning clashes occur several mitigation strategies can be applied.
    #[command(name = "jit")]
    JustInTime(JustInTimeOptions),

    /// Ahead-of-time: Sends all necessary tuning messages at startup.
    /// The key-to-channel mapping is fixed and eliminates tuning clashes s.t. this mode offers the highest degree of musical freedom.
    /// On the downside, the number of output channels cannot be changed by the user and might be a large number.
    #[command(name = "aot")]
    AheadOfTime(AheadOfTimeOptions),
}

#[derive(Parser)]
struct JustInTimeOptions {
    /// Describes what to do when a note is triggered that cannot be handled by any channel without tuning clashes.
    /// [block] Do not accept the new note. It will remain silent.
    /// [stop] Stop an old note and accept the new note.
    /// [ignore] Neither block nor stop. Accept that an old note receives an arbitrary tuning update.
    #[arg(long = "clash", default_value = "stop", value_parser = parse_mitigation)]
    clash_mitigation: PoolingMode,

    /// MIDI-out tuning method
    #[arg(value_enum)]
    method: TuningMethod,

    #[command(subcommand)]
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
    #[arg(value_enum)]
    method: TuningMethod,

    #[command(subcommand)]
    scale: ScaleCommand,
}

impl LiveOptions {
    pub async fn run(self, app: &mut App<'_>) -> CliResult {
        let (midi_send, midi_recv) = flume::unbounded();
        let (status_send, status_recv) = flume::unbounded();

        let handler = move |message| midi_send.send(message).unwrap();

        let source = self.midi_in_args.get_midi_source()?;
        let target = self.midi_out_args.get_midi_target(handler)?;

        let in_chans = source.channels.clone();
        let out_chans = target.channels.clone();

        match &self.mode {
            LiveMode::JustInTime(options) => options.run(
                app,
                source,
                target,
                self.midi_in_device,
                self.midi_out_args,
                status_send.clone(),
            )?,
            LiveMode::AheadOfTime(options) => options.run(
                app,
                source,
                target,
                self.midi_in_device,
                self.midi_out_args,
                status_send.clone(),
            )?,
        };

        let (out_device, mut out_connection) =
            midi::connect_to_out_device("tune-cli", &self.midi_out_device)
                .handle_error::<CliError>("Could not connect to MIDI output device")?;

        app.writeln(format_args!("Sending MIDI data to {out_device}"))?;
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

        futures::join!(
            async {
                while let Ok(message) = midi_recv.recv_async().await {
                    message.send_to(|message| out_connection.send(message).unwrap());
                }
            },
            async {
                while let Ok(status) = status_recv.recv_async().await {
                    app.writeln(status).unwrap();
                }
            }
        );

        Ok(())
    }
}

impl JustInTimeOptions {
    fn run(
        &self,
        app: &mut App,
        source: MidiSource,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
        midi_in_device: String,
        midi_out_args: MidiOutArgs,
        status_send: Sender<String>,
    ) -> CliResult<()> {
        let tuning = self.scale.to_scale(app)?.tuning;

        let synth = midi_out_args.create_synth(target, self.method);
        let mut tuner = JitTuner::start(synth, self.clash_mitigation);

        connect_to_in_device(
            midi_in_device,
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
                    if let Some(pitch) = tuning.maybe_pitch_of(piano_key) {
                        tuner.note_on(piano_key, pitch, velocity);
                    }
                }
                ChannelMessageType::PolyphonicKeyPressure { key, pressure } => {
                    let piano_key = offset.get_piano_key(key);
                    tuner.note_attr(piano_key, pressure);
                }
                message_type @ (ChannelMessageType::ControlChange { .. }
                | ChannelMessageType::ProgramChange { .. }
                | ChannelMessageType::ChannelPressure { .. }
                | ChannelMessageType::PitchBendChange { .. }) => {
                    tuner.global_attr(message_type);
                }
            },
            move |status| status_send.send(format!("[MIDI-in] {status}")).unwrap(),
        );

        Ok(())
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        app: &mut App,
        source: MidiSource,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
        midi_in_device: String,
        midi_out_args: MidiOutArgs,
        status_send: Sender<String>,
    ) -> CliResult<()> {
        let scale = self.scale.to_scale(app)?;

        let synth = midi_out_args.create_synth(target, self.method);
        let mut tuner = AotTuner::start(synth);

        let required_channels = tuner.set_tuning(&*scale.tuning, scale.keys).unwrap();
        if tuner.tuned() {
            app.writeln(format_args!(
                "Tuning requires {required_channels} MIDI channels"
            ))?
        } else {
            let available_channels = midi_out_args.num_out_channels;
            return Err(format!(
                "Tuning requires {required_channels} MIDI channels but only {available_channels} MIDI channels are available",
            )
            .into());
        }

        connect_to_in_device(
            midi_in_device,
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
                    tuner.note_attr(piano_key, pressure);
                }
                message_type @ (ChannelMessageType::ControlChange { .. }
                | ChannelMessageType::ProgramChange { .. }
                | ChannelMessageType::ChannelPressure { .. }
                | ChannelMessageType::PitchBendChange { .. }) => {
                    tuner.global_attr(message_type);
                }
            },
            move |status| status_send.send(format!("[MIDI-in] {status}")).unwrap(),
        );

        Ok(())
    }
}

fn connect_to_in_device(
    port_name: String,
    source: MidiSource,
    mut callback: impl FnMut(ChannelMessageType, MultiChannelOffset) + Send + 'static,
    status: impl FnMut(String) + Send + 'static,
) {
    midi::start_in_connect_loop(
        "tune-cli".to_owned(),
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
        status,
    );
}
