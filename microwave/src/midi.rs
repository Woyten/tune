use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use flume::Sender;
use midi::MultiChannelOffset;
use serde::Deserialize;
use serde::Serialize;
use shared::midi;
use shared::midi::MidiInArgs;
use tune::midi::ChannelMessage;
use tune::midi::ChannelMessageType;
use tune::pitch::Pitch;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::tuner::MidiTunerMessage;
use tune::tuner::MidiTunerMessageHandler;
use tune::tuner::TunableMidi;
use tune_cli::shared;
use tune_cli::shared::error::ResultExt;
use tune_cli::shared::midi::MidiOutArgs;
use tune_cli::shared::midi::MidiSource;
use tune_cli::shared::midi::TuningMethod;
use tune_cli::CliResult;

use crate::backend::Backend;
use crate::backend::Backends;
use crate::backend::IdleBackend;
use crate::backend::NoteInput;
use crate::lumatone;
use crate::piano::PianoEngine;
use crate::portable;
use crate::tunable::TunableBackend;

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
        K: Copy + Eq + Hash + Debug + Send + 'static,
        E: From<MidiOutEvent> + From<MidiOutError> + Send + 'static,
    >(
        &self,
        backends: &mut Backends<K>,
        events: &Sender<E>,
    ) -> CliResult {
        let (midi_send, midi_recv) = flume::unbounded();

        let (device, mut midi_out, target) =
            match midi::connect_to_out_device("microwave", &self.out_device)
                .handle_error("Could not connect to MIDI output device")
                .and_then(|(device, midi_out)| {
                    self.out_args
                        .get_midi_target(MidiOutHandler {
                            midi_events: midi_send,
                        })
                        .map(|target| (device, midi_out, target))
                }) {
                Ok(ok) => ok,
                Err(error_message) => {
                    let midi_out_error = MidiOutError {
                        out_device: self.out_device.clone(),
                        error_message: error_message.to_string(),
                    };
                    backends.push(Box::new(IdleBackend::new(events, midi_out_error)));
                    return Ok(());
                }
            };

        portable::spawn_task(async move {
            while let Ok(message) = midi_recv.recv_async().await {
                message.send_to(|m| midi_out.send(m).unwrap());
            }
        });

        let synth = self.out_args.create_synth(target, self.tuning_method);

        let backend = MidiOutBackend {
            note_input: self.note_input,
            events: events.clone(),
            device: device.into(),
            tuning_method: self.tuning_method,
            curr_program: 0,
            backend: TunableBackend::new(synth),
        };

        backends.push(Box::new(backend));

        Ok(())
    }
}

struct MidiOutBackend<K, E> {
    note_input: NoteInput,
    events: Sender<E>,
    device: Arc<str>,
    tuning_method: TuningMethod,
    curr_program: usize,
    backend: TunableBackend<K, TunableMidi<MidiOutHandler>>,
}

impl<K: Copy + Eq + Hash + Debug + Send, E: From<MidiOutEvent> + Send> Backend<K>
    for MidiOutBackend<K, E>
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

        self.events
            .send(
                MidiOutEvent {
                    device: self.device.clone(),
                    program_number: self.curr_program,
                    tuning_method: is_tuned.then_some(self.tuning_method),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, key_id: K, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.start(key_id, degree, pitch, velocity);
    }

    fn update_pitch(&mut self, key_id: K, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.update_pitch(key_id, degree, pitch, velocity);
    }

    fn update_pressure(&mut self, key_id: K, pressure: u8) {
        self.backend.update_pressure(key_id, pressure);
    }

    fn stop(&mut self, key_id: K, velocity: u8) {
        self.backend.stop(key_id, velocity);
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

pub struct MidiOutEvent {
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
    lumatone_mode: bool,
) -> CliResult<()> {
    let midi_source = midi_in_options.get_midi_source()?;

    midi::start_in_connect_loop(
        "microwave".to_owned(),
        target_port,
        move |message| handle_midi_message(message, &engine, &midi_source, lumatone_mode),
        |status| log::info!("[MIDI-in] {status}"),
    );

    Ok(())
}

fn handle_midi_message(
    message: &[u8],
    engine: &Arc<PianoEngine>,
    midi_source: &MidiSource,
    lumatone_mode: bool,
) {
    if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
        log::debug!("MIDI message received");
        log::debug!("{channel_message:#?}");

        if lumatone_mode {
            engine.handle_midi_event(
                channel_message.message_type(),
                MultiChannelOffset {
                    offset: i32::from(channel_message.channel()) * 128 - lumatone::RANGE_RADIUS,
                },
                true,
            );
        } else if midi_source.channels.contains(&channel_message.channel()) {
            engine.handle_midi_event(
                channel_message.message_type(),
                midi_source.get_offset(channel_message.channel()),
                false,
            );
        }
    } else {
        log::debug!("Unsupported MIDI message received");
        for byte in message {
            log::debug!("{byte:02x}");
        }
    }
}
