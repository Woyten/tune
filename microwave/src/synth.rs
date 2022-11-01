use std::{
    collections::HashMap,
    hash::Hash,
    sync::mpsc::{self, Receiver, Sender},
};

use magnetron::{
    spec::Creator,
    waveform::{Waveform, WaveformState},
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
    control::{LiveParameter, LiveParameterStorage},
    magnetron::{
        source::{Controller, LfSource},
        WaveformSpec, WaveformStateAndStorage, WaveformsSpec,
    },
    piano::Backend,
};

pub fn create<I, S>(
    info_sender: Sender<I>,
    waveforms: WaveformsSpec<LfSource<LiveParameter>>,
    pitch_wheel_sensitivity: Ratio,
    num_buffers: usize,
    buffer_size: u32,
    sample_rate_hz: f64,
    audio_in: Consumer<f64>,
) -> CliResult<(WaveformBackend<I, S>, WaveformSynth<S>)> {
    let state = SynthState {
        playing: HashMap::new(),
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

    let num_envelopes = waveforms.envelopes.len();
    let envelope_map: HashMap<_, _> = waveforms
        .envelopes
        .iter()
        .map(|spec| (spec.name.clone(), spec.create_envelope()))
        .collect();

    if envelope_map.len() != num_envelopes {
        return Err(CliError::CommandError(
            "The waveforms file contains a duplicate envelope name".to_owned(),
        ));
    }

    Ok((
        WaveformBackend {
            messages: send,
            info_sender,
            waveforms: waveforms.waveforms,
            curr_waveform: 0,
            envelopes: waveforms
                .envelopes
                .into_iter()
                .map(|spec| spec.name)
                .collect(),
            curr_envelope: num_envelopes, // curr_envelope == num_envelopes means default envelope
            creator: Creator::new(envelope_map),
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
    waveforms: Vec<WaveformSpec<LfSource<LiveParameter>>>,
    curr_waveform: usize,
    envelopes: Vec<String>,
    curr_envelope: usize,
    creator: Creator,
}

impl<I: From<WaveformInfo> + Send, S: Send> Backend<S> for WaveformBackend<I, S> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        let (waveform_spec, envelope_name) = self.get_curr_spec();
        self.info_sender
            .send(
                WaveformInfo {
                    waveform_number: self.curr_waveform,
                    waveform_name: waveform_spec.name.to_owned(),
                    envelope_name: envelope_name.to_owned(),
                    is_default_envelope: self.curr_envelope < self.envelopes.len(),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: S, _degree: i32, pitch: Pitch, velocity: u8) {
        let (waveform_spec, envelope_name) = self.get_curr_spec();
        let mut create_waveform_spec =
            waveform_spec.with_pitch_and_velocity(pitch, f64::from(velocity) / 127.0);
        create_waveform_spec.envelope = envelope_name;
        let waveform = self.creator.create(create_waveform_spec).unwrap();
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::Start { waveform },
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
        self.curr_envelope = (self.curr_envelope + 1) % (self.envelopes.len() + 1);
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

    fn get_curr_spec(&self) -> (&WaveformSpec<LfSource<LiveParameter>>, &str) {
        let waveform_spec = &self.waveforms[self.curr_waveform];
        let envelope_spec = self
            .envelopes
            .get(self.curr_envelope)
            .unwrap_or(&waveform_spec.envelope);
        (waveform_spec, envelope_spec)
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
        waveform: Waveform<LfSource<LiveParameter>>,
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
    playing: HashMap<PlayingWaveform<S>, Waveform<LfSource<LiveParameter>>>,
    magnetron: Magnetron,
    pitch_wheel_sensitivity: Ratio,
    pitch_bend: Ratio,
    last_id: u64,
    audio_in_synchronized: bool,
}

#[derive(Eq, Hash, PartialEq)]
enum PlayingWaveform<S> {
    Stable(S),
    Fading(u64),
}

impl<S: Eq + Hash + Send> AudioStage for WaveformSynth<S> {
    fn render(&mut self, buffer: &mut [f64], storage: &LiveParameterStorage) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        let mut context = WaveformStateAndStorage {
            state: WaveformState {
                pitch_hz: 0.0,
                velocity: 0.0,
                key_pressure: 0.0,
                secs_since_pressed: 0.0,
                secs_since_released: 0.0,
            },
            storage: *storage,
        };

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

        let damper_pedal_pressure = storage.read_parameter(LiveParameter::Damper).cbrt();

        self.state.playing.retain(|id, waveform| {
            let note_suspension = match id {
                PlayingWaveform::Stable(_) => 1.0,
                PlayingWaveform::Fading(_) => damper_pedal_pressure,
            };

            context.state = waveform.state;

            self.state
                .magnetron
                .write(waveform, &context, note_suspension);

            waveform.is_active()
        });

        for (&out, target) in self.state.magnetron.mix().iter().zip(buffer.chunks_mut(2)) {
            if let [left, right] = target {
                *left += out / 10.0;
                *right += out / 10.0;
            }
        }
    }

    fn mute(&mut self) {}
}

impl<S: Eq + Hash> SynthState<S> {
    fn process_message(&mut self, message: Message<S>) {
        match message {
            Message::Lifecycle { id, action } => match action {
                Lifecycle::Start { waveform } => {
                    self.playing.insert(PlayingWaveform::Stable(id), waveform);
                }
                Lifecycle::UpdatePitch { pitch } => {
                    if let Some(waveform) = self.playing.get_mut(&PlayingWaveform::Stable(id)) {
                        waveform.state.pitch_hz = pitch.as_hz();
                    }
                }
                Lifecycle::UpdatePressure { pressure } => {
                    if let Some(waveform) = self.playing.get_mut(&PlayingWaveform::Stable(id)) {
                        waveform.state.key_pressure = pressure
                    }
                }
                Lifecycle::Stop => {
                    if let Some(waveform) = self.playing.remove(&PlayingWaveform::Stable(id)) {
                        self.playing
                            .insert(PlayingWaveform::Fading(self.last_id), waveform);
                        self.last_id += 1;
                    }
                }
            },
            Message::PitchBend { bend_level } => {
                let new_pitch_bend = self.pitch_wheel_sensitivity.repeated(bend_level);
                let pitch_bend_difference = new_pitch_bend.deviation_from(self.pitch_bend);
                self.pitch_bend = new_pitch_bend;

                for (state, waveform) in &mut self.playing {
                    match state {
                        PlayingWaveform::Stable(_) => {
                            waveform.state.pitch_hz *= pitch_bend_difference.as_float()
                        }
                        PlayingWaveform::Fading(_) => {}
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

impl Controller for LiveParameter {
    type Storage = LiveParameterStorage;

    fn access(&mut self, storage: &Self::Storage) -> f64 {
        storage.read_parameter(*self)
    }
}
