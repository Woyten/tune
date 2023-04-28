use std::{collections::HashMap, hash::Hash, mem};

use cpal::SampleRate;
use crossbeam::channel::{self, Receiver, Sender};
use magnetron::{
    creator::Creator,
    envelope::EnvelopeSpec,
    waveform::{Waveform, WaveformProperties},
    Magnetron, Stage, StageState,
};
use serde::{Deserialize, Serialize};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};

use crate::{
    audio::{AudioContext, AudioStage},
    control::{LiveParameter, LiveParameterStorage, ParameterValue},
    magnetron::{source::LfSource, WaveformProperty, WaveformSpec},
    piano::Backend,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MagnetronSpec {
    pub num_buffers: usize,
    pub waveforms: Vec<WaveformSpec<LfSource<WaveformProperty, LiveParameter>>>,
}

impl MagnetronSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn create<I: From<MagnetronInfo> + Send + 'static, S: Eq + Hash + Send + 'static>(
        &self,
        info_sender: &Sender<I>,
        buffer_size: u32,
        sample_rate: SampleRate,
        waveform_templates: &HashMap<String, LfSource<WaveformProperty, LiveParameter>>,
        waveform_envelopes: &HashMap<
            String,
            EnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>,
        >,
        backends: &mut Vec<Box<dyn Backend<S>>>,
        stages: &mut Vec<AudioStage>,
    ) {
        let state = MagnetronState {
            active: HashMap::new(),
            magnetron: Magnetron::new(
                f64::from(sample_rate.0).recip(),
                self.num_buffers,
                2 * usize::try_from(buffer_size).unwrap(),
            ), // The first invocation of cpal uses the double buffer size
            last_id: 0,
        };

        let (message_sender, message_receiver) = channel::unbounded();

        let envelope_names: Vec<_> = waveform_envelopes.keys().cloned().collect();

        let backend = MagnetronBackend {
            message_sender,
            info_sender: info_sender.clone(),
            waveforms: self.waveforms.clone(),
            curr_waveform: 0,
            curr_envelope: envelope_names.len(), // curr_envelope == num_envelopes means default envelope
            envelope_names,
            creator: Creator::new(waveform_templates.clone()),
            waveform_envelopes: waveform_envelopes.clone(),
        };

        backends.push(Box::new(backend));
        stages.push(create_stage(message_receiver, state));
    }
}

struct MagnetronBackend<I, S> {
    message_sender: Sender<Message<S>>,
    info_sender: Sender<I>,
    waveforms: Vec<WaveformSpec<LfSource<WaveformProperty, LiveParameter>>>,
    curr_waveform: usize,
    envelope_names: Vec<String>,
    curr_envelope: usize,
    creator: Creator<LfSource<WaveformProperty, LiveParameter>>,
    waveform_envelopes: HashMap<String, EnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>>,
}

impl<I: From<MagnetronInfo> + Send, S: Send> Backend<S> for MagnetronBackend<I, S> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        self.info_sender
            .send(
                MagnetronInfo {
                    waveform_number: self.curr_waveform,
                    waveform_name: self.waveforms[self.curr_waveform].name.to_owned(),
                    envelope_name: self.selected_envelope().to_owned(),
                    is_default_envelope: self.curr_envelope < self.envelope_names.len(),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: S, _degree: i32, pitch: Pitch, velocity: u8) {
        let selected_envelope = self.selected_envelope().to_owned();

        let waveform_spec = &mut self.waveforms[self.curr_waveform];
        let default_envelope = mem::replace(&mut waveform_spec.envelope, selected_envelope);
        let waveform = waveform_spec.use_creator(&self.creator, &self.waveform_envelopes);
        waveform_spec.envelope = default_envelope;

        self.send(Message {
            id,
            action: Action::Start {
                waveform,
                pitch,
                velocity: velocity.as_f64(),
            },
        });
    }

    fn update_pitch(&mut self, id: S, _degree: i32, pitch: Pitch, _velocity: u8) {
        // Should we update the velocity as well?
        self.send(Message {
            id,
            action: Action::UpdatePitch { pitch },
        });
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.send(Message {
            id,
            action: Action::UpdatePressure {
                pressure: f64::from(pressure) / 127.0,
            },
        });
    }

    fn stop(&mut self, id: S, velocity: u8) {
        self.send(Message {
            id,
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

impl<I, S> MagnetronBackend<I, S> {
    fn send(&self, message: Message<S>) {
        self.message_sender
            .send(message)
            .unwrap_or_else(|_| println!("[ERROR] The waveform engine has died."))
    }

    fn selected_envelope(&self) -> &str {
        self.envelope_names
            .get(self.curr_envelope)
            .unwrap_or(&self.waveforms[self.curr_waveform].envelope)
    }
}

struct Message<S> {
    id: S,
    action: Action,
}

enum Action {
    Start {
        waveform: Waveform<(WaveformProperties, LiveParameterStorage)>,
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

struct MagnetronState<S> {
    active: ActiveWaveforms<S>,
    magnetron: Magnetron,
    last_id: u64,
}

type ActiveWaveforms<S> = HashMap<
    ActiveWaveformId<S>,
    (
        Waveform<(WaveformProperties, LiveParameterStorage)>,
        WaveformProperties,
    ),
>;

#[derive(Eq, Hash, PartialEq)]
enum ActiveWaveformId<S> {
    Stable(S),
    Fading(u64),
}

fn create_stage<S: Eq + Hash + Send + 'static>(
    message_receiver: Receiver<Message<S>>,
    mut state: MagnetronState<S>,
) -> AudioStage {
    Stage::new(move |buffers, context: &AudioContext| {
        for message in message_receiver.try_iter() {
            state.process_message(message)
        }

        let mut payload = (WaveformProperties::initial(0.0, 0.0), context.payload.1);

        state.active.retain(|_, waveform| {
            payload.0 = waveform.1;
            state
                .magnetron
                .process_nested(buffers, &payload, waveform.0.stages());
            waveform.0.is_active()
        });
        StageState::Active
    })
}

impl<S: Eq + Hash> MagnetronState<S> {
    fn process_message(&mut self, message: Message<S>) {
        match message.action {
            Action::Start {
                waveform,
                pitch,
                velocity,
            } => {
                let properties = WaveformProperties::initial(pitch.as_hz(), velocity);
                self.active
                    .insert(ActiveWaveformId::Stable(message.id), (waveform, properties));
            }
            Action::UpdatePitch { pitch } => {
                if let Some(waveform) = self.active.get_mut(&ActiveWaveformId::Stable(message.id)) {
                    waveform.1.pitch_hz = pitch.as_hz();
                }
            }
            Action::UpdatePressure { pressure } => {
                if let Some(waveform) = self.active.get_mut(&ActiveWaveformId::Stable(message.id)) {
                    waveform.1.key_pressure = Some(pressure)
                }
            }
            Action::Stop { velocity } => {
                if let Some(mut waveform) =
                    self.active.remove(&ActiveWaveformId::Stable(message.id))
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

pub struct MagnetronInfo {
    pub waveform_number: usize,
    pub waveform_name: String,
    pub envelope_name: String,
    pub is_default_envelope: bool,
}
