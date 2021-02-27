use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};

use midir::MidiOutputConnection;
use structopt::StructOpt;
use tune::{
    mts::{DeviceId, SingleNoteTuningChangeMessage},
    tuner::ChannelTuner,
};

use crate::{midi, App, CliResult, ScaleCommand};

#[derive(StructOpt)]
pub(crate) struct MtsOptions {
    /// Write binary tuning message to a file
    #[structopt(long = "bin")]
    binary_file: Option<PathBuf>,

    /// Send tuning message to a MIDI device
    #[structopt(long = "send-to")]
    midi_out_device: Option<usize>,

    #[structopt(subcommand)]
    command: MtsCommand,
}

#[derive(StructOpt)]
enum MtsCommand {
    /// Retune a MIDI device (Single Note Tuning Change)
    #[structopt(name = "full")]
    FullKeyboard(FullKeyboardOptions),

    /// Retune a MIDI device (Non-Real Time Scale/Octave Tuning, 1 byte format).
    /// If necessary, multiple tuning messages are distributed over multiple channels.
    #[structopt(name = "octave")]
    Octave(OctaveOptions),

    /// Select a tuning program
    #[structopt(name = "tun-pg")]
    TuningProgram(TuningProgramOptions),

    /// Select a tuning bank
    #[structopt(name = "tun-bk")]
    TuningBank(TuningBankOptions),
}

#[derive(StructOpt)]
struct FullKeyboardOptions {
    #[structopt(flatten)]
    device_id: DeviceIdArg,

    /// Tuning program that should be affected
    #[structopt(long = "tun-pg", default_value = "0")]
    tuning_program: u8,

    #[structopt(subcommand)]
    scale: ScaleCommand,
}

#[derive(StructOpt)]
struct OctaveOptions {
    #[structopt(flatten)]
    device_id: DeviceIdArg,

    /// Lower MIDI channel bound (inclusve)
    #[structopt(long = "lo-chan", default_value = "0")]
    lower_channel_bound: u8,

    /// Upper MIDI channel bound (exclusive)
    #[structopt(long = "up-chan", default_value = "16")]
    upper_channel_bound: u8,

    #[structopt(subcommand)]
    scale: ScaleCommand,
}

#[derive(StructOpt)]
struct TuningProgramOptions {
    /// MIDI channel to apply the tuning program change to
    #[structopt(long = "chan", default_value = "0")]
    midi_channel: u8,

    /// Tuning program to select
    tuning_program: u8,
}

#[derive(StructOpt)]
struct TuningBankOptions {
    /// MIDI channel to apply the tuning bank change to
    #[structopt(long = "chan", default_value = "0")]
    midi_channel: u8,

    /// Tuning bank to select
    tuning_bank: u8,
}

#[derive(StructOpt)]
pub struct DeviceIdArg {
    /// ID of the device that should respond to the tuning messages
    #[structopt(long = "dev-id", default_value = "127")]
    device_id: u8,
}

impl MtsOptions {
    pub fn run(&self, app: &mut App) -> CliResult<()> {
        let mut outputs = Outputs {
            open_file: self
                .binary_file
                .as_ref()
                .map(|path| OpenOptions::new().write(true).create_new(true).open(path))
                .transpose()
                .map_err(|err| format!("Could not open output file: {}", err))?,

            midi_out: self
                .midi_out_device
                .map(midi::connect_to_out_device)
                .transpose()?,
        };

        match &self.command {
            MtsCommand::FullKeyboard(options) => options.run(app, &mut outputs),
            MtsCommand::Octave(options) => options.run(app, &mut outputs),
            MtsCommand::TuningProgram(options) => options.run(app, &mut outputs),
            MtsCommand::TuningBank(options) => options.run(app, &mut outputs),
        }
    }
}

impl FullKeyboardOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        let scale = self.scale.to_scale(app)?;

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning(
            &*scale.tuning,
            scale.keys,
            self.device_id.get()?,
            self.tuning_program,
        )
        .map_err(|err| format!("Could not apply single note tuning ({:?})", err))?;

        for message in tuning_message.sysex_bytes() {
            app.errln(format_args!("== SysEx start =="))?;
            outputs.write_midi_message(app, message)?;
            app.errln(format_args!("== SysEx end =="))?;
        }
        app.errln(format_args!(
            "Number of retuned notes: {}",
            tuning_message.retuned_notes().len(),
        ))?;
        app.errln(format_args!(
            "Number of out-of-range notes: {}",
            tuning_message.out_of_range_notes().len()
        ))?;

        Ok(())
    }
}

impl OctaveOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        let scale = self.scale.to_scale(app)?;

        let (_, channel_tunings) =
            ChannelTuner::apply_octave_based_tuning(&*scale.tuning, scale.keys.iter().copied());

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
            let tuning_message = channel_tuning
                .to_mts_format(self.device_id.get()?, channel)
                .map_err(|err| format!("Could not apply octave tuning ({:?})", err))?;

            app.errln(format_args!("== SysEx start (channel {}) ==", channel))?;
            outputs.write_midi_message(app, tuning_message.sysex_bytes())?;
            app.errln(format_args!("== SysEx end =="))?;
        }

        Ok(())
    }
}

impl TuningProgramOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        for (enumeration, message) in
            tune::mts::tuning_program_change(self.midi_channel, self.tuning_program)
                .unwrap()
                .iter()
                .enumerate()
        {
            app.errln(format_args!("== RPN part {} ==", enumeration))?;
            outputs.write_midi_message(app, &message.to_raw_message())?;
        }
        app.errln(format_args!("== Tuning program change end =="))?;

        Ok(())
    }
}

impl TuningBankOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        for (enumeration, message) in
            tune::mts::tuning_bank_change(self.midi_channel, self.tuning_bank)
                .unwrap()
                .iter()
                .enumerate()
        {
            app.errln(format_args!("== RPN part {} ==", enumeration))?;
            outputs.write_midi_message(app, &message.to_raw_message())?;
        }
        app.errln(format_args!("== Tuning bank change end =="))?;

        Ok(())
    }
}

impl DeviceIdArg {
    pub fn get(&self) -> Result<DeviceId, String> {
        DeviceId::from(self.device_id).ok_or_else(|| "Invalid device ID".to_owned())
    }
}

struct Outputs {
    open_file: Option<File>,
    midi_out: Option<(String, MidiOutputConnection)>,
}

impl Outputs {
    fn write_midi_message(&mut self, app: &mut App, message: &[u8]) -> CliResult<()> {
        for byte in message {
            app.writeln(format_args!("0x{:02x}", byte))?;
        }
        if let Some(open_file) = &mut self.open_file {
            open_file.write_all(message)?;
        }
        if let Some((device_name, midi_out)) = &mut self.midi_out {
            app.errln(format_args!("Sending MIDI data to {}", device_name))?;
            midi_out
                .send(message)
                .map_err(|err| format!("Could not send MIDI message: {}", err))?
        }

        Ok(())
    }
}
