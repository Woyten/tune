use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use clap::Parser;
use midir::MidiOutputConnection;
use tune::{
    mts::{
        ScaleOctaveTuningFormat, ScaleOctaveTuningOptions, SingleNoteTuningChangeMessage,
        SingleNoteTuningChangeOptions,
    },
    tuner::AotTuningModel,
};

use crate::{
    shared::midi::{self, DeviceIdArg},
    App, CliResult, ScaleCommand,
};

#[derive(Parser)]
pub(crate) struct MtsOptions {
    /// Write binary tuning message to a file
    #[arg(long = "bin")]
    binary_file: Option<PathBuf>,

    /// Send tuning message to a MIDI device
    #[arg(long = "send-to")]
    midi_out_device: Option<String>,

    #[command(subcommand)]
    command: MtsCommand,
}

#[derive(Parser)]
enum MtsCommand {
    /// Retune a MIDI device (Single Note Tuning Change)
    #[command(name = "full")]
    FullKeyboard(FullKeyboardOptions),

    /// Retune a MIDI device (Real-Time Single Note Tuning Change)
    #[command(name = "full-rt")]
    FullKeyboardRt(FullKeyboardOptions),

    /// Retune a MIDI device (Scale/Octave Tuning, 1 byte format).
    /// If necessary, multiple tuning messages are distributed over multiple channels.
    #[command(name = "octave-1")]
    Octave1(OctaveOptions),

    /// Retune a MIDI device (Real-Time Scale/Octave Tuning, 1 byte format).
    /// If necessary, multiple tuning messages are distributed over multiple channels.
    #[command(name = "octave-1-rt")]
    Octave1Rt(OctaveOptions),

    /// Retune a MIDI device (Scale/Octave Tuning, 2 byte format).
    /// If necessary, multiple tuning messages are distributed over multiple channels.
    #[command(name = "octave-2")]
    Octave2(OctaveOptions),

    /// Retune a MIDI device (Real-Time Scale/Octave Tuning, 2 byte format).
    /// If necessary, multiple tuning messages are distributed over multiple channels.
    #[command(name = "octave-2-rt")]
    Octave2Rt(OctaveOptions),

    /// Select a tuning program
    #[command(name = "tun-pg")]
    TuningProgram(TuningProgramOptions),

    /// Select a tuning bank
    #[command(name = "tun-bk")]
    TuningBank(TuningBankOptions),
}

#[derive(Parser)]
struct FullKeyboardOptions {
    #[command(flatten)]
    device_id: DeviceIdArg,

    /// Tuning program that should be affected
    #[arg(long = "tun-pg", default_value = "0")]
    tuning_program: u8,

    #[command(subcommand)]
    scale: ScaleCommand,
}

#[derive(Parser)]
struct OctaveOptions {
    #[command(flatten)]
    device_id: DeviceIdArg,

    /// Lower MIDI channel bound (inclusive)
    #[arg(long = "lo-chan", default_value = "0")]
    lower_channel_bound: u8,

    /// Upper MIDI channel bound (exclusive)
    #[arg(long = "up-chan", default_value = "16")]
    upper_channel_bound: u8,

    #[command(subcommand)]
    scale: ScaleCommand,
}

#[derive(Parser)]
struct TuningProgramOptions {
    /// MIDI channel to apply the tuning program change to
    #[arg(long = "chan", default_value = "0")]
    midi_channel: u8,

    /// Tuning program to select
    tuning_program: u8,
}

#[derive(Parser)]
struct TuningBankOptions {
    /// MIDI channel to apply the tuning bank change to
    #[arg(long = "chan", default_value = "0")]
    midi_channel: u8,

    /// Tuning bank to select
    tuning_bank: u8,
}

impl MtsOptions {
    pub fn run(&self, app: &mut App) -> CliResult {
        let mut outputs = Outputs {
            open_file: self
                .binary_file
                .as_ref()
                .map(|path| OpenOptions::new().write(true).create_new(true).open(path))
                .transpose()
                .map_err(|err| format!("Could not open output file: {err}"))?,

            midi_out: self
                .midi_out_device
                .as_deref()
                .map(|target_port| midi::connect_to_out_device("tune-cli", target_port))
                .transpose()?,
        };

        match &self.command {
            MtsCommand::FullKeyboard(options) => options.run(app, &mut outputs, false),
            MtsCommand::FullKeyboardRt(options) => options.run(app, &mut outputs, true),
            MtsCommand::Octave1(options) => {
                options.run(app, &mut outputs, false, ScaleOctaveTuningFormat::OneByte)
            }
            MtsCommand::Octave1Rt(options) => {
                options.run(app, &mut outputs, true, ScaleOctaveTuningFormat::OneByte)
            }
            MtsCommand::Octave2(options) => {
                options.run(app, &mut outputs, false, ScaleOctaveTuningFormat::TwoByte)
            }
            MtsCommand::Octave2Rt(options) => {
                options.run(app, &mut outputs, true, ScaleOctaveTuningFormat::TwoByte)
            }
            MtsCommand::TuningProgram(options) => options.run(app, &mut outputs),
            MtsCommand::TuningBank(options) => options.run(app, &mut outputs),
        }
    }
}

impl FullKeyboardOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs, realtime: bool) -> CliResult {
        let scale = self.scale.to_scale(app)?;
        let options = SingleNoteTuningChangeOptions {
            realtime,
            device_id: self.device_id.device_id,
            tuning_program: self.tuning_program,
            with_bank_select: None,
        };

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning(
            &options,
            &*scale.tuning,
            scale.keys.iter().cloned(),
        )
        .map_err(|err| format!("Could not apply single note tuning ({err:?})"))?;

        for message in tuning_message.sysex_bytes() {
            app.errln(format_args!("== SysEx start =="))?;
            outputs.write_midi_message(app, message)?;
            app.errln(format_args!("== SysEx end =="))?;
        }
        app.errln(format_args!(
            "Number of retuned notes: {}",
            scale.keys.len() - tuning_message.out_of_range_notes().len(),
        ))?;
        app.errln(format_args!(
            "Number of out-of-range notes: {}",
            tuning_message.out_of_range_notes().len()
        ))?;

        Ok(())
    }
}

impl OctaveOptions {
    fn run(
        &self,
        app: &mut App,
        outputs: &mut Outputs,
        realtime: bool,
        format: ScaleOctaveTuningFormat,
    ) -> CliResult {
        let scale = self.scale.to_scale(app)?;

        let (_, channel_tunings) =
            AotTuningModel::apply_octave_based_tuning(&*scale.tuning, scale.keys);

        let channel_range = self.lower_channel_bound..self.upper_channel_bound.min(16);

        if channel_tunings.len() > channel_range.len() {
            return Err(format!(
                "The tuning requires {} output channels but the number of selected channels is {}",
                channel_tunings.len(),
                channel_range.len()
            )
            .into());
        }

        for (channel_tuning, channel) in channel_tunings.iter().zip(channel_range) {
            let options = ScaleOctaveTuningOptions {
                realtime,
                device_id: self.device_id.device_id,
                channels: channel.into(),
                format,
            };
            let tuning_message = channel_tuning
                .to_mts_format(&options)
                .map_err(|err| format!("Could not apply octave tuning ({err:?})"))?;

            app.errln(format_args!("== SysEx start (channel {channel}) =="))?;
            outputs.write_midi_message(app, tuning_message.sysex_bytes())?;
            app.errln(format_args!("== SysEx end =="))?;
        }

        Ok(())
    }
}

impl TuningProgramOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult {
        for (enumeration, message) in
            tune::mts::tuning_program_change(self.midi_channel, self.tuning_program)
                .unwrap()
                .iter()
                .enumerate()
        {
            app.errln(format_args!("== RPN part {enumeration} =="))?;
            outputs.write_midi_message(app, &message.to_raw_message())?;
        }
        app.errln(format_args!("== Tuning program change end =="))?;

        Ok(())
    }
}

impl TuningBankOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult {
        for (enumeration, message) in
            tune::mts::tuning_bank_change(self.midi_channel, self.tuning_bank)
                .unwrap()
                .iter()
                .enumerate()
        {
            app.errln(format_args!("== RPN part {enumeration} =="))?;
            outputs.write_midi_message(app, &message.to_raw_message())?;
        }
        app.errln(format_args!("== Tuning bank change end =="))?;

        Ok(())
    }
}

struct Outputs {
    open_file: Option<File>,
    midi_out: Option<(String, MidiOutputConnection)>,
}

impl Outputs {
    fn write_midi_message(&mut self, app: &mut App, message: &[u8]) -> CliResult {
        for byte in message {
            app.writeln(format_args!("0x{byte:02x}"))?;
        }
        if let Some(open_file) = &mut self.open_file {
            open_file.write_all(message)?;
        }
        if let Some((device_name, midi_out)) = &mut self.midi_out {
            app.errln(format_args!("Sending MIDI data to {device_name}"))?;
            midi_out
                .send(message)
                .map_err(|err| format!("Could not send MIDI message: {err}"))?
        }

        Ok(())
    }
}
