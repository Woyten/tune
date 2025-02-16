use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
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
use crate::backend::BankSelect;
use crate::backend::IdleBackend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::lumatone;
use crate::piano::PianoEngine;
use crate::portable;
use crate::tunable::TunableBackend;

#[derive(Deserialize, Serialize)]
pub struct MidiOutSpec {
    pub note_input: NoteInput,

    pub out_device: String,
    #[serde(flatten)]
    pub out_args: MidiOutArgs,
    pub tuning_method: TuningMethod,

    pub banks: BTreeSet<Bank>,
    pub default_bank: Option<Bank>,
    pub default_program: Option<u8>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Bank {
    pub msb: u8,
    pub lsb: u8,
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
                .debug_err("Could not connect to MIDI output device")
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
                log::debug!("Sending MIDI message: {message:?}");

                message.send_to(|m| {
                    if let Err(err) = midi_out.send(m) {
                        log::error!("Error sending MIDI message: {err}");
                    }
                })
            }
        });

        let mut backend = MidiOutBackend {
            note_input: self.note_input,
            events: events.clone(),
            device: device.into(),
            tuning_method: self.tuning_method,
            banks: self.banks.iter().copied().collect(),
            curr_bank_msb: self.default_bank.map(|b| b.msb),
            curr_bank_lsb: self.default_bank.map(|b| b.lsb),
            curr_program: self.default_program.unwrap_or_default(),
            backend: TunableBackend::new(self.out_args.create_synth(target, self.tuning_method)),
        };
        backend.init();
        backends.push(Box::new(backend));

        Ok(())
    }
}

struct MidiOutBackend<K, E> {
    note_input: NoteInput,
    events: Sender<E>,
    device: Arc<str>,
    tuning_method: TuningMethod,
    banks: Vec<Bank>,
    curr_bank_msb: Option<u8>,
    curr_bank_lsb: Option<u8>,
    curr_program: u8,
    backend: TunableBackend<K, TunableMidi<MidiOutHandler>>,
}

impl<K: Copy + Eq + Hash + Debug + Send, E: From<MidiOutEvent> + Send> MidiOutBackend<K, E> {
    const CCN_BANK_SELECT_MSB: u8 = 0;
    const CCN_BANK_SELECT_LSB: u8 = 32;

    fn init(&mut self) {
        if let Some(curr_bank_msb) = self.curr_bank_msb {
            log::info!("Initializing bank MSB to {}.", curr_bank_msb);
            self.control_change(Self::CCN_BANK_SELECT_MSB, curr_bank_msb);
        }
        if let Some(curr_bank_lsb) = self.curr_bank_lsb {
            log::info!("Initializing bank LSB to {}.", curr_bank_lsb);
            self.control_change(Self::CCN_BANK_SELECT_LSB, curr_bank_lsb);
        }
        log::info!("Initializing program to {}.", self.curr_program);
        self.program_change(ProgramChange::ProgramId(self.curr_program));
    }
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

    fn request_status(&mut self) {
        self.events
            .send(
                MidiOutEvent {
                    device: self.device.clone(),
                    program_number: self.curr_program,
                    bank_msb: self.curr_bank_msb,
                    bank_lsb: self.curr_bank_lsb,
                    tuning_method: self.backend.is_tuned().then_some(self.tuning_method),
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

    fn bank_select(&mut self, bank_select: BankSelect) {
        let curr_bank = Bank {
            msb: self.curr_bank_msb.unwrap_or_default(),
            lsb: self.curr_bank_lsb.unwrap_or_default(),
        };

        let updated_bank_index = match bank_select {
            BankSelect::Inc => match self.banks.binary_search(&curr_bank) {
                Ok(exact_match) => exact_match.saturating_add(1),
                Err(inexact_match) => inexact_match,
            },
            BankSelect::Dec => match self.banks.binary_search(&curr_bank) {
                Ok(exact_match) => exact_match.saturating_sub(1),
                Err(inexact_match) => inexact_match.saturating_sub(1),
            },
        };

        if let Some(&updated_bank) = self.banks.get(updated_bank_index) {
            self.curr_bank_msb = Some(updated_bank.msb);
            self.curr_bank_lsb = Some(updated_bank.lsb);

            self.control_change(Self::CCN_BANK_SELECT_MSB, updated_bank.msb);
            self.control_change(Self::CCN_BANK_SELECT_LSB, updated_bank.lsb);
            self.program_change(ProgramChange::ProgramId(self.curr_program));
        }
    }

    fn program_change(&mut self, program_change: ProgramChange) {
        self.curr_program = match program_change {
            ProgramChange::ProgramId(program_id) => program_id,
            ProgramChange::Inc => (self.curr_program + 1).min(127),
            ProgramChange::Dec => self.curr_program.saturating_sub(1),
        };

        self.backend
            .send_monophonic_message(ChannelMessageType::ProgramChange {
                program: self.curr_program,
            });
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        match controller {
            Self::CCN_BANK_SELECT_MSB => self.curr_bank_msb = Some(value),
            Self::CCN_BANK_SELECT_LSB => self.curr_bank_lsb = Some(value),
            _ => {}
        }

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

#[derive(Debug, PartialEq, Eq)]
pub struct MidiOutEvent {
    pub device: Arc<str>,
    pub tuning_method: Option<TuningMethod>,
    pub bank_msb: Option<u8>,
    pub bank_lsb: Option<u8>,
    pub program_number: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
        log::debug!("Received MIDI message: {channel_message:?}");

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
        struct HexFormatter<'a>(&'a [u8]);

        impl<'a> Display for HexFormatter<'a> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(f, "{:02x} ", byte)?;
                }
                Ok(())
            }
        }

        log::debug!(
            "Received unsupported MIDI message: {}",
            HexFormatter(message)
        );
    }
}

#[cfg(test)]
mod tests {
    use std::any;

    use flume::Receiver;
    use midir::os::unix::VirtualInput;
    use midir::MidiInput;
    use midir::MidiInputConnection;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::pipeline::PipelineEvent;

    #[test]
    fn cannot_change_bank_when_bank_list_is_empty_no_default_bank_set() {
        let mut fixture = MidiOutFixture::from_bank_settings([], None);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: None,
                bank_lsb: None,
                program_number: 0,
            }
        );

        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: None,
                bank_lsb: None,
                program_number: 0,
            }
        );

        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: None,
                bank_lsb: None,
                program_number: 0,
            }
        );
    }

    #[test]
    fn cannot_change_bank_when_bank_list_is_empty_default_bank_set() {
        let mut fixture = MidiOutFixture::from_bank_settings([], Some(Bank { msb: 12, lsb: 34 }));
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );

        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );

        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );
    }

    #[test]
    fn can_change_bank_when_bank_list_is_provided_no_default_bank_set() {
        let mut fixture = MidiOutFixture::from_bank_settings(
            [Bank { msb: 56, lsb: 78 }, Bank { msb: 12, lsb: 34 }],
            None,
        );

        // No bank is selected
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: None,
                bank_lsb: None,
                program_number: 0,
            }
        );

        // Inc selects first bank
        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );

        let mut fixture = MidiOutFixture::from_bank_settings(
            [Bank { msb: 56, lsb: 78 }, Bank { msb: 12, lsb: 34 }],
            None,
        );

        // Dec selects first bank
        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );
    }

    #[test]
    fn can_change_bank_when_bank_list_is_provided_default_bank_set() {
        let mut fixture = MidiOutFixture::from_bank_settings(
            [Bank { msb: 56, lsb: 78 }, Bank { msb: 12, lsb: 34 }],
            Some(Bank { msb: 34, lsb: 56 }),
        );

        // Default bank is selected
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(34),
                bank_lsb: Some(56),
                program_number: 0,
            }
        );

        // Inc selects second bank
        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(56),
                bank_lsb: Some(78),
                program_number: 0,
            }
        );

        let mut fixture = MidiOutFixture::from_bank_settings(
            [Bank { msb: 56, lsb: 78 }, Bank { msb: 12, lsb: 34 }],
            Some(Bank { msb: 34, lsb: 56 }),
        );

        // Dec selects first bank
        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );
    }

    #[test]
    fn can_inc_and_dec_through_banks() {
        let mut fixture = MidiOutFixture::from_bank_settings(
            [
                Bank { msb: 56, lsb: 78 },
                Bank { msb: 12, lsb: 34 },
                Bank { msb: 90, lsb: 12 },
            ],
            Some(Bank { msb: 34, lsb: 56 }),
        );

        // Start in the middle between first and second bank
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(34),
                bank_lsb: Some(56),
                program_number: 0,
            }
        );

        // Second bank is selected
        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(56),
                bank_lsb: Some(78),
                program_number: 0,
            }
        );

        // Last bank is selected
        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(90),
                bank_lsb: Some(12),
                program_number: 0,
            }
        );

        // No change when trying to go past last bank
        fixture.backend.bank_select(BankSelect::Inc);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(90),
                bank_lsb: Some(12),
                program_number: 0,
            }
        );

        // Second bank is selected
        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(56),
                bank_lsb: Some(78),
                program_number: 0,
            }
        );

        // First bank is selected
        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );

        // No change when trying to go before first bank
        fixture.backend.bank_select(BankSelect::Dec);
        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );
    }

    #[test]
    fn handle_control_change_messages() {
        let mut fixture = MidiOutFixture::from_bank_settings([], None);

        fixture.backend.control_change(0, 12);

        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: None,
                program_number: 0,
            }
        );

        fixture.backend.control_change(32, 34);

        let received_event = fixture.request_midi_out_event();
        assert_eq!(
            received_event,
            MidiOutEvent {
                device: received_event.device.clone(),
                tuning_method: None,
                bank_msb: Some(12),
                bank_lsb: Some(34),
                program_number: 0,
            }
        );
    }

    struct MidiOutFixture {
        backend: Box<dyn Backend<()>>,
        events: Receiver<PipelineEvent>,
        _midi_input_connection: MidiInputConnection<()>,
    }

    impl MidiOutFixture {
        fn from_bank_settings<const N: usize>(
            banks: [Bank; N],
            default_bank: Option<Bank>,
        ) -> Self {
            let random_device_name = rand::random::<u64>().to_string();

            let _midi_input_connection = MidiInput::new("microwave-test")
                .unwrap()
                .create_virtual(&random_device_name, |_, _, _| {}, ())
                .unwrap();

            let spec = MidiOutSpec {
                note_input: NoteInput::Foreground,
                out_device: random_device_name,
                out_args: MidiOutArgs {
                    out_channel: 0,
                    num_out_channels: 9,
                    device_id: Default::default(),
                    tuning_program: 0,
                },
                tuning_method: TuningMethod::FullKeyboard,
                banks: banks.into(),
                default_bank,
                default_program: None,
            };

            let mut backends = Vec::new();
            let (events_send, events_recv) = flume::unbounded();

            spec.create::<(), PipelineEvent>(&mut backends, &events_send)
                .ok()
                .unwrap();

            assert_eq!(backends.len(), 1);

            Self {
                backend: backends.pop().unwrap(),
                events: events_recv,
                _midi_input_connection,
            }
        }

        fn request_midi_out_event(&mut self) -> MidiOutEvent {
            self.backend.request_status();

            match self.events.recv().unwrap() {
                PipelineEvent::MidiOut(midi_out_event) => midi_out_event,
                other_event => panic!(
                    "Expected {} but got {other_event:?}",
                    any::type_name::<MidiOutEvent>(),
                ),
            }
        }
    }
}
