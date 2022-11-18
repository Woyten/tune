use std::{
    collections::HashMap,
    hash::Hash,
    mem,
    sync::mpsc::{self, Receiver, Sender},
};

use magnetron::{
    automation::AutomationContext,
    spec::Creator,
    waveform::{Waveform, WaveformProperties},
    Magnetron,
};
use ringbuf::Consumer;
use tune::{
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
};
use tune_cli::{CliError, CliResult};

use crate::{
    audio::AudioStage,
    control::{LiveParameter, LiveParameterStorage, ParameterValue},
    magnetron::{
        source::{LfSource, StorageAccess},
        AudioSpec, WaveformProperty, WaveformSpec,
    },
    piano::Backend,
};

pub fn create<I, S>(
    info_sender: Sender<I>,
    waveforms: AudioSpec,
    pitch_wheel_sensitivity: Ratio,
    num_buffers: usize,
    buffer_size: u32,
    sample_rate_hz: f64,
    audio_in: Consumer<f64>,
) -> CliResult<(WaveformBackend<I, S>, WaveformSynth<S>)> {
    let state = SynthState {
        active: HashMap::new(),
        magnetron: Magnetron::new(
            sample_rate_hz.recip(),
            num_buffers,
            2 * usize::try_from(buffer_size).unwrap(),
        ), // The first invocation of cpal uses the double buffer size
        pitch_wheel_sensitivity,
        pitch_bend: Default::default(),
        last_id: 0,
        audio_in_synchronized: false,
    };

    let (send, recv) = mpsc::channel();

    let envelope_names: Vec<_> = waveforms
        .envelopes
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect();

    let envelopes: HashMap<_, _> = waveforms
        .envelopes
        .into_iter()
        .map(|spec| (spec.name, spec.spec))
        .collect();

    if envelopes.len() != envelope_names.len() {
        return Err(CliError::CommandError(
            "The config file contains a duplicate envelope name".to_owned(),
        ));
    }

    Ok((
        WaveformBackend {
            messages: send,
            info_sender,
            waveforms: waveforms.waveforms,
            curr_waveform: 0,
            curr_envelope: envelope_names.len(), // curr_envelope == num_envelopes means default envelope
            envelope_names,
            creator: Creator::new(envelopes),
        },
        WaveformSynth {
            messages: recv,
            state,
            audio_in,
        },
    ))
}

pub struct WaveformBackend<I, S> {
    messages: Sender<Message<S>>,
    info_sender: Sender<I>,
    waveforms: Vec<WaveformSpec<LfSource<WaveformProperty, LiveParameter>>>,
    curr_waveform: usize,
    envelope_names: Vec<String>,
    curr_envelope: usize,
    creator: Creator<LfSource<WaveformProperty, LiveParameter>>,
}

impl<I: From<WaveformInfo> + Send, S: Send> Backend<S> for WaveformBackend<I, S> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        self.info_sender
            .send(
                WaveformInfo {
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
        let waveform = self.creator.create(&*waveform_spec).unwrap();
        waveform_spec.envelope = default_envelope;

        let properties = WaveformProperties::initial(pitch.as_hz(), velocity.as_f64());

        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::Start {
                waveform,
                properties,
            },
        });
    }

    fn update_pitch(&mut self, id: S, _degree: i32, pitch: Pitch, _velocity: u8) {
        // Should we update the velocity as well?
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::UpdatePitch { pitch },
        });
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::UpdatePressure {
                pressure: f64::from(pressure) / 127.0,
            },
        });
    }

    fn stop(&mut self, id: S, _velocity: u8) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::Stop,
        });
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_waveform = update_fn(self.curr_waveform).min(self.waveforms.len() - 1);
    }

    fn control_change(&mut self, _controller: u8, _value: u8) {}

    fn channel_pressure(&mut self, _pressure: u8) {}

    fn pitch_bend(&mut self, value: i16) {
        self.send(Message::PitchBend {
            bend_level: f64::from(value) / 8192.0,
        });
    }

    fn toggle_envelope_type(&mut self) {
        self.curr_envelope = (self.curr_envelope + 1) % (self.envelope_names.len() + 1);
    }

    fn has_legato(&self) -> bool {
        true
    }
}

impl<I, S> WaveformBackend<I, S> {
    fn send(&self, message: Message<S>) {
        self.messages
            .send(message)
            .unwrap_or_else(|_| println!("[ERROR] The waveform engine has died."))
    }

    fn selected_envelope(&self) -> &str {
        self.envelope_names
            .get(self.curr_envelope)
            .unwrap_or(&self.waveforms[self.curr_waveform].envelope)
    }
}

pub struct WaveformSynth<S> {
    messages: Receiver<Message<S>>,
    state: SynthState<S>,
    audio_in: Consumer<f64>,
}

enum Message<S> {
    Lifecycle { id: S, action: Lifecycle },
    PitchBend { bend_level: f64 },
}

enum Lifecycle {
    Start {
        waveform: Waveform<(WaveformProperties, LiveParameterStorage)>,
        properties: WaveformProperties,
    },
    UpdatePitch {
        pitch: Pitch,
    },
    UpdatePressure {
        pressure: f64,
    },
    Stop,
}

struct SynthState<S> {
    active: HashMap<ActiveWaveformId<S>, ActiveWaveform>,
    magnetron: Magnetron,
    pitch_wheel_sensitivity: Ratio,
    pitch_bend: Ratio,
    last_id: u64,
    audio_in_synchronized: bool,
}

#[derive(Eq, Hash, PartialEq)]
enum ActiveWaveformId<S> {
    Stable(S),
    Fading(u64),
}

type ActiveWaveform = (
    Waveform<(WaveformProperties, LiveParameterStorage)>,
    WaveformProperties,
);

impl<S: Eq + Hash + Send> AudioStage<((), LiveParameterStorage)> for WaveformSynth<S> {
    fn render(
        &mut self,
        buffer: &mut [f64],
        context: &AutomationContext<((), LiveParameterStorage)>,
    ) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        let mut context = (
            WaveformProperties {
                pitch_hz: 0.0,
                velocity: 0.0,
                key_pressure: 0.0,
                fadeout: 0.0,
            },
            context.payload.1,
        );

        self.state.magnetron.clear(buffer.len() / 2);

        if self.audio_in.len() >= buffer.len() {
            if !self.state.audio_in_synchronized {
                self.state.audio_in_synchronized = true;
                println!("[INFO] Audio-in synchronized");
            }
            self.state.magnetron.set_audio_in(|| {
                let l = self.audio_in.pop().unwrap_or_default();
                let r = self.audio_in.pop().unwrap_or_default();
                l + r / 2.0
            });
        } else if self.state.audio_in_synchronized {
            self.state.audio_in_synchronized = false;
            println!("[WARNING] Exchange buffer underrun - Waiting for audio-in to be in sync with audio-out");
        }

        let damper_pedal_pressure = LiveParameter::Damper.access(&context.1).cbrt();
        let volume = LiveParameter::Volume.access(&context.1) / 16.0;

        self.state.active.retain(|id, waveform| {
            let fadeout = match id {
                ActiveWaveformId::Stable(_) => 0.0,
                ActiveWaveformId::Fading(_) => 1.0 - damper_pedal_pressure,
            };

            context.0 = waveform.1;
            context.0.fadeout = fadeout;

            self.state.magnetron.write(&mut waveform.0, &context);

            waveform.0.is_active
        });

        for (&out, target) in self.state.magnetron.mix().iter().zip(buffer.chunks_mut(2)) {
            if let [left, right] = target {
                *left += out * volume;
                *right += out * volume;
            }
        }
    }

    fn mute(&mut self) {}
}

impl<S: Eq + Hash> SynthState<S> {
    fn process_message(&mut self, message: Message<S>) {
        match message {
            Message::Lifecycle { id, action } => match action {
                Lifecycle::Start {
                    waveform,
                    properties,
                } => {
                    self.active
                        .insert(ActiveWaveformId::Stable(id), (waveform, properties));
                }
                Lifecycle::UpdatePitch { pitch } => {
                    if let Some(waveform) = self.active.get_mut(&ActiveWaveformId::Stable(id)) {
                        waveform.1.pitch_hz = pitch.as_hz();
                    }
                }
                Lifecycle::UpdatePressure { pressure } => {
                    if let Some(waveform) = self.active.get_mut(&ActiveWaveformId::Stable(id)) {
                        waveform.1.key_pressure = pressure
                    }
                }
                Lifecycle::Stop => {
                    if let Some(waveform) = self.active.remove(&ActiveWaveformId::Stable(id)) {
                        self.active
                            .insert(ActiveWaveformId::Fading(self.last_id), waveform);
                        self.last_id += 1;
                    }
                }
            },
            Message::PitchBend { bend_level } => {
                let new_pitch_bend = self.pitch_wheel_sensitivity.repeated(bend_level);
                let pitch_bend_difference = new_pitch_bend.deviation_from(self.pitch_bend);
                self.pitch_bend = new_pitch_bend;

                for (state, waveform) in &mut self.active {
                    match state {
                        ActiveWaveformId::Stable(_) => {
                            waveform.1.pitch_hz *= pitch_bend_difference.as_float()
                        }
                        ActiveWaveformId::Fading(_) => {}
                    }
                }
            }
        }
    }
}

pub struct WaveformInfo {
    pub waveform_number: usize,
    pub waveform_name: String,
    pub envelope_name: String,
    pub is_default_envelope: bool,
}
