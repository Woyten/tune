use crate::{dto::ScaleDto, shared::SclCommand, App, CliResult, KbmOptions};
use midir::{MidiOutput, MidiOutputConnection};
use std::{
    error::Error,
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

    /// ID of the device that should respond to the tuning messages
    #[structopt(long = "dev-id", default_value = "127")]
    device_id: u8,

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
}

#[derive(StructOpt)]
struct FromJsonOptions {
    /// Tuning program that should be affected
    #[structopt(long = "tun-pg", default_value = "0")]
    tuning_program: u8,
}

#[derive(StructOpt)]
struct OctaveOptions {
    /// Lower MIDI channel bound (inclusve)
    #[structopt(long = "lo-chan", default_value = "0")]
    lowest_midi_channel: u8,

    /// Upper MIDI channel bound (exclusive)
    #[structopt(long = "up-chan", default_value = "16")]
    highest_midi_channel: u8,

    #[structopt(flatten)]
    kbm_options: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
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
                .map(|index| connect_to_device(index))
                .transpose()
                .map_err(|err| format!("Could not connect to MIDI device ({:?})", err))?,
        };

        let device_id =
            DeviceId::from(self.device_id).ok_or_else(|| "Invalid device ID".to_owned())?;

        match &self.command {
            MtsCommand::FromJson(options) => options.run(app, &mut outputs, device_id),
            MtsCommand::Octave(options) => options.run(app, &mut outputs, device_id),
        }
    }
}

impl FromJsonOptions {
    fn run(&self, app: &mut App, outputs: &mut Outputs, device_id: DeviceId) -> CliResult<()> {
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
            device_id,
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
    fn run(&self, app: &mut App, outputs: &mut Outputs, device_id: DeviceId) -> CliResult<()> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.kbm_options.to_kbm();

        let channel_tunings = ChannelTuner::new()
            .apply_octave_based_tuning(&(&scl, kbm), scl.period())
            .map_err(|err| format!("Octave tuning not applicable ({:?})", err))?;

        for (channel_tuning, channel) in channel_tunings
            .iter()
            .zip(self.lowest_midi_channel..self.highest_midi_channel.min(128))
        {
            let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                channel_tuning,
                channel,
                device_id,
            )
            .map_err(|err| format!("Could not apply octave tuning ({:?})", err))?;

            app.errln(format_args!("== SysEx start (channel {}) ==", channel))?;
            outputs.write_midi_message(app, tuning_message.sysex_bytes())?;
            app.errln(format_args!("== SysEx end =="))?;
        }

        Ok(())
    }
}

struct Outputs {
    open_file: Option<File>,
    midi_out: Option<MidiOutputConnection>,
}

impl Outputs {
    fn write_midi_message(&mut self, app: &mut App, message: &[u8]) -> CliResult<()> {
        for byte in message {
            app.writeln(format_args!("0x{:02x}", byte))?;
        }
        if let Some(open_file) = &mut self.open_file {
            open_file.write_all(message)?;
        }
        if let Some(midi_out) = &mut self.midi_out {
            midi_out
                .send(message)
                .map_err(|err| format!("Could not send MIDI message: {}", err))?
        }

        Ok(())
    }
}

fn connect_to_device(target_port: usize) -> Result<MidiOutputConnection, MidiError> {
    let midi_output = MidiOutput::new("tune-cli")?;
    match midi_output.ports().get(target_port) {
        Some(port) => Ok(midi_output.connect(port, "tune-cli-output-connection")?),
        None => Err(MidiError::MidiDeviceNotFound(target_port)),
    }
}

#[derive(Clone, Debug)]
enum MidiError {
    MidiDeviceNotFound(usize),
    Other(String),
}

impl<T: Error> From<T> for MidiError {
    fn from(error: T) -> Self {
        MidiError::Other(error.to_string())
    }
}
