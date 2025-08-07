use std::collections::BTreeSet;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_std::task;
use clap::Parser;
use clap::ValueEnum;
use midir::ConnectError;
use midir::InitError;
use midir::MidiIO;
use midir::MidiInput;
use midir::MidiOutput;
use midir::MidiOutputConnection;
use serde::Deserialize;
use serde::Serialize;
use tune::key::PianoKey;
use tune::mts::ScaleOctaveTuningFormat;
use tune::tuner::MidiTarget;
use tune::tuner::TunableMidi;

use crate::portable;
use crate::portable::SendTask;
use crate::CliResult;

#[derive(Parser)]
pub struct MidiInArgs {
    /// First MIDI channel to listen to for MIDI events
    #[arg(long = "in-chan", default_value = "0")]
    pub in_channel: u8,

    /// Number of MIDI input channels to listen to.
    /// Wraps around at zero-based channel number 15.
    /// For example --in-chan=10 and --in-chans=15 uses all channels but the drum channel.
    #[arg(long = "in-chans", default_value = "16")]
    pub num_in_channels: u8,

    /// Offset in scale steps per channel number.
    /// Required for keyboards with more than 128 keys like the Lumatone.
    #[arg(long = "luma-offs", default_value = "0")]
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
    pub offset: i32,
}

impl MultiChannelOffset {
    pub fn get_piano_key(&self, midi_number: u8) -> PianoKey {
        PianoKey::from_midi_number(i32::from(midi_number) + self.offset)
    }
}

const DEFAULT_OUT_CHANNEL: u8 = 0;
const DEFAULT_NUM_OUT_CHANS: u8 = 9;

#[derive(Clone, Debug, Deserialize, Serialize, Parser)]
pub struct MidiOutArgs {
    /// First MIDI channel to send the modified MIDI events to
    #[arg(long = "out-chan", default_value_t = DEFAULT_OUT_CHANNEL)]
    pub out_channel: u8,

    /// Number of MIDI output channels that should be retuned.
    /// Wraps around at zero-based channel number 15.
    /// For example --out-chan=10 and --out-chans=15 uses all channels but the drum channel.
    #[arg(long = "out-chans", default_value_t = DEFAULT_NUM_OUT_CHANS)]
    pub num_out_channels: u8,

    #[serde(flatten)]
    #[command(flatten)]
    pub device_id: DeviceIdArg,

    /// First tuning program to be used to store the channel-specific tuning information.
    /// Wraps around at tuning program number 127.
    #[arg(long = "tun-pg", default_value = "0")]
    pub tuning_program: u8,
}

impl Default for MidiOutArgs {
    fn default() -> Self {
        Self {
            out_channel: DEFAULT_OUT_CHANNEL,
            num_out_channels: DEFAULT_NUM_OUT_CHANS,
            device_id: Default::default(),
            tuning_program: Default::default(),
        }
    }
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
        return Err(format!("{description} channel is not in the range [0..16)").into());
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

const DEFAULT_DEVICE_ID: u8 = 0x7f;

#[derive(Clone, Debug, Deserialize, Serialize, Parser)]
pub struct DeviceIdArg {
    /// ID of the device that should respond to MTS messages
    #[arg(long = "dev-id", default_value_t = DEFAULT_DEVICE_ID)]
    pub device_id: u8,
}

impl Default for DeviceIdArg {
    fn default() -> Self {
        Self {
            device_id: DEFAULT_DEVICE_ID,
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, ValueEnum)]
pub enum TuningMethod {
    #[value(name = "full")]
    #[serde(rename = "full")]
    FullKeyboard,
    #[value(name = "full-rt")]
    #[serde(rename = "full-rt")]
    FullKeyboardRt,
    #[value(name = "octave-1")]
    #[serde(rename = "octave-1")]
    Octave1,
    #[value(name = "octave-1-rt")]
    #[serde(rename = "octave-1-rt")]
    Octave1Rt,
    #[value(name = "octave-2")]
    #[serde(rename = "octave-2")]
    Octave2,
    #[value(name = "octave-2-rt")]
    #[serde(rename = "octave-2-rt")]
    Octave2Rt,
    #[value(name = "fine-tuning")]
    #[serde(rename = "fine-tuning")]
    ChannelFineTuning,
    #[value(name = "pitch-bend")]
    #[serde(rename = "pitch-bend")]
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

pub fn start_in_connect_loop(
    client_name: String,
    fuzzy_port_name: String,
    callback: impl FnMut(&[u8]) + Send + 'static,
    report_status: impl FnMut(String) + Send + 'static,
) {
    let callback = Arc::new(Mutex::new(callback));

    start_connect_loop(
        fuzzy_port_name,
        move || MidiInput::new(&client_name),
        move |driver, port| {
            driver.connect(
                port,
                "MIDI-in",
                {
                    let callback = callback.clone();
                    move |_, message, _| (callback.try_lock().unwrap())(message)
                },
                (),
            )
        },
        |conn| {
            conn.close();
        },
        report_status,
    );
}

pub fn connect_to_out_device(
    client_name: &str,
    fuzzy_port_name: &str,
) -> MidiResult<(String, MidiOutputConnection)> {
    let midi_output = MidiOutput::new(client_name)?;

    let (port_name, port) = find_port_by_name(&midi_output, fuzzy_port_name)?;

    Ok((port_name, midi_output.connect(&port, "MIDI-out")?))
}

fn start_connect_loop<D: MidiIO, C>(
    fuzzy_port_name: String,
    mut driver_factory: impl FnMut() -> Result<D, InitError> + SendTask + 'static,
    mut connect: impl FnMut(D, &D::Port) -> Result<C, ConnectError<D>> + SendTask + 'static,
    mut disconnect: impl FnMut(C) + SendTask + 'static,
    mut report_status: impl FnMut(String) + SendTask + 'static,
) where
    D::Port: SendTask + 'static,
    C: SendTask + 'static,
{
    const SCAN_INTERVAL: Duration = Duration::from_secs(1);
    const REPORT_INTERVAL: u8 = 10;

    let mut port_name_conn = None;
    let mut report_count = 0;

    portable::spawn_task(async move {
        loop {
            match driver_factory() {
                Ok(driver) => {
                    if let Some((port, name, conn)) = port_name_conn.take() {
                        match driver.port_name(&port) {
                            Ok(_) => port_name_conn = Some((port, name, conn)),
                            Err(err) => {
                                report_status(format!("Lost connection to {name}: {err}"));
                                disconnect(conn);
                            }
                        }
                    };

                    if port_name_conn.is_none() {
                        if report_count == 0 {
                            report_status(format!(
                                "Waiting for MIDI device `{fuzzy_port_name}` to come online..."
                            ));
                        }

                        if let Ok((name, port)) = find_port_by_name(&driver, &fuzzy_port_name) {
                            match connect(driver, &port) {
                                Ok(conn) => {
                                    report_status(format!("Connected to {name}"));
                                    port_name_conn = Some((port, name, conn));
                                }
                                Err(err) => {
                                    report_status(format!("Failed to connect to {name}: {err}"));
                                }
                            }
                        }
                    }
                }
                Err(err) => report_status(format!("Unable to initialize MIDI driver: {err}")),
            }

            report_count += 1;
            report_count %= REPORT_INTERVAL;

            task::sleep(SCAN_INTERVAL).await;
        }
    });
}

fn find_port_by_name<IO: MidiIO>(
    midi_io: &IO,
    fuzzy_port_name: &str,
) -> MidiResult<(String, IO::Port)> {
    let target_port_lowercase = fuzzy_port_name.to_lowercase();

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
