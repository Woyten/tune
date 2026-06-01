use std::collections::HashMap;
use std::hash::Hash;
use std::iter;
use std::mem;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Acquire;
use std::sync::atomic::Ordering::Release;

use flume::Receiver;
use flume::Sender;
use magnetron::Magnetron;
use magnetron::automation::AutomatableParam;
use magnetron::automation::AutomationFactory;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;
use serde::Deserialize;
use serde::Serialize;
use tune::pitch::Pitch;
use tune::scala::KbmRoot;
use tune::scala::Scl;

use crate::backend::Backend;
use crate::backend::NoteInput;
use crate::backend::ProgramChange;
use crate::control::LiveParameterStorage;
use crate::control::ParameterValue;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::waveform::WaveformProperties;
use crate::magnetron::waveform::WaveformSpec;
use crate::profile::PipelineParam;
use crate::profile::WaveformParam;
use crate::toggle::Direction;
use crate::toggle::Toggle;

#[derive(Deserialize, Serialize)]
pub struct MagnetronSpec {
    pub note_input: NoteInput,

    pub num_buffers: usize,

    pub waveforms: Vec<WaveformSpec<WaveformParam>>,
    pub default_waveform: Option<usize>,
}

impl MagnetronSpec {
    pub fn create<
        K: Eq + Hash + Clone + Send + 'static,
        E: From<MagnetronEvent> + Send + 'static,
    >(
        &self,
        buffer_size: u32,
        sample_rate: u32,
        templates: &HashMap<String, WaveformParam>,
        envelopes: &HashMap<String, EnvelopeSpec<WaveformParam>>,
        stages: &mut Vec<Stage<PipelineParam>>,
        backends: &mut Vec<Box<dyn Backend<K>>>,
        events: &Sender<E>,
    ) {
        let mut waveform_slots = Vec::with_capacity(MAX_POLYPHONY);
        let mut slot_freed_by_index = Vec::with_capacity(MAX_POLYPHONY);
        for _ in 0..MAX_POLYPHONY {
            let freed = Arc::new(AtomicBool::new(false));
            slot_freed_by_index.push(Arc::clone(&freed));
            waveform_slots.push(WaveformSlot {
                waveform: None,
                freed,
            });
        }

        let (commands_send, commands_recv) = flume::unbounded();

        let mut free_slots = Vec::with_capacity(MAX_POLYPHONY);
        for i in (0..MAX_POLYPHONY).rev() {
            free_slots.push(SlotIndex(i));
        }

        let state = MagnetronState {
            waveform_slots,
            magnetron: Magnetron::new(
                f64::from(sample_rate).recip(),
                self.num_buffers,
                2 * usize::try_from(buffer_size).unwrap(),
            ),
        };

        let backend = MagnetronBackend {
            note_input: self.note_input,
            commands: commands_send,
            events: events.clone(),
            waveforms: Toggle::with_initial_index(
                self.waveforms.clone(),
                self.default_waveform.unwrap_or_default(),
            ),
            envelope_names: Toggle::from(
                iter::once(None)
                    .chain(envelopes.keys().cloned().map(Some))
                    .collect::<Vec<_>>(),
            ),
            factory: AutomationFactory::new(templates.clone()),
            envelopes: envelopes.clone(),
            stable_keys: HashMap::new(),
            free_slots,
            slot_freed_by_index,
            fading_slots: Vec::new(),
        };

        backends.push(Box::new(backend));
        stages.push(create_stage(commands_recv, state));
    }
}

pub(crate) const MAX_POLYPHONY: usize = 128;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct SlotIndex(usize);

struct MagnetronBackend<K, E> {
    note_input: NoteInput,
    commands: Sender<Command<WaveformParam>>,
    events: Sender<E>,
    waveforms: Toggle<WaveformSpec<WaveformParam>>,
    envelope_names: Toggle<Option<String>>,
    factory: AutomationFactory<WaveformParam>,
    envelopes: HashMap<String, EnvelopeSpec<WaveformParam>>,
    stable_keys: HashMap<K, SlotIndex>,
    free_slots: Vec<SlotIndex>,
    slot_freed_by_index: Vec<Arc<AtomicBool>>,
    fading_slots: Vec<(SlotIndex, Arc<AtomicBool>)>,
}

impl<K: Eq + Hash + Clone + Send, E: From<MagnetronEvent> + Send> Backend<K>
    for MagnetronBackend<K, E>
{
    fn note_input(&self) -> NoteInput {
        self.note_input
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn request_status(&mut self) {
        self.events
            .send(
                MagnetronEvent {
                    waveform_number: self.waveforms.curr_index(),
                    waveform_name: self.waveforms.curr_option().name.to_owned(),
                    envelope_name: self.selected_envelope().to_owned(),
                    is_default_envelope: self.envelope_names.curr_option().is_none(),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, key_id: K, _degree: i32, pitch: Pitch, velocity: u8) {
        let Some(slot) = self.allocate_slot(&key_id) else {
            return;
        };

        let selected_envelope = self.selected_envelope().to_owned();

        let waveform_spec = self.waveforms.curr_option_mut();
        let default_envelope = mem::replace(&mut waveform_spec.envelope, selected_envelope);
        let waveform = waveform_spec.create(&mut self.factory, &self.envelopes);
        self.waveforms.curr_option_mut().envelope = default_envelope;

        self.send(Command {
            slot,
            action: Action::Start {
                waveform,
                pitch,
                velocity: velocity.as_f64(),
            },
        });
    }

    fn update_pitch(&mut self, key_id: K, _degree: i32, pitch: Pitch, _velocity: u8) {
        if let Some(&slot) = self.stable_keys.get(&key_id) {
            self.send(Command {
                slot,
                action: Action::UpdatePitch { pitch },
            });
        }
    }

    fn update_pressure(&mut self, key_id: K, pressure: u8) {
        if let Some(&slot) = self.stable_keys.get(&key_id) {
            self.send(Command {
                slot,
                action: Action::UpdatePressure {
                    pressure: f64::from(pressure) / 127.0,
                },
            });
        }
    }

    fn stop(&mut self, key_id: K, velocity: u8) {
        if let Some(slot) = self.stable_keys.remove(&key_id) {
            self.fading_slots
                .push((slot, Arc::clone(&self.slot_freed_by_index[slot.0])));
            self.send(Command {
                slot,
                action: Action::Stop {
                    velocity: velocity.as_f64(),
                },
            });
        }
    }

    fn program_change(&mut self, program_change: ProgramChange) {
        match program_change {
            ProgramChange::ProgramId(program_id) => {
                let program_id = usize::from(program_id);
                if !self.waveforms.set_curr_index(program_id) {
                    log::warn!("Ignoring invalid waveform number: {program_id}");
                }
            }
            ProgramChange::Directional(direction) => self.waveforms.switch(direction),
        };
    }

    fn control_change(&mut self, _controller: u8, _value: u8) {}

    fn channel_pressure(&mut self, _pressure: u8) {}

    fn pitch_bend(&mut self, _value: i16) {}

    fn switch_bank(&mut self, _direction: Direction) {}

    fn switch_envelope_type(&mut self, direction: Direction) {
        self.envelope_names.switch(direction);
    }

    fn has_legato(&self) -> bool {
        true
    }
}

impl<K, E> MagnetronBackend<K, E> {
    fn send(&self, command: Command<WaveformParam>) {
        self.commands
            .send(command)
            .unwrap_or_else(|_| log::error!("Main audio thread stopped"))
    }

    fn selected_envelope(&self) -> &str {
        self.envelope_names
            .curr_option()
            .as_ref()
            .unwrap_or(&self.waveforms.curr_option().envelope)
    }
}

impl<K: Eq + Hash + Clone, E> MagnetronBackend<K, E> {
    fn allocate_slot(&mut self, key_id: &K) -> Option<SlotIndex> {
        self.reclaim_freed_slots();

        if let Some(old_slot) = self.stable_keys.get(key_id).copied() {
            self.free_slots.push(old_slot);
        }
        let slot = self.free_slots.pop();
        if let Some(s) = slot {
            self.stable_keys.insert(key_id.clone(), s);
        } else {
            log::warn!("Polyphony limit reached, dropping note");
        }
        slot
    }

    fn reclaim_freed_slots(&mut self) {
        self.fading_slots.retain(|(slot, freed)| {
            if freed.swap(false, Acquire) {
                self.free_slots.push(*slot);
                false
            } else {
                true
            }
        });
    }
}

struct Command<A: AutomatableParam> {
    slot: SlotIndex,
    action: Action<A>,
}

enum Action<A: AutomatableParam> {
    Start {
        waveform: Vec<Stage<A>>,
        pitch: Pitch,
        velocity: f64,
    },
    UpdatePitch {
        pitch: Pitch,
    },
    UpdatePressure {
        pressure: f64,
    },
    Stop {
        velocity: f64,
    },
}

struct ActiveWaveform<A: AutomatableParam> {
    stages: Vec<Stage<A>>,
    properties: WaveformProperties,
}

struct WaveformSlot<A: AutomatableParam> {
    waveform: Option<ActiveWaveform<A>>,
    freed: Arc<AtomicBool>,
}

struct MagnetronState<A: AutomatableParam> {
    waveform_slots: Vec<WaveformSlot<A>>,
    magnetron: Magnetron,
}

fn create_stage(
    commands: Receiver<Command<WaveformParam>>,
    mut state: MagnetronState<WaveformParam>,
) -> Stage<PipelineParam> {
    Stage::new(
        move |buffers, context: (&(), &LiveParameterStorage, &HashMap<String, f64>)| {
            for message in commands.try_iter() {
                state.handle_command(message)
            }

            let mut any_active = false;
            for slot in state.waveform_slots.iter_mut() {
                let Some(waveform) = slot.waveform.as_mut() else {
                    continue;
                };
                let reset = buffers.reset();

                let active = state.magnetron.prepare_nested(buffers).process(
                    (&waveform.properties, context.1, context.2),
                    &mut waveform.stages,
                ) >= StageActivity::External
                    && !reset;

                if !active {
                    slot.freed.store(true, Release);
                    slot.waveform = None;
                } else {
                    any_active = true;
                }
            }

            if any_active {
                StageActivity::External
            } else {
                StageActivity::Internal
            }
        },
    )
}

impl<A: AutomatableParam> MagnetronState<A> {
    fn handle_command(&mut self, command: Command<A>) {
        let SlotIndex(idx) = command.slot;
        let Some(slot) = self.waveform_slots.get_mut(idx) else {
            return;
        };
        match command.action {
            Action::Start {
                waveform,
                pitch,
                velocity,
            } => {
                slot.freed.store(false, Release);
                let properties = WaveformProperties::initial(pitch.as_hz(), velocity);
                slot.waveform = Some(ActiveWaveform {
                    stages: waveform,
                    properties,
                });
            }
            Action::UpdatePitch { pitch } => {
                if let Some(waveform) = slot.waveform.as_mut() {
                    waveform.properties.pitch_hz = pitch.as_hz();
                }
            }
            Action::UpdatePressure { pressure } => {
                if let Some(waveform) = slot.waveform.as_mut() {
                    waveform.properties.key_pressure = Some(pressure);
                }
            }
            Action::Stop { velocity } => {
                if let Some(waveform) = slot.waveform.as_mut() {
                    waveform.properties.off_velocity = Some(velocity);
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MagnetronEvent {
    pub waveform_number: usize,
    pub waveform_name: String,
    pub envelope_name: String,
    pub is_default_envelope: bool,
}
