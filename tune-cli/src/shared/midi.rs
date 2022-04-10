use std::{collections::BTreeSet, error::Error, io};

use clap::{ArgEnum, Parser};
use midir::{MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use tune::{
    key::PianoKey,
    mts::ScaleOctaveTuningFormat,
    tuner::{MidiTarget, TunableMidi},
};

use crate::{CliError, CliResult};

#[derive(Parser)]
pub struct MidiInArgs {
    /// First MIDI channel to listen to for MIDI events
    #[clap(long = "in-chan", default_value = "0")]
    pub in_channel: u8,

    /// Number of MIDI input channels to listen to.
    /// Wraps around at zero-based channel number 15.
    /// For example --in-chan=10 and --in-chans=15 uses all channels but the drum channel.
    #[clap(long = "in-chans", default_value = "16")]
    pub num_in_channels: u8,

    /// Offset in scale steps per channel number.
    /// Required for keyboards with more than 128 keys like the Lumatone.
    #[clap(long = "luma-offs", default_value = "0")]
    pub lumatone_offset: i16,
}

impl MidiInArgs {
    pub fn get_midi_source(&self) -> CliResult<MidiSource> {
        Ok(MidiSource {
            channels: get_channels("Input", self.in_channel, self.num_in_channels)?.collect(),
            lumatone_offset: self.lumatone_offset,
        })
    }
}

pub struct MidiSource {
    pub channels: BTreeSet<u8>,
    pub lumatone_offset: i16,
}

impl MidiSource {
    pub fn get_offset(&self, channel: u8) -> MultiChannelOffset {
        MultiChannelOffset {
            offset: i32::from(channel) * i32::from(self.lumatone_offset),
        }
    }
}

pub struct MultiChannelOffset {
    offset: i32,
}

impl MultiChannelOffset {
    pub fn get_piano_key(&self, midi_number: u8) -> PianoKey {
        PianoKey::from_midi_number(i32::from(midi_number) + self.offset)
    }
}

#[derive(Parser)]
pub struct MidiOutArgs {
    /// First MIDI channel to send the modified MIDI events to
    #[clap(long = "out-chan", default_value = "0")]
    pub out_channel: u8,

    /// Number of MIDI output channels that should be retuned.
    /// Wraps around at zero-based channel number 15.
    /// For example --out-chan=10 and --out-chans=15 uses all channels but the drum channel.
    #[clap(long = "out-chans", default_value = "9")]
    pub num_out_channels: u8,

    #[clap(flatten)]
    pub device_id: DeviceIdArg,

    /// First tuning program to be used to store the channel-specific tuning information.
    /// Wraps around at tuning program number 127.
    #[clap(long = "tun-pg", default_value = "0")]
    pub tuning_program: u8,
}

impl MidiOutArgs {
    pub fn get_midi_target<H>(&self, handler: H) -> CliResult<MidiTarget<H>> {
        Ok(MidiTarget {
            handler,
            channels: get_channels("Output", self.out_channel, self.num_out_channels)?.collect(),
        })
    }

    pub fn create_synth<H>(&self, target: MidiTarget<H>, method: TuningMethod) -> TunableMidi<H> {
        match method {
            TuningMethod::FullKeyboard => TunableMidi::single_note_tuning_change(
                target,
                false,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::FullKeyboardRt => TunableMidi::single_note_tuning_change(
                target,
                true,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::Octave1 => TunableMidi::scale_octave_tuning(
                target,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave1Rt => TunableMidi::scale_octave_tuning(
                target,
                true,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2 => TunableMidi::scale_octave_tuning(
                target,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::Octave2Rt => TunableMidi::scale_octave_tuning(
                target,
                true,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => TunableMidi::channel_fine_tuning(target),
            TuningMethod::PitchBend => TunableMidi::pitch_bend(target),
        }
    }
}

fn get_channels(
    description: &str,
    first_channel: u8,
    num_channels: u8,
) -> CliResult<impl Iterator<Item = u8>> {
    if first_channel >= 16 {
        return Err(format!("{} channel is not in the range [0..16)", description).into());
    }
    if num_channels > 16 {
        return Err(format!(
            "Cannot use more than 16 {} channels",
            description.to_lowercase()
        )
        .into());
    }
    Ok((0..num_channels).map(move |channel| (first_channel + channel) % 16))
}

#[derive(Parser)]
pub struct DeviceIdArg {
    /// ID of the device that should respond to MTS messages
    #[clap(long = "dev-id", default_value = "127")]
    pub device_id: u8,
}

#[derive(Copy, Clone, ArgEnum)]
pub enum TuningMethod {
    #[clap(name = "full")]
    FullKeyboard,
    #[clap(name = "full-rt")]
    FullKeyboardRt,
    #[clap(name = "octave-1")]
    Octave1,
    #[clap(name = "octave-1-rt")]
    Octave1Rt,
    #[clap(name = "octave-2")]
    Octave2,
    #[clap(name = "octave-2-rt")]
    Octave2Rt,
    #[clap(name = "fine-tuning")]
    ChannelFineTuning,
    #[clap(name = "pitch-bend")]
    PitchBend,
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
    mut callback: impl FnMut(u64, &[u8]) + Send + 'static,
) -> MidiResult<(String, MidiInputConnection<()>)> {
    let midi_input = MidiInput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_input, fuzzy_port_name)?;

    Ok((
        port_name,
        midi_input.connect(
            &port,
            "MIDI out",
            move |timestamp_microsecs, message, _| callback(timestamp_microsecs, message),
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
