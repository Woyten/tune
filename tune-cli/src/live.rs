use crate::{midi, mts::DeviceIdArg, shared::SclCommand, App, CliResult, KbmOptions};
use std::{thread, time::Duration};
use structopt::StructOpt;
use tune::{
    key::PianoKey,
    midi::{ChannelMessage, ChannelMessageType},
    mts::ScaleOctaveTuning,
    mts::ScaleOctaveTuningMessage,
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

    #[structopt(flatten)]
    key_map_params: KbmOptions,

    #[structopt(subcommand)]
    command: SclCommand,
}

impl LiveOptions {
    pub fn run(&self, _app: &mut App) -> CliResult<()> {
        let scl = self.command.to_scl(None)?;
        let kbm = self.key_map_params.to_kbm();

        let tuning = (scl, kbm);
        let device_id = self.device_id.get()?;

        let mut out_connection = midi::connect_to_out_device(self.midi_out_device)
            .map_err(|err| format!("Could not connect to MIDI output device ({:?})", err))?;
        let mut octave_tuning = ScaleOctaveTuning::default();

        let conn = midi::connect_to_in_device(self.midi_in_device, move |message| {
            if let Some(ChannelMessage {
                channel,
                message_type,
            }) = ChannelMessage::from_raw_message(message)
            {
                match message_type {
                    ChannelMessageType::NoteOff { key, velocity } => {
                        let piano_key = PianoKey::from_midi_number(key.into());
                        let pitch = tuning.pitch_of(piano_key);
                        let approximation = pitch.find_in(&());
                        let note = approximation.approx_value;

                        if note.midi_number() < 128 {
                            out_connection
                                .send(&midi::note_off(channel, note.midi_number() as u8, velocity))
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

                        out_connection.send(tuning_message.sysex_bytes()).unwrap();

                        if note.midi_number() < 128 {
                            out_connection
                                .send(&midi::note_on(channel, note.midi_number() as u8, velocity))
                                .unwrap();
                        }
                        return;
                    }
                    _ => {}
                }
            }

            out_connection.send(message).unwrap();
        })
        .map_err(|err| format!("Could not connect to MIDI output device ({:?})", err))?;

        std::mem::forget(conn);

        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }
}
