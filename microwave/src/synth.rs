use std::collections::HashMap;
use std::hash::Hash;
use std::mem;

use flume::Receiver;
use flume::Sender;
use magnetron::automation::AutomatableParam;
use magnetron::automation::AutomationFactory;
use magnetron::stage::Stage;
use magnetron::stage::StageActivity;
use magnetron::Magnetron;
use serde::Deserialize;
use serde::Serialize;
use tune::pitch::Pitch;
use tune::scala::KbmRoot;
use tune::scala::Scl;

use crate::backend::Backend;
use crate::backend::NoteInput;
use crate::control::LiveParameterStorage;
use crate::control::ParameterValue;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::waveform::WaveformProperties;
use crate::magnetron::waveform::WaveformSpec;
use crate::profile::PipelineParam;
use crate::profile::WaveformParam;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MagnetronSpec {
    pub note_input: NoteInput,
    pub num_buffers: usize,
    pub waveforms: Vec<WaveformSpec<WaveformParam>>,
}

impl MagnetronSpec {
    pub fn create<K: Eq + Hash + Send + 'static, E: From<MagnetronEvent> + Send + 'static>(
        &self,
        buffer_size: u32,
        sample_rate: u32,
        templates: &HashMap<String, WaveformParam>,
        envelopes: &HashMap<String, EnvelopeSpec<WaveformParam>>,
        stages: &mut Vec<Stage<PipelineParam>>,
        backends: &mut Vec<Box<dyn Backend<K>>>,
        events: &Sender<E>,
    ) {
        let state = MagnetronState {
            active: HashMap::new(),
            magnetron: Magnetron::new(
                f64::from(sample_rate).recip(),
                self.num_buffers,
                2 * usize::try_from(buffer_size).unwrap(),
            ), // The first invocation of cpal uses the double buffer size
            last_id: 0,
        };

        let (commands_send, commands_recv) = flume::unbounded();

        let backend = MagnetronBackend {
            note_input: self.note_input,
            commands: commands_send,
            events: events.clone(),
            waveforms: self.waveforms.clone(),
            curr_waveform: 0,
            curr_envelope: envelopes.len(), // curr_envelope == num_envelopes means default envelope
            envelope_names: envelopes.keys().cloned().collect(),
            factory: AutomationFactory::new(templates.clone()),
            envelopes: envelopes.clone(),
        };

        backends.push(Box::new(backend));
        stages.push(create_stage(commands_recv, state));
    }
}

struct MagnetronBackend<K, E> {
    note_input: NoteInput,
    commands: Sender<Command<WaveformParam, K>>,
    events: Sender<E>,
    waveforms: Vec<WaveformSpec<WaveformParam>>,
    curr_waveform: usize,
    envelope_names: Vec<String>,
    curr_envelope: usize,
    factory: AutomationFactory<WaveformParam>,
    envelopes: HashMap<String, EnvelopeSpec<WaveformParam>>,
}

impl<K: Send, E: From<MagnetronEvent> + Send> Backend<K> for MagnetronBackend<K, E> {
    fn note_input(&self) -> NoteInput {
        self.note_input
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        self.events
            .send(
                MagnetronEvent {
                    waveform_number: self.curr_waveform,
                    waveform_name: self.waveforms[self.curr_waveform].name.to_owned(),
                    envelope_name: self.selected_envelope().to_owned(),
                    is_default_envelope: self.curr_envelope < self.envelope_names.len(),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, key_id: K, _degree: i32, pitch: Pitch, velocity: u8) {
        let selected_envelope = self.selected_envelope().to_owned();

        let waveform_spec = &mut self.waveforms[self.curr_waveform];
        let default_envelope = mem::replace(&mut waveform_spec.envelope, selected_envelope);
        let waveform = waveform_spec.create(&mut self.factory, &self.envelopes);
        waveform_spec.envelope = default_envelope;

        self.send(Command {
            key_id,
            action: Action::Start {
                waveform,
                pitch,
                velocity: velocity.as_f64(),
            },
        });
    }

    fn update_pitch(&mut self, key_id: K, _degree: i32, pitch: Pitch, _velocity: u8) {
        // Should we update the velocity as well?
        self.send(Command {
            key_id,
            action: Action::UpdatePitch { pitch },
        });
    }

    fn update_pressure(&mut self, key_id: K, pressure: u8) {
        self.send(Command {
            key_id,
            action: Action::UpdatePressure {
                pressure: f64::from(pressure) / 127.0,
            },
        });
    }

    fn stop(&mut self, key_id: K, velocity: u8) {
        self.send(Command {
            key_id,
            action: Action::Stop {
                velocity: velocity.as_f64(),
            },
        });
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_waveform = update_fn(self.curr_waveform).min(self.waveforms.len() - 1);
    }

    fn control_change(&mut self, _controller: u8, _value: u8) {}

    fn channel_pressure(&mut self, _pressure: u8) {}

    fn pitch_bend(&mut self, _value: i16) {}

    fn toggle_envelope_type(&mut self) {
        self.curr_envelope = (self.curr_envelope + 1) % (self.envelope_names.len() + 1);
    }

    fn has_legato(&self) -> bool {
        true
    }
}

impl<K, E> MagnetronBackend<K, E> {
    fn send(&self, command: Command<WaveformParam, K>) {
        self.commands
            .send(command)
            .unwrap_or_else(|_| log::error!("Main audio thread stopped"))
    }

    fn selected_envelope(&self) -> &str {
        self.envelope_names
            .get(self.curr_envelope)
            .unwrap_or(&self.waveforms[self.curr_waveform].envelope)
    }
}

struct Command<A: AutomatableParam, K> {
    key_id: K,
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

struct MagnetronState<A: AutomatableParam, K> {
    active: ActiveWaveforms<A, K>,
    magnetron: Magnetron,
    last_id: u64,
}

type ActiveWaveforms<A, K> = HashMap<ActiveWaveformId<K>, (Vec<Stage<A>>, WaveformProperties)>;

#[derive(Eq, Hash, PartialEq)]
enum ActiveWaveformId<S> {
    Stable(S),
    Fading(u64),
}

fn create_stage<K: Eq + Hash + Send + 'static>(
    commands: Receiver<Command<WaveformParam, K>>,
    mut state: MagnetronState<WaveformParam, K>,
) -> Stage<PipelineParam> {
    Stage::new(
        move |buffers, context: (&(), &LiveParameterStorage, &HashMap<String, f64>)| {
            for message in commands.try_iter() {
                state.handle_command(message)
            }

            state.active.retain(|_, waveform| {
                let reset = buffers.reset();

                state
                    .magnetron
                    .prepare_nested(buffers)
                    .process((&waveform.1, context.1, context.2), &mut waveform.0)
                    >= StageActivity::External
                    && !reset
            });

            StageActivity::Internal
        },
    )
}

impl<A: AutomatableParam, K: Eq + Hash> MagnetronState<A, K> {
    fn handle_command(&mut self, command: Command<A, K>) {
        match command.action {
            Action::Start {
                waveform,
                pitch,
                velocity,
            } => {
                let properties = WaveformProperties::initial(pitch.as_hz(), velocity);
                self.active.insert(
                    ActiveWaveformId::Stable(command.key_id),
                    (waveform, properties),
                );
            }
            Action::UpdatePitch { pitch } => {
                if let Some(waveform) = self
                    .active
                    .get_mut(&ActiveWaveformId::Stable(command.key_id))
                {
                    waveform.1.pitch_hz = pitch.as_hz();
                }
            }
            Action::UpdatePressure { pressure } => {
                if let Some(waveform) = self
                    .active
                    .get_mut(&ActiveWaveformId::Stable(command.key_id))
                {
                    waveform.1.key_pressure = Some(pressure)
                }
            }
            Action::Stop { velocity } => {
                if let Some(mut waveform) = self
                    .active
                    .remove(&ActiveWaveformId::Stable(command.key_id))
                {
                    waveform.1.off_velocity = Some(velocity);
                    self.active
                        .insert(ActiveWaveformId::Fading(self.last_id), waveform);
                    self.last_id += 1;
                }
            }
        }
    }
}

pub struct MagnetronEvent {
    pub waveform_number: usize,
    pub waveform_name: String,
    pub envelope_name: String,
    pub is_default_envelope: bool,
}
