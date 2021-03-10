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
use tune_cli::CliResult;

use crate::{
    assets, audio,
    magnetron::{
        control::Controller,
        envelope::EnvelopeType,
        waveform::{Waveform, WaveformSpec},
        Magnetron,
    },
    piano::Backend,
};

pub fn create<E>(
    waveforms_file_location: &Path,
    pitch_wheel_sensivity: Ratio,
    cc_numbers: ControlChangeNumbers,
    buffer_size: usize,
) -> CliResult<(WaveformSynth<E>, WaveformBackend<E>)> {
    let state = SynthState {
        playing: HashMap::new(),
        storage: ControlStorage {
            values: HashMap::new(),
        },
        magnetron: Magnetron::new(2 * buffer_size), // The first invocation of cpal uses the double buffer size
        damper_pedal_pressure: 0.0,
        pitch_wheel_sensivity,
        pitch_bend: Ratio::default(),
        last_id: 0,
    };

    let (send, recv) = mpsc::channel();

    Ok((
        WaveformSynth {
            messages: recv,
            state,
        },
        WaveformBackend {
            messages: send,
            waveforms: Arc::from(assets::load_waveforms(waveforms_file_location)?),
            curr_waveform: 0,
            cc_numbers,
            envelope_type: None,
        },
    ))
}

pub struct WaveformBackend<E> {
    messages: Sender<Message<E>>,
    waveforms: Arc<[WaveformSpec<SynthControl>]>, // Arc used here in order to prevent cloning of the inner Vec
    curr_waveform: usize,
    cc_numbers: ControlChangeNumbers,
    envelope_type: Option<EnvelopeType>,
}

impl<E: Send> Backend<E> for WaveformBackend<E> {
    fn start(&mut self, id: E, _degree: i32, pitch: Pitch, velocity: u8) {
        let waveform = self.waveforms[self.curr_waveform].create_waveform(
            pitch,
            f64::from(velocity) / 127.0,
            self.envelope_type,
        );
        self.start_note(id, waveform);
    }

    fn update(&mut self, id: E, _degree: i32, pitch: Pitch) {
        self.update_pitch(id, pitch);
    }

    fn stop(&mut self, id: E, _velocity: u8) {
        self.stop_note(id);
    }

    fn update_program(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_waveform =
            update_fn(self.curr_waveform + self.waveforms.len()) % self.waveforms.len();
    }

    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn polyphonic_key_pressure(&mut self, id: E, pressure: u8) {
        self.update_pressure(id, f64::from(pressure) / 127.0);
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        let value = f64::from(value) / 127.0;
        if controller == self.cc_numbers.modulation {
            self.control(SynthControl::Modulation, value);
        }
        if controller == self.cc_numbers.breath {
            self.control(SynthControl::Breath, value);
        }
        if controller == self.cc_numbers.foot {
            self.control(SynthControl::Foot, value);
        }
        if controller == self.cc_numbers.expression {
            self.control(SynthControl::Expression, value);
        }
        if controller == self.cc_numbers.damper {
            self.damper(value);
            self.control(SynthControl::Damper, value);
        }
        if controller == self.cc_numbers.sostenuto {
            self.control(SynthControl::Sostenuto, value);
        }
        if controller == self.cc_numbers.soft {
            self.control(SynthControl::SoftPedal, value);
        }
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.control(SynthControl::ChannelPressure, f64::from(pressure) / 127.0);
    }
}

impl<E> WaveformBackend<E> {
    fn start_note(&self, id: E, waveform: Waveform<ControlStorage>) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::Start { waveform },
        });
    }

    fn update_pitch(&self, id: E, pitch: Pitch) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::UpdatePitch { pitch },
        });
    }

    fn update_pressure(&self, id: E, pressure: f64) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::UpdatePressure { pressure },
        });
    }

    fn stop_note(&self, id: E) {
        self.send(Message::Lifecycle {
            id,
            action: Lifecycle::Stop,
        });
    }

    fn damper(&self, pressure: f64) {
        self.send(Message::DamperPedal { pressure });
    }

    fn pitch_bend(&self, value: u16) {
        self.send(Message::PitchBend {
            bend_level: (f64::from(value) / f64::from(2 << 12)) - 1.0,
        });
    }

    fn control(&self, control: SynthControl, value: f64) {
        self.send(Message::Control { control, value });
    }

    fn send(&self, message: Message<E>) {
        self.messages.send(message).unwrap()
    }
}

pub struct WaveformSynth<E> {
    messages: Receiver<Message<E>>,
    state: SynthState<E>,
}

enum Message<E> {
    Lifecycle { id: E, action: Lifecycle },
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

struct SynthState<E> {
    playing: HashMap<WaveformState<E>, Waveform<ControlStorage>>,
    storage: ControlStorage,
    magnetron: Magnetron,
    damper_pedal_pressure: f64,
    pitch_wheel_sensivity: Ratio,
    pitch_bend: Ratio,
    last_id: u64,
}

#[derive(Eq, Hash, PartialEq)]
enum WaveformState<E> {
    Stable(E),
    Fading(u64),
}

impl<E: Eq + Hash> WaveformSynth<E> {
    pub fn write(&mut self, buffer: &mut [f64], audio_in: &mut Consumer<f32>) {
        for message in self.messages.try_iter() {
            self.state.process_message(message)
        }

        let sample_width = 1.0 / audio::DEFAULT_SAMPLE_RATE;

        let SynthState {
            playing,
            magnetron: buffers,
            storage: control,
            pitch_bend,
            ..
        } = &mut self.state;

        buffers.clear(buffer.len() / 2);
        buffers.set_audio_in(audio_in);

        playing.retain(|id, waveform| {
            if waveform.properties.curr_amplitude < 0.0001 {
                false
            } else {
                if let WaveformState::Stable(_) = id {
                    waveform.properties.pitch_bend = *pitch_bend;
                }
                buffers.write(waveform, control, sample_width);
                true
            }
        });

        for (&out, target) in buffers.total().iter().zip(buffer.chunks_mut(2)) {
            if let [left, right] = target {
                *left += out / 10.0;
                *right += out / 10.0;
            }
        }
    }
}

impl<E: Eq + Hash> SynthState<E> {
    fn process_message(&mut self, message: Message<E>) {
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
                    if let Some(mut waveform) = self.playing.remove(&WaveformState::Stable(id)) {
                        waveform.set_fade(self.damper_pedal_pressure);
                        self.playing
                            .insert(WaveformState::Fading(self.last_id), waveform);
                        self.last_id += 1;
                    }
                }
            },
            Message::DamperPedal { pressure } => {
                let curve = pressure.max(0.0).min(1.0).cbrt();
                self.damper_pedal_pressure = curve;
                for (id, waveform) in &mut self.playing {
                    if let WaveformState::Fading(_) = id {
                        waveform.set_fade(self.damper_pedal_pressure)
                    }
                }
            }
            Message::PitchBend { bend_level } => {
                self.pitch_bend = self.pitch_wheel_sensivity.repeated(bend_level);
            }
            Message::Control { control, value } => {
                self.storage.write(control, value);
            }
        }
    }
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

#[derive(Clone)]
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
    MouseY,
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
