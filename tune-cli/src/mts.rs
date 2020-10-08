use crate::{dto::ScaleDto, midi, shared::SclCommand, App, CliResult, KbmOptions};
use midir::MidiOutputConnection;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
};
use structopt::StructOpt;
use tune::{
    mts::{
        DeviceId, ScaleOctaveTuningMessage, SingleNoteTuningChange, SingleNoteTuningChangeMessage,
    },
    pitch::Pitch,
    tuner::ChannelTuner,
};

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
    /// [in] Retune a MIDI device based on the JSON provided to stdin (Real Time Single Note Tuning Change)
    #[structopt(name = "from-json")]
    FromJson(FromJsonOptions),

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
struct FromJsonOptions {
    #[structopt(flatten)]
    device_id: DeviceIdArg,

    /// Tuning program that should be affected
    #[structopt(long = "tun-pg", default_value = "0")]
    tuning_program: u8,
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

    #[structopt(flatten)]
    kbm_options: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
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
                .transpose()
                .map_err(|err| format!("Could not connect to MIDI device ({:?})", err))?,
        };

        match &self.command {
            MtsCommand::FromJson(options) => options.run(app, &mut outputs),
            MtsCommand::Octave(options) => options.run(app, &mut outputs),
            MtsCommand::TuningProgram(options) => options.run(app, &mut outputs),
            MtsCommand::TuningBank(options) => options.run(app, &mut outputs),
        }
    }
}

impl FromJsonOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        let scale = ScaleDto::read(app.read())?;

        let tuning_changes = scale.items.iter().map(|item| {
            let approx = Pitch::from_hz(item.pitch_in_hz).find_in(&());
            SingleNoteTuningChange::new(
                item.key_midi_number as u8,
                approx.approx_value.midi_number(),
                approx.deviation,
            )
        });

        let tuning_message = SingleNoteTuningChangeMessage::from_tuning_changes(
            tuning_changes,
            self.device_id.get()?,
            self.tuning_program,
        )
        .map_err(|err| format!("Could not apply single note tuning ({:?})", err))?;

        app.errln(format_args!("== SysEx start =="))?;
        outputs.write_midi_message(app, tuning_message.sysex_bytes())?;
        app.errln(format_args!(
            "Number of retuned notes: {}",
            tuning_message.retuned_notes().len(),
        ))?;
        app.errln(format_args!(
            "Number of out-of-range notes: {}",
            tuning_message.out_of_range_notes().len()
        ))?;
        app.errln(format_args!("== SysEx end =="))?;

        Ok(())
    }
}

impl OctaveOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.kbm_options.to_kbm();

        let channel_tunings = ChannelTuner::new()
            .apply_octave_based_tuning(&(&scl, kbm), scl.period())
            .map_err(|err| format!("Octave tuning not applicable ({:?})", err))?;

        // The channel bitmask of the Scale/Octave tuning has 3*7 = 21 bytes. Therefore, we can print messages for up to 21 channels.
        let channel_range = self.lower_channel_bound..self.upper_channel_bound.min(21);

        if channel_tunings.len() > channel_range.len() {
            return Err(format!(
                "The tuning requires {} output channels but the number of selected channels is {}",
                channel_tunings.len(),
                channel_range.len()
            )
            .into());
        }

        for (channel_tuning, channel) in channel_tunings.iter().zip(channel_range) {
            let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                channel_tuning,
                channel,
                self.device_id.get()?,
            )
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
        const TUNING_PROGRAM_CHANGE_MSB: u8 = 0x00;
        const TUNING_PROGRAM_CHANGE_LSB: u8 = 0x03;

        for (enumeration, message) in midi::rpn_message(
            self.midi_channel,
            TUNING_PROGRAM_CHANGE_MSB,
            TUNING_PROGRAM_CHANGE_LSB,
            self.tuning_program,
        )
        .iter()
        .enumerate()
        {
            app.errln(format_args!("== RPN part {} ==", enumeration))?;
            outputs.write_midi_message(app, message)?;
        }
        app.errln(format_args!("== Tuning program change end =="))?;

        Ok(())
    }
}

impl TuningBankOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs) -> CliResult<()> {
        const TUNING_BANK_CHANGE_MSB: u8 = 0x00;
        const TUNING_BANK_CHANGE_LSB: u8 = 0x04;

        for (enumeration, message) in midi::rpn_message(
            self.midi_channel,
            TUNING_BANK_CHANGE_MSB,
            TUNING_BANK_CHANGE_LSB,
            self.tuning_bank,
        )
        .iter()
        .enumerate()
        {
            app.errln(format_args!("== RPN part {} ==", enumeration))?;
            outputs.write_midi_message(app, message)?;
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
            app.writeln(format_args!("Sending MIDI data to {}", device_name))?;
            midi_out
                .send(message)
                .map_err(|err| format!("Could not send MIDI message: {}", err))?
        }

        Ok(())
    }
}
