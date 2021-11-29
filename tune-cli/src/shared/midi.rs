use std::{error::Error, io};

use midir::{MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use structopt::StructOpt;

use crate::{CliError, CliResult};

#[derive(StructOpt)]
pub struct MidiOutArgs {
    /// First MIDI channel to send the modified MIDI events to
    #[structopt(long = "out-chan", default_value = "0")]
    pub out_channel: u8,

    /// Number of MIDI output channels that should be retuned.
    /// Wraps around at zero-based channel number 15.
    /// For example --out-chan=10 and --out-chans=15 uses all channels but the drum channel.
    #[structopt(long = "out-chans", default_value = "9")]
    pub num_out_channels: u8,

    #[structopt(flatten)]
    pub device_id: DeviceIdArg,

    /// First tuning program to be used to store the channel-specific tuning information.
    /// Wraps around at tuning program number 127.
    #[structopt(long = "tun-pg", default_value = "0")]
    pub tuning_program: u8,
}

impl MidiOutArgs {
    pub fn validate_channels(&self) -> CliResult<()> {
        Err(if self.num_out_channels > 16 {
            "Cannot use more than 16 channels"
        } else if self.out_channel >= 16 {
            "Output channel is not in the range [0..16)"
        } else {
            return Ok(());
        }
        .to_owned()
        .into())
    }
}

#[derive(StructOpt)]
pub struct DeviceIdArg {
    /// ID of the device that should respond to MTS messages
    #[structopt(long = "dev-id", default_value = "127")]
    pub device_id: u8,
}

#[derive(Copy, Clone)]
pub enum TuningMethod {
    FullKeyboard(bool),
    Octave1(bool),
    Octave2(bool),
    ChannelFineTuning,
    PitchBend,
}

pub fn parse_tuning_method(src: &str) -> Result<TuningMethod, String> {
    const FULL: &str = "full";
    const FULL_RT: &str = "full-rt";
    const OCTAVE_1: &str = "octave-1";
    const OCTAVE_1_RT: &str = "octave-1-rt";
    const OCTAVE_2: &str = "octave-2";
    const OCTAVE_2_RT: &str = "octave-2-rt";
    const FINE_TUNING: &str = "fine-tuning";
    const PITCH_BEND: &str = "pitch-bend";

    Ok(match &*src.to_lowercase() {
        FULL => TuningMethod::FullKeyboard(false),
        FULL_RT => TuningMethod::FullKeyboard(true),
        OCTAVE_1 => TuningMethod::Octave1(false),
        OCTAVE_1_RT => TuningMethod::Octave1(true),
        OCTAVE_2 => TuningMethod::Octave2(false),
        OCTAVE_2_RT => TuningMethod::Octave2(true),
        FINE_TUNING => TuningMethod::ChannelFineTuning,
        PITCH_BEND => TuningMethod::PitchBend,
        _ => {
            return Err(format!(
                "Invalid tuning method. Should be `{}`, `{}`, `{}`, `{}`, `{}`, `{}`, `{}` or `{}`",
                FULL,
                FULL_RT,
                OCTAVE_1,
                OCTAVE_1_RT,
                OCTAVE_2,
                OCTAVE_2_RT,
                FINE_TUNING,
                PITCH_BEND,
            ))
        }
    })
}

pub type MidiResult<T> = Result<T, MidiError>;

#[derive(Clone, Debug)]
pub enum MidiError {
    DeviceNotFound {
        wanted: String,
        available: Vec<String>,
    },
    AmbiguousDevice {
        wanted: String,
        matches: Vec<String>,
    },
    Other(String),
}

impl<T: Error> From<T> for MidiError {
    fn from(error: T) -> Self {
        MidiError::Other(error.to_string())
    }
}

impl From<MidiError> for CliError {
    fn from(v: MidiError) -> Self {
        CliError::CommandError(format!("Could not connect to MIDI device ({:#?})", v))
    }
}

pub fn print_midi_devices(mut dst: impl io::Write, client_name: &str) -> MidiResult<()> {
    let midi_input = MidiInput::new(client_name)?;
    writeln!(dst, "Readable MIDI devices:")?;
    for port in midi_input.ports() {
        writeln!(dst, "- {}", midi_input.port_name(&port)?)?;
    }

    let midi_output = MidiOutput::new(client_name)?;
    writeln!(dst, "Writable MIDI devices:")?;
    for port in midi_output.ports() {
        writeln!(dst, "- {}", midi_output.port_name(&port)?)?;
    }

    Ok(())
}

pub fn connect_to_in_device(
    client_name: &str,
    fuzzy_port_name: &str,
    mut callback: impl FnMut(&[u8]) + Send + 'static,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    let midi_input = MidiInput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_input, fuzzy_port_name)?;

    Ok((
        port_name,
        midi_input.connect(
            &port,
            "MIDI out",
            move |_, message, _| callback(message),
            (),
        )?,
    ))
}

pub fn connect_to_out_device(
    client_name: &str,
    fuzzy_port_name: &str,
) -> MidiResult<(String, MidiOutputConnection)> {
    let midi_output = MidiOutput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_output, fuzzy_port_name)?;

    Ok((port_name, midi_output.connect(&port, "MIDI in")?))
}

fn find_port_by_name<IO: MidiIO>(
    midi_io: &IO,
    target_port: &str,
) -> MidiResult<(String, IO::Port)> {
    let target_port_lowercase = target_port.to_lowercase();

    let mut matching_ports = midi_io
        .ports()
        .into_iter()
        .filter_map(|port| {
            midi_io
                .port_name(&port)
                .ok()
                .filter(|port_name| port_name.to_lowercase().contains(&target_port_lowercase))
                .map(|port_name| (port_name, port))
        })
        .collect::<Vec<_>>();

    match matching_ports.len() {
        0 => Err(MidiError::DeviceNotFound {
            wanted: target_port_lowercase,
            available: midi_io
                .ports()
                .iter()
                .filter_map(|port| midi_io.port_name(port).ok())
                .collect(),
        }),
        1 => Ok(matching_ports.pop().unwrap()),
        _ => Err(MidiError::AmbiguousDevice {
            wanted: target_port_lowercase,
            matches: matching_ports
                .into_iter()
                .map(|(port_name, _)| port_name)
                .collect(),
        }),
    }
}
