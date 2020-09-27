use crate::{midi, mts::DeviceIdArg, shared::SclCommand, App, CliResult, KbmOptions};
use midir::MidiInputConnection;
use mpsc::Sender;
use std::{mem, sync::mpsc};
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    mts::{DeviceId, ScaleOctaveTuning, ScaleOctaveTuningMessage},
    tuner::ChannelTuner,
    tuning::Tuning,
};

#[derive(StructOpt)]
pub(crate) struct LiveOptions {
    /// MIDI input device
    #[structopt(long = "midi-in")]
    midi_in_device: usize,

    /// MIDI output device
    #[structopt(long = "midi-out")]
    midi_out_device: usize,

    #[structopt(flatten)]
    device_id: DeviceIdArg,

    #[structopt(subcommand)]
    tuning_method: TuningMethod,
}

#[derive(StructOpt)]
enum TuningMethod {
    /// Ahead-of-time: Implant a Scale/Octave tuning message (1 byte format) in front of each NOTE ON message.
    /// This tuning method isn't perfect but, in return, only a single MIDI channel is consumed (in-channel = out-channel).
    #[structopt(name = "jit")]
    JustInTime(JustInTimeOptions),

    /// Just-in-time: Retune multiple MIDI channels via Scale/Octave tuning messages (1 byte format) once on startup.
    /// This tunung method provides the best sound quality but several MIDI channels will be consumed.
    #[structopt(name = "aot")]
    AheadOfTime(AheadOfTimeOptions),
}

#[derive(StructOpt)]
struct JustInTimeOptions {
    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

#[derive(StructOpt)]
struct AheadOfTimeOptions {
    /// Specifies the MIDI channel to listen to
    #[structopt(long = "in-chan", default_value = "0")]
    in_channel: u8,

    /// Lower MIDI output channel bound (inclusve)
    #[structopt(long = "lo-chan", default_value = "0")]
    lower_out_channel_bound: u8,

    /// Upper MIDI output channel bound (exclusive)
    #[structopt(long = "up-chan", default_value = "16")]
    upper_out_channel_bound: u8,

    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

impl LiveOptions {
    pub fn run(&self, _app: &mut App) -> CliResult<()> {
        let midi_in_device = self.midi_in_device;
        let mut out_connection = midi::connect_to_out_device(self.midi_out_device)
            .map_err(|err| format!("Could not connect to MIDI output device ({:?})", err))?;
        let device_id = self.device_id.get()?;

        let (send, recv) = mpsc::channel();

        let in_connection = match &self.tuning_method {
            TuningMethod::JustInTime(options) => options.run(midi_in_device, send, device_id)?,
            TuningMethod::AheadOfTime(options) => options.run(midi_in_device, send, device_id)?,
        };

        for message in recv {
            match message {
                Message::Simple(simple) => out_connection.send(&simple),
                Message::Tuning(tuning) => out_connection.send(tuning.sysex_bytes()),
            }
            .unwrap();
        }

        mem::drop(in_connection); // TODO: Remove this?

        Ok(())
    }
}

impl JustInTimeOptions {
    fn run(
        &self,
        midi_in_device: usize,
        messages: Sender<Message>,
        device_id: DeviceId,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();
        let tuning = (scl, kbm);

        let mut octave_tuning = ScaleOctaveTuning::default();

        midi::connect_to_in_device(midi_in_device, move |message| {
            if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
                let channel = channel_message.channel();
                match channel_message.message_type() {
                    ChannelMessageType::NoteOff { key, velocity } => {
                        let piano_key = PianoKey::from_midi_number(key.into());
                        let pitch = tuning.pitch_of(piano_key);
                        let approximation = pitch.find_in(&());
                        let note = approximation.approx_value;

                        if note.midi_number() < 128 {
                            messages
                                .send(Message::Simple(midi::note_off(
                                    channel,
                                    note.midi_number() as u8,
                                    velocity,
                                )))
                                .unwrap();
                        }
                        return;
                    }
                    ChannelMessageType::NoteOn { key, velocity } => {
                        let piano_key = PianoKey::from_midi_number(key.into());
                        let pitch = tuning.pitch_of(piano_key);
                        let approximation = pitch.find_in(&());
                        let note = approximation.approx_value;

                        *octave_tuning.as_mut(note.letter_and_octave().0) = approximation.deviation;

                        let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                            &octave_tuning,
                            channel,
                            device_id,
                        )
                        .unwrap();

                        messages.send(Message::Tuning(tuning_message)).unwrap();

                        if note.midi_number() < 128 {
                            messages
                                .send(Message::Simple(midi::note_on(
                                    channel,
                                    note.midi_number() as u8,
                                    velocity,
                                )))
                                .unwrap();
                        }
                        return;
                    }
                    _ => {}
                }
            }

            if let &[a, b, c] = message {
                messages.send(Message::Simple([a, b, c])).unwrap();
            }
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

impl AheadOfTimeOptions {
    fn run(
        &self,
        midi_in_device: usize,
        messages: Sender<Message>,
        device_id: DeviceId,
    ) -> CliResult<MidiInputConnection<()>> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();
        let tuning = (&scl, kbm);

        let mut tuner = ChannelTuner::new();

        let octave_tunings = tuner
            .apply_octave_based_tuning(&tuning, scl.period())
            .map_err(|err| format!("Could not apply tuning ({:?})", err))?;

        let out_channel_range = self.lower_out_channel_bound..self.upper_out_channel_bound.min(16);
        if octave_tunings.len() > out_channel_range.len() {
            return Err(format!(
                "The tuning requires {} output channels but the number of selected channels is {}",
                octave_tunings.len(),
                out_channel_range.len()
            )
            .into());
        }

        for (octave_tuning, channel) in octave_tunings.iter().zip(out_channel_range) {
            let tuning_message = ScaleOctaveTuningMessage::from_scale_octave_tuning(
                &octave_tuning,
                channel,
                device_id,
            )
            .map_err(|err| format!("Could not apply tuning ({:?})", err))?;

            messages.send(Message::Tuning(tuning_message)).unwrap();
        }

        let in_channel = self.in_channel;
        let channel_offset = self.lower_out_channel_bound;
        midi::connect_to_in_device(midi_in_device, move |message| {
            if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
                if channel_message.channel() == in_channel {
                    for message in channel_message.distribute(&tuner, channel_offset) {
                        messages.send(Message::Simple(message)).unwrap();
                    }
                }
            }
        })
        .map_err(|err| format!("Could not connect to MIDI input device ({:?})", err).into())
    }
}

enum Message {
    Simple([u8; 3]),
    Tuning(ScaleOctaveTuningMessage),
}
