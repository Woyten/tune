use std::{fmt::Debug, hash::Hash, sync::Arc};

use fluid_xenth::{
    oxisynth::{MidiEvent, SoundFont, SynthDescriptor},
    TunableFluid,
};
use flume::Sender;
use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::Stage,
};
use serde::{Deserialize, Serialize};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};
use tune_cli::shared::error::ResultExt;

use crate::{
    backend::{Backend, Backends, IdleBackend, NoteInput},
    portable,
    tunable::TunableBackend,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FluidSpec<A> {
    pub out_buffers: (usize, usize),
    pub out_levels: Option<(A, A)>,
    pub note_input: NoteInput,
    pub soundfont_location: String,
}

impl<A: AutomatableParam> FluidSpec<A> {
    pub async fn create<
        K: Copy + Eq + Hash + Send + 'static + Debug,
        E: From<FluidEvent> + From<FluidError> + Send + 'static,
    >(
        &self,
        sample_rate: u32,
        factory: &mut AutomationFactory<A>,
        stages: &mut Vec<Stage<A>>,
        backends: &mut Backends<K>,
        events: &Sender<E>,
    ) {
        let soundfont = match portable::read_file(&self.soundfont_location)
            .await
            .and_then(|maybe_file| maybe_file.ok_or_else(|| "Soundfont file not found".to_owned()))
            .and_then(|mut soundfont_file| {
                SoundFont::load(&mut soundfont_file).handle_error("Could not load soundfont")
            }) {
            Ok(soundfont) => soundfont,
            Err(error_message) => {
                let fluid_error = FluidError {
                    soundfont_location: self.soundfont_location.to_owned().into(),
                    error_message,
                };
                backends.push(Box::new(IdleBackend::new(events, fluid_error)));
                return;
            }
        };

        let synth_descriptor = SynthDescriptor {
            sample_rate: sample_rate as f32,
            ..Default::default()
        };

        let (mut xenth, xenth_control) = fluid_xenth::create::<K>(synth_descriptor, 16).unwrap();
        xenth.synth_mut().add_font(soundfont, false);

        let mut backend = FluidBackend {
            note_input: self.note_input,
            backend: TunableBackend::new(xenth_control.into_iter().next().unwrap()),
            soundfont_location: self.soundfont_location.to_owned().into(),
            events: events.clone(),
        };
        backend.program_change(Box::new(|_| 0));

        let out_buffers = self.out_buffers;
        let stage = factory
            .automate(&self.out_levels)
            .into_stage(move |buffers, out_levels| {
                let mut next_sample = xenth.read().unwrap();
                buffers.read_0_write_2(
                    (
                        BufferIndex::Internal(out_buffers.0),
                        BufferIndex::Internal(out_buffers.1),
                    ),
                    out_levels,
                    || {
                        let next_sample = next_sample();
                        (f64::from(next_sample.0), f64::from(next_sample.1))
                    },
                )
            });

        backends.push(Box::new(backend));
        stages.push(stage);
    }
}

struct FluidBackend<K, E> {
    note_input: NoteInput,
    backend: TunableBackend<K, TunableFluid>,
    soundfont_location: Arc<str>,
    events: Sender<E>,
}

impl<K: Copy + Eq + Hash + Send + Debug, E: From<FluidEvent> + Send + 'static> Backend<K>
    for FluidBackend<K, E>
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
        let soundfont_location = self.soundfont_location.clone();
        let events = self.events.clone();

        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                if channel == 0 {
                    let preset = s.channel_preset(0);
                    let program = preset.map(|p| p.num());
                    let program_name = preset.map(|p| p.name()).map(str::to_owned);
                    events
                        .send(
                            FluidEvent {
                                soundfont_location: soundfont_location.clone(),
                                program,
                                program_name,
                                is_tuned,
                            }
                            .into(),
                        )
                        .unwrap();
                }
                Ok(())
            }));
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
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                let (_, _, curr_program) = s.get_program(channel)?;
                let updated_program =
                    u8::try_from(update_fn(usize::try_from(curr_program).unwrap()).min(127))
                        .unwrap();
                s.send_event(MidiEvent::ProgramChange {
                    channel,
                    program_id: updated_program,
                })
            }));
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.send_event(MidiEvent::ControlChange {
                    channel,
                    ctrl: controller,
                    value,
                })
            }));
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.send_event(MidiEvent::ChannelPressure {
                    channel,
                    value: pressure,
                })
            }));
    }

    fn pitch_bend(&mut self, value: i16) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.send_event(MidiEvent::PitchBend {
                    channel,
                    value: u16::try_from(value + 8192).unwrap(),
                })
            }));
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        self.backend.is_aot()
    }
}

pub struct FluidEvent {
    pub soundfont_location: Arc<str>,
    pub program: Option<u32>,
    pub program_name: Option<String>,
    pub is_tuned: bool,
}

#[derive(Clone)]
pub struct FluidError {
    pub soundfont_location: Arc<str>,
    pub error_message: String,
}
