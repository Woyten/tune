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

use crate::{
    midi,
    shared::{self, MidiOutArgs, TuningMethod},
    App, CliError, CliResult, ScaleCommand,
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

    #[structopt(flatten)]
    midi_out_args: MidiOutArgs,

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

    /// MIDI-out tuning method
    #[structopt(parse(try_from_str=shared::parse_tuning_method))]
    method: TuningMethod,

    #[structopt(subcommand)]
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

#[derive(StructOpt)]
struct AheadOfTimeOptions {
    /// MIDI-out tuning method
    #[structopt(parse(try_from_str=shared::parse_tuning_method))]
    method: TuningMethod,

    #[structopt(subcommand)]
    scale: ScaleCommand,
}

impl LiveOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let midi_out = &self.midi_out_args;
        midi_out.validate_channels()?;

        let (send, recv) = mpsc::channel();
        let target = MidiTarget {
            handler: move |message| send.send(message).unwrap(),
            first_channel: midi_out.out_channel,
            num_channels: midi_out.num_out_channels,
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
            (0..self.midi_out_args.num_out_channels)
                .map(|c| (midi_out.out_channel + c) % 16)
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
        options: &LiveOptions,
        target: MidiTarget<impl MidiTunerMessageHandler + Send + 'static>,
    ) -> CliResult<(String, MidiInputConnection<()>)> {
        let tuning = self.scale.to_scale(app)?.tuning;
        let midi_out = &options.midi_out_args;

        let tuner = match &self.method {
            TuningMethod::FullKeyboard(realtime) => JitMidiTuner::single_note_tuning_change(
                target,
                self.clash_mitigation,
                *realtime,
                midi_out.device_id.device_id,
                midi_out.tuning_program,
            ),
            TuningMethod::Octave1(realtime) => JitMidiTuner::scale_octave_tuning(
                target,
                self.clash_mitigation,
                *realtime,
                midi_out.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2(realtime) => JitMidiTuner::scale_octave_tuning(
                target,
                self.clash_mitigation,
                *realtime,
                midi_out.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                JitMidiTuner::channel_fine_tuning(target, self.clash_mitigation)
            }
            TuningMethod::PitchBend => JitMidiTuner::pitch_bend(target, self.clash_mitigation),
        };
        self.run_internal(tuner, tuning, options)
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
        let scale = self.scale.to_scale(app)?;
        let midi_out = &options.midi_out_args;

        let tuner = match &self.method {
            TuningMethod::FullKeyboard(realtime) => AotMidiTuner::single_note_tuning_change(
                target,
                &*scale.tuning,
                scale.keys,
                *realtime,
                midi_out.device_id.device_id,
                midi_out.tuning_program,
            ),
            TuningMethod::Octave1(realtime) => AotMidiTuner::scale_octave_tuning(
                target,
                &*scale.tuning,
                scale.keys,
                *realtime,
                midi_out.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2(realtime) => AotMidiTuner::scale_octave_tuning(
                target,
                &*scale.tuning,
                scale.keys,
                *realtime,
                midi_out.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                AotMidiTuner::channel_fine_tuning(target, &*scale.tuning, scale.keys)
            }
            TuningMethod::PitchBend => AotMidiTuner::pitch_bend(target, &*scale.tuning, scale.keys),
        };

        match tuner {
            Err((_, num_required_channels)) => Result::Err(CliError::CommandError(format!(
                "Tuning requires {} channels but only {} channels are available",
                num_required_channels, midi_out.num_out_channels,
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
