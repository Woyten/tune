use std::{error::Error, hash::Hash, io};

use clap::{ArgEnum, Parser};
use midir::{MidiIO, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use tune::{
    mts::ScaleOctaveTuningFormat,
    tuner::{AotMidiTuner, JitMidiTuner, MidiTarget, MidiTunerMessageHandler, PoolingMode},
    tuning::KeyboardMapping,
};

use crate::{CliError, CliResult};

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

    pub fn create_jit_tuner<K, H>(
        &self,
        handler: H,
        method: TuningMethod,
        pooling_mode: PoolingMode,
    ) -> JitMidiTuner<K, H> {
        let target = MidiTarget {
            handler,
            first_channel: self.out_channel,
            num_channels: self.num_out_channels,
        };

        match method {
            TuningMethod::FullKeyboard => JitMidiTuner::single_note_tuning_change(
                target,
                pooling_mode,
                false,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::FullKeyboardRt => JitMidiTuner::single_note_tuning_change(
                target,
                pooling_mode,
                true,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::Octave1 => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave1Rt => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                true,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2 => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::Octave2Rt => JitMidiTuner::scale_octave_tuning(
                target,
                pooling_mode,
                true,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                JitMidiTuner::channel_fine_tuning(target, pooling_mode)
            }
            TuningMethod::PitchBend => JitMidiTuner::pitch_bend(target, pooling_mode),
        }
    }

    pub fn create_aot_tuner<K: Copy + Eq + Hash, H: MidiTunerMessageHandler>(
        &self,
        handler: H,
        method: TuningMethod,
        tuning: impl KeyboardMapping<K>,
        keys: impl IntoIterator<Item = K>,
    ) -> Result<AotMidiTuner<K, H>, (MidiTarget<H>, usize)> {
        let target = MidiTarget {
            handler,
            first_channel: self.out_channel,
            num_channels: self.num_out_channels,
        };

        match method {
            TuningMethod::FullKeyboard => AotMidiTuner::single_note_tuning_change(
                target,
                tuning,
                keys,
                false,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::FullKeyboardRt => AotMidiTuner::single_note_tuning_change(
                target,
                tuning,
                keys,
                true,
                self.device_id.device_id,
                self.tuning_program,
            ),
            TuningMethod::Octave1 => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave1Rt => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::OneByte,
            ),
            TuningMethod::Octave2 => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                false,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::Octave2Rt => AotMidiTuner::scale_octave_tuning(
                target,
                tuning,
                keys,
                true,
                self.device_id.device_id,
                ScaleOctaveTuningFormat::TwoByte,
            ),
            TuningMethod::ChannelFineTuning => {
                AotMidiTuner::channel_fine_tuning(target, tuning, keys)
            }
            TuningMethod::PitchBend => AotMidiTuner::pitch_bend(target, tuning, keys),
        }
    }
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
