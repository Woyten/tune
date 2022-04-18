use std::{
    collections::HashMap,
    hash::Hash,
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

use ringbuf::Consumer;
use serde::{Deserialize, Serialize};
use tune::{
    pitch::{Pitch, Ratio},
    scala::{KbmRoot, Scl},
};
use tune_cli::{CliError, CliResult};

use crate::{
    assets,
    magnetron::{
        control::Controller,
        spec::{EnvelopeSpec, WaveformSpec},
        waveform::Waveform,
        Magnetron,
    },
    piano::Backend,
};

pub fn create<I, S>(
    info_sender: Sender<I>,
    waveforms_file_location: &Path,
    pitch_wheel_sensitivity: Ratio,
    cc_numbers: ControlChangeNumbers,
    num_buffers: usize,
    buffer_size: u32,
    sample_rate_hz: f64,
) -> CliResult<(WaveformBackend<I, S>, WaveformSynth<S>)> {
    let state = SynthState {
        playing: HashMap::new(),
        storage: ControlStorage::default(),
        magnetron: Magnetron::new(
            sample_rate_hz.recip(),
            num_buffers,
            2 * usize::try_from(buffer_size).unwrap(),
        ), // The first invocation of cpal uses the double buffer size
        damper_pedal_pressure: 0.0,
        pitch_wheel_sensitivity,
        last_id: 0,
    };

    let (send, recv) = mpsc::channel();

    let waveforms = assets::load_waveforms(waveforms_file_location)?;
    let num_envelopes = waveforms.envelopes.len();
    let envelope_map: HashMap<_, _> = waveforms
        .envelopes
        .iter()
        .map(|spec| (spec.name.clone(), spec.clone()))
        .collect();

    if envelope_map.len() != num_envelopes {
        return Err(CliError::CommandError(
            "The waveforms file contains a duplicate envelope name".to_owned(),
        ));
    }

    let envelope_map = Arc::new(envelope_map);

    Ok((
        WaveformBackend {
            messages: send,
            info_sender,
            waveforms: waveforms.waveforms,
            curr_waveform: 0,
            envelopes: waveforms.envelopes,
            curr_envelope: num_envelopes,
            envelope_map: envelope_map.clone(),
            cc_numbers,
        },
        WaveformSynth {
            messages: recv,
            state,
            envelope_map,
        },
    ))
}

pub struct WaveformBackend<I, S> {
    messages: Sender<Message<S>>,
    info_sender: Sender<I>,
    waveforms: Vec<WaveformSpec<SynthControl>>,
    curr_waveform: usize,
    envelopes: Vec<EnvelopeSpec>,
    curr_envelope: usize,
    envelope_map: Arc<HashMap<String, EnvelopeSpec>>,
    cc_numbers: ControlChangeNumbers,
}

impl<I: From<WaveformInfo> + Send, S: Send> Backend<S> for WaveformBackend<I, S> {
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&mut self) {
        let (waveform_spec, envelope_spec) = &self.get_curr_spec();
        self.info_sender
            .send(
                WaveformInfo {
                    waveform_number: self.curr_waveform,
                    waveform_name: waveform_spec.name().to_owned(),
                    envelope_name: envelope_spec.name.to_owned(),
                    is_default_envelope: self.curr_envelope < self.envelopes.len(),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: S, _degree: i32, pitch: Pitch, velocity: u8) {
        let (waveform_spec, envelope_spec) = &self.get_curr_spec();
        let waveform = waveform_spec.create_waveform(
            pitch,
            f64::from(velocity) / 127.0,
            envelope_spec.create_envelope(),
        );
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
        self.curr_waveform =
            update_fn(self.curr_waveform + self.waveforms.len()) % self.waveforms.len();
        self.send_status();
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        let value = f64::from(value) / 127.0;
        if controller == self.cc_numbers.modulation {
            self.send_control(SynthControl::Modulation, value);
        }
        if controller == self.cc_numbers.breath {
            self.send_control(SynthControl::Breath, value);
        }
        if controller == self.cc_numbers.foot {
            self.send_control(SynthControl::Foot, value);
        }
        if controller == self.cc_numbers.expression {
            self.send_control(SynthControl::Expression, value);
        }
        if controller == self.cc_numbers.damper {
            self.send(Message::DamperPedal { pressure: value });
            self.send_control(SynthControl::Damper, value);
        }
        if controller == self.cc_numbers.sostenuto {
            self.send_control(SynthControl::Sostenuto, value);
        }
        if controller == self.cc_numbers.soft {
            self.send_control(SynthControl::SoftPedal, value);
        }
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.send_control(SynthControl::ChannelPressure, f64::from(pressure) / 127.0);
    }

    fn pitch_bend(&mut self, value: i16) {
        self.send(Message::PitchBend {
            bend_level: f64::from(value) / 8192.0,
        });
    }

    fn toggle_envelope_type(&mut self) {
        self.curr_envelope = (self.curr_envelope + 1) % (self.envelopes.len() + 1);
        self.send_status();
    }

    fn has_legato(&self) -> bool {
        true
    }
}

impl<I, S> WaveformBackend<I, S> {
    fn send_control(&self, control: SynthControl, value: f64) {
        self.send(Message::Control { control, value });
    }

    fn send(&self, message: Message<S>) {
        self.messages
            .send(message)
            .unwrap_or_else(|_| println!("[ERROR] The waveform engine has died."))
    }

    fn get_curr_spec(&self) -> (&WaveformSpec<SynthControl>, &EnvelopeSpec) {
        let waveform_spec = &self.waveforms[self.curr_waveform];
        let envelope_spec = self
            .envelopes
            .get(self.curr_envelope)
            .unwrap_or_else(|| &self.envelope_map[&waveform_spec.envelope]);
        (waveform_spec, envelope_spec)
    }
}

pub struct WaveformSynth<S> {
    messages: Receiver<Message<S>>,
    state: SynthState<S>,
    envelope_map: Arc<HashMap<String, EnvelopeSpec>>,
}

enum Message<S> {
    Lifecycle { id: S, action: Lifecycle },
    DamperPedal { pressure: f64 },
    PitchBend { bend_level: f64 },
    Control { control: SynthControl, value: f64 },
}

enum Lifecycle {
    Start { waveform: Waveform<ControlStorage> },
    UpdatePitch { pitch: Pitch },
    UpdatePressure { pressure: f64 },
    Stop,
}

struct SynthState<S> {
    playing: HashMap<WaveformState<S>, Waveform<ControlStorage>>,
    storage: ControlStorage,
    magnetron: Magnetron,
    damper_pedal_pressure: f64,
    pitch_wheel_sensitivity: Ratio,
    last_id: u64,
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformState<S> {
    Stable(S),
    Fading(u64),
}

impl<S: Eq + Hash> WaveformSynth<S> {
    pub fn write(&mut self, buffer: &mut [f64], audio_in: &mut Consumer<f64>) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        self.state.magnetron.clear(buffer.len() / 2);
        self.state
            .magnetron
            .set_audio_in(buffer.len() / 2, audio_in);

        self.state.playing.retain(|id, waveform| {
            let note_suspension = match id {
                WaveformState::Stable(_) => 1.0,
                WaveformState::Fading(_) => self.state.damper_pedal_pressure,
            };
            self.state.magnetron.write(
                waveform,
                &self.envelope_map,
                &self.state.storage,
                note_suspension,
            )
        });

        for (&out, target) in self
            .state
            .magnetron
            .total()
            .iter()
            .zip(buffer.chunks_mut(2))
        {
            if let [left, right] = target {
                *left += out / 10.0;
                *right += out / 10.0;
            }
        }
    }
}

impl<S: Eq + Hash> SynthState<S> {
    fn process_message(&mut self, message: Message<S>) {
        match message {
            Message::Lifecycle { id, action } => match action {
                Lifecycle::Start { waveform } => {
                    self.playing.insert(WaveformState::Stable(id), waveform);
                }
                Lifecycle::UpdatePitch { pitch } => {
                    if let Some(waveform) = self.playing.get_mut(&WaveformState::Stable(id)) {
                        waveform.properties.pitch = pitch;
                    }
                }
                Lifecycle::UpdatePressure { pressure } => {
                    if let Some(waveform) = self.playing.get_mut(&WaveformState::Stable(id)) {
                        waveform.properties.pressure = pressure
                    }
                }
                Lifecycle::Stop => {
                    if let Some(waveform) = self.playing.remove(&WaveformState::Stable(id)) {
                        self.playing
                            .insert(WaveformState::Fading(self.last_id), waveform);
                        self.last_id += 1;
                    }
                }
            },
            Message::DamperPedal { pressure } => {
                let curve = pressure.max(0.0).min(1.0).cbrt();
                self.damper_pedal_pressure = curve;
            }
            Message::PitchBend { bend_level } => {
                self.magnetron
                    .set_pitch_bend(self.pitch_wheel_sensitivity.repeated(bend_level));
            }
            Message::Control { control, value } => {
                self.storage.write(control, value);
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

pub struct ControlChangeNumbers {
    pub modulation: u8,
    pub breath: u8,
    pub foot: u8,
    pub expression: u8,
    pub damper: u8,
    pub sostenuto: u8,
    pub soft: u8,
}

#[derive(Clone, Default)]
pub struct ControlStorage {
    values: HashMap<SynthControl, f64>,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum SynthControl {
    Modulation,
    Breath,
    Foot,
    Expression,
    Damper,
    Sostenuto,
    SoftPedal,
    ChannelPressure,
}

impl Controller for SynthControl {
    type Storage = ControlStorage;

    fn read(&self, storage: &Self::Storage) -> f64 {
        storage.values.get(self).copied().unwrap_or_default()
    }
}

impl ControlStorage {
    pub fn write(&mut self, control: SynthControl, value: f64) {
        self.values.insert(control, value);
    }
}
