use crate::{midi, mts::DeviceIdArg, shared::SclCommand, App, CliResult, KbmOptions};
use midir::{MidiInputConnection, MidiOutputConnection};
use std::{collections::HashMap, collections::VecDeque, mem, ops::Range, thread, time::Duration};
use structopt::StructOpt;
use tune::{
    midi::{ChannelMessage, ChannelMessageType, TransformResult},
    mts::{ScaleOctaveTuning, ScaleOctaveTuningMessage},
    note::Note,
    tuner::ChannelTuner,
};

#[derive(StructOpt)]
pub(crate) struct LiveOptions {
    /// MIDI input device
    #[structopt(long = "midi-in")]
    midi_in_device: usize,

    /// MIDI output device
    #[structopt(long = "midi-out")]
    midi_out_device: usize,

    #[structopt(subcommand)]
    tuning_method: TuningMethod,
}

#[derive(StructOpt)]
enum TuningMethod {
    /// Just-in-time: Implant a Scale/Octave tuning message (1 byte format) when NOTE ON is transmitted.
    /// This tuning method isn't perfect but, in return, only one MIDI channel is used (in-channel = out-channel).
    #[structopt(name = "jit")]
    JustInTime(JustInTimeOptions),

    /// Ahead-of-time: Retune multiple MIDI channels via Scale/Octave tuning messages (1 byte format) once on startup.
    /// This tuning method offers the highest musical flexibility but several MIDI channels need to be used.
    #[structopt(name = "aot")]
    AheadOfTime(AheadOfTimeOptions),

    /// Monophonic pitch-bend: Implant a pitch-bend message when NOTE ON is transmitted.
    /// This will work on most synthesizers. Since only one MIDI channel is used (in-channel = out-channel) this method is limited to  monophonic music.
    #[structopt(name = "mpb")]
    MonophonicPitchBend(MonophonicPitchBendOptions),

    /// Polyphonic pitch-bend: Implant a pitch-bend message when NOTE ON is transmitted.
    /// This will mork on most synthesizers. Multiple MIDI channel are used s.t. polyphonic music is possible.
    #[structopt(name = "ppb")]
    PolyphonicPitchBend(PolyphonicPitchBendOptions),
}

#[derive(StructOpt)]
struct JustInTimeOptions {
    #[structopt(flatten)]
    device_id: DeviceIdArg,

    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct AheadOfTimeOptions {
    #[structopt(flatten)]
    device_id: DeviceIdArg,

    #[structopt(flatten)]
    channels: ChannelsArg,

    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct MonophonicPitchBendOptions {
    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct PolyphonicPitchBendOptions {
    #[structopt(flatten)]
    channels: ChannelsArg,

    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(Clone, Copy, StructOpt)]
struct ChannelsArg {
    /// Specifies the MIDI channel to listen to
    #[structopt(long = "in-chan", default_value = "0")]
    in_channel: u8,

    /// Lower MIDI output channel bound (inclusve)
    #[structopt(long = "lo-chan", default_value = "0")]
    lower_out_channel_bound: u8,

    /// Upper MIDI output channel bound (exclusive)
    #[structopt(long = "up-chan", default_value = "16")]
    upper_out_channel_bound: u8,
}

impl LiveOptions {
    pub fn run(&self, _app: &mut App) -> CliResult<()> {
        let midi_in_device = self.midi_in_device;
        let out_connection = midi::connect_to_out_device(self.midi_out_device)
            .map_err(|err| format!("Could not connect to MIDI output device ({:?})", err))?;

        let in_connection = match &self.tuning_method {
            TuningMethod::JustInTime(options) => options.run(midi_in_device, out_connection)?,
            TuningMethod::AheadOfTime(options) => options.run(midi_in_device, out_connection)?,
            TuningMethod::MonophonicPitchBend(options) => {
                options.run(midi_in_device, out_connection)?
            }
            TuningMethod::PolyphonicPitchBend(options) => {
                options.run(midi_in_device, out_connection)?
            }
        };

        mem::forget(in_connection);

        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

impl JustInTimeOptions {
    fn run(
        &self,
        midi_in_device: usize,
        mut out_connection: MidiOutputConnection,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();
        let device_id = self.device_id.get()?;

        let mut octave_tuning = ScaleOctaveTuning::default();

        midi::connect_to_in_device(midi_in_device, move |message| {
            if let Some(original_message) = ChannelMessage::from_raw_message(message) {
                match original_message.transform(&(&scl, &kbm)) {
                    TransformResult::Transformed {
                        message,
                        note,
                        deviation,
                    } => {
                        if let ChannelMessageType::NoteOn { .. } = message.message_type() {
                            let note_letter = Note::from_midi_number(note).letter_and_octave().0;
                            *octave_tuning.as_mut(note_letter) = deviation;

                            let tuning_message =
                                ScaleOctaveTuningMessage::from_scale_octave_tuning(
                                    &octave_tuning,
                                    original_message.channel(),
                                    device_id,
                                )
                                .unwrap();

                            out_connection.send(tuning_message.sysex_bytes()).unwrap();
                        }

                        out_connection.send(&message.to_raw_message()).unwrap();
                    }
                    TransformResult::NotKeyBased => {
                        out_connection
                            .send(&original_message.to_raw_message())
                            .unwrap();
                    }
                    TransformResult::NoteOutOfRange => {}
                }
            }
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        midi_in_device: usize,
        mut out_connection: MidiOutputConnection,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();
        let device_id = self.device_id.get()?;

        let mut tuner = ChannelTuner::new();

        let octave_tunings = tuner
            .apply_octave_based_tuning(&(&scl, kbm), scl.period())
            .map_err(|err| format!("Could not apply tuning ({:?})", err))?;

        let channels = self.channels;
        let out_channels_range = channels.out_range();
        if octave_tunings.len() > out_channels_range.len() {
            return Err(format!(
                "The tuning requires {} output channels but the number of selected channels is {}",
                octave_tunings.len(),
                out_channels_range.len()
            )
            .into());
        }

        for (octave_tuning, channel) in octave_tunings.iter().zip(out_channels_range) {
            let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                &octave_tuning,
                channel,
                device_id,
            )
            .map_err(|err| format!("Could not apply tuning ({:?})", err))?;

            out_connection.send(tuning_message.sysex_bytes()).unwrap();
        }

        midi::connect_to_in_device(midi_in_device, move |message| {
            if let Some(original_message) = ChannelMessage::from_raw_message(message) {
                if original_message.channel() == channels.in_channel {
                    for message in original_message
                        .message_type()
                        .distribute(&tuner, channels.lower_out_channel_bound)
                    {
                        out_connection.send(&message.to_raw_message()).unwrap();
                    }
                }
            }
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

impl MonophonicPitchBendOptions {
    fn run(
        &self,
        midi_in_device: usize,
        mut out_connection: MidiOutputConnection,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();

        midi::connect_to_in_device(midi_in_device, move |message| {
            if let Some(original_message) = ChannelMessage::from_raw_message(message) {
                match original_message.transform(&(&scl, &kbm)) {
                    TransformResult::Transformed {
                        message, deviation, ..
                    } => {
                        if let ChannelMessageType::NoteOn { .. } = message.message_type() {
                            let pitch_bend_message = ChannelMessageType::PitchBendChange {
                                value: ((deviation.as_semitones() / 2.0 + 1.0) * 8192.0) as u16,
                            }
                            .in_channel(message.channel())
                            .unwrap();

                            out_connection
                                .send(&pitch_bend_message.to_raw_message())
                                .unwrap();
                        }

                        out_connection.send(&message.to_raw_message()).unwrap();
                    }
                    TransformResult::NotKeyBased => {
                        if let ChannelMessageType::PitchBendChange { .. } =
                            original_message.message_type()
                        {
                            return;
                        }

                        out_connection
                            .send(&original_message.to_raw_message())
                            .unwrap();
                    }
                    TransformResult::NoteOutOfRange => {}
                };
            }
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

impl PolyphonicPitchBendOptions {
    fn run(
        &self,
        midi_in_device: usize,
        mut out_connection: MidiOutputConnection,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();

        let channels = self.channels;

        let mut active_notes = HashMap::new();
        let mut free_channels = channels.out_range().collect::<VecDeque<_>>();

        midi::connect_to_in_device(midi_in_device, move |message| {
            let original_message = match ChannelMessage::from_raw_message(message) {
                Some(original_message) if original_message.channel() == channels.in_channel => {
                    original_message
                }
                _ => return,
            };

            match original_message.transform(&(&scl, &kbm)) {
                TransformResult::Transformed {
                    message, deviation, ..
                } => {
                    let suitable_channel = match original_message.message_type() {
                        ChannelMessageType::NoteOn { key, .. } => {
                            let free_channel = free_channels.pop_front();

                            if let Some(free_channel) = free_channel {
                                let pitch_bend_message = ChannelMessageType::PitchBendChange {
                                    value: ((deviation.as_semitones() / 2.0 + 1.0) * 8192.0) as u16,
                                }
                                .in_channel(free_channel)
                                .unwrap();

                                out_connection
                                    .send(&pitch_bend_message.to_raw_message())
                                    .unwrap();

                                active_notes.insert(key, free_channel);
                            }

                            free_channel
                        }
                        ChannelMessageType::NoteOff { key, .. } => {
                            let freed_channel = active_notes.remove(&key);

                            if let Some(freed_channel) = freed_channel {
                                free_channels.push_back(freed_channel);
                            }

                            freed_channel
                        }
                        ChannelMessageType::PolyphonicKeyPressure { key, .. } => {
                            active_notes.get(&key).copied()
                        }
                        _ => None,
                    };

                    if let Some(suitable_channel) = suitable_channel {
                        let message_with_correct_channel =
                            message.message_type().in_channel(suitable_channel).unwrap();

                        out_connection
                            .send(&message_with_correct_channel.to_raw_message())
                            .unwrap();
                    }
                }
                TransformResult::NotKeyBased => {
                    if let ChannelMessageType::PitchBendChange { .. } =
                        original_message.message_type()
                    {
                        return;
                    }
                    for channel in channels.out_range() {
                        let message_with_correct_channel =
                            original_message.message_type().in_channel(channel).unwrap();

                        out_connection
                            .send(&message_with_correct_channel.to_raw_message())
                            .unwrap();
                    }
                }
                TransformResult::NoteOutOfRange => {}
            };
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

impl ChannelsArg {
    fn out_range(&self) -> Range<u8> {
        self.lower_out_channel_bound..self.upper_out_channel_bound.min(16)
    }
}
