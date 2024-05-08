use std::{fmt::Debug, hash::Hash, sync::Arc};

use flume::Sender;
use serde::{Deserialize, Serialize};
use shared::midi::{self, MidiInArgs};
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    pitch::Pitch,
    scala::{KbmRoot, Scl},
    tuner::{MidiTunerMessage, MidiTunerMessageHandler, TunableMidi},
};
use tune_cli::{
    shared::{
        self,
        midi::{MidiOutArgs, MidiSource, TuningMethod},
    },
    CliResult,
};

use crate::{
    backend::{Backend, Backends, IdleBackend, NoteInput},
    piano::PianoEngine,
    portable,
    tunable::TunableBackend,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MidiOutSpec {
    pub note_input: NoteInput,

    pub out_device: String,

    #[serde(flatten)]
    pub out_args: MidiOutArgs,

    pub tuning_method: TuningMethod,
}

impl MidiOutSpec {
    pub fn create<
        I: From<MidiOutInfo> + From<MidiOutError> + Send + 'static,
        S: Copy + Eq + Hash + Debug + Send + 'static,
    >(
        &self,
        info_updates: &Sender<I>,
        backends: &mut Backends<S>,
    ) -> CliResult {
        let (midi_send, midi_recv) = flume::unbounded();

        let (device, mut midi_out, target) =
            match midi::connect_to_out_device("microwave", &self.out_device)
                .map_err(|err| format!("{err:?}"))
                .and_then(|(device, midi_out)| {
                    self.out_args
                        .get_midi_target(MidiOutHandler {
                            midi_events: midi_send,
                        })
                        .map(|target| (device, midi_out, target))
                        .map_err(|err| format!("{err:#?}"))
                }) {
                Ok(ok) => ok,
                Err(error_message) => {
                    let midi_out_error = MidiOutError {
                        out_device: self.out_device.clone(),
                        error_message,
                    };
                    backends.push(Box::new(IdleBackend::new(info_updates, midi_out_error)));
                    return Ok(());
                }
            };

        portable::spawn_task(async move {
            for message in midi_recv {
                message.send_to(|m| midi_out.send(m).unwrap());
            }
        });

        let synth = self.out_args.create_synth(target, self.tuning_method);

        let backend = MidiOutBackend {
            note_input: self.note_input,
            info_updates: info_updates.clone(),
            device: device.into(),
            tuning_method: self.tuning_method,
            curr_program: 0,
            backend: TunableBackend::new(synth),
        };

        backends.push(Box::new(backend));

        Ok(())
    }
}

struct MidiOutBackend<I, S> {
    note_input: NoteInput,
    info_updates: Sender<I>,
    device: Arc<str>,
    tuning_method: TuningMethod,
    curr_program: usize,
    backend: TunableBackend<S, TunableMidi<MidiOutHandler>>,
}

impl<I: From<MidiOutInfo> + Send, S: Copy + Eq + Hash + Debug + Send> Backend<S>
    for MidiOutBackend<I, S>
{
    fn note_input(&self) -> NoteInput {
        self.note_input
    }

    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        self.backend.set_tuning(tuning);
    }

    fn set_no_tuning(&mut self) {
        self.backend.set_no_tuning();
    }

    fn send_status(&mut self) {
        let is_tuned = self.backend.is_tuned();

        self.info_updates
            .send(
                MidiOutInfo {
                    device: self.device.clone(),
                    program_number: self.curr_program,
                    tuning_method: is_tuned.then_some(self.tuning_method),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.start(id, degree, pitch, velocity);
    }

    fn update_pitch(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.update_pitch(id, degree, pitch, velocity);
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.backend.update_pressure(id, pressure);
    }

    fn stop(&mut self, id: S, velocity: u8) {
        self.backend.stop(id, velocity);
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_program = update_fn(self.curr_program).min(127);

        self.backend
            .send_monophonic_message(ChannelMessageType::ProgramChange {
                program: u8::try_from(self.curr_program).unwrap(),
            });
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.backend
            .send_monophonic_message(ChannelMessageType::ControlChange { controller, value });
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.backend
            .send_monophonic_message(ChannelMessageType::ChannelPressure { pressure });
    }

    fn pitch_bend(&mut self, value: i16) {
        self.backend
            .send_monophonic_message(ChannelMessageType::PitchBendChange { value });
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        true
    }
}

struct MidiOutHandler {
    midi_events: Sender<MidiTunerMessage>,
}

impl MidiTunerMessageHandler for MidiOutHandler {
    fn handle(&mut self, message: MidiTunerMessage) {
        self.midi_events.send(message).unwrap();
    }
}

pub struct MidiOutInfo {
    pub device: Arc<str>,
    pub tuning_method: Option<TuningMethod>,
    pub program_number: usize,
}

#[derive(Clone)]
pub struct MidiOutError {
    pub out_device: String,
    pub error_message: String,
}

pub fn connect_to_in_device(
    engine: Arc<PianoEngine>,
    target_port: String,
    midi_in_options: &MidiInArgs,
) -> CliResult<()> {
    let midi_source = midi_in_options.get_midi_source()?;

    midi::start_in_connect_loop(
        "microwave".to_owned(),
        target_port,
        move |message| process_midi_event(message, &engine, &midi_source),
        |status| log::info!("[MIDI-in] {status}"),
    );

    Ok(())
}

fn process_midi_event(message: &[u8], engine: &Arc<PianoEngine>, midi_source: &MidiSource) {
    if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
        log::debug!("MIDI message received");
        log::debug!("{channel_message:#?}");

        if midi_source.channels.contains(&channel_message.channel()) {
            engine.handle_midi_event(
                channel_message.message_type(),
                midi_source.get_offset(channel_message.channel()),
            );
        }
    } else {
        log::warn!("Unsupported MIDI message received");
        for byte in message {
            log::warn!("{byte:02x}");
        }
    }
}
