use std::{fmt::Debug, fs::File, hash::Hash, sync::Arc};

use cpal::SampleRate;
use crossbeam::channel::Sender;
use fluid_xenth::{
    oxisynth::{MidiEvent, SoundFont, SynthDescriptor},
    TunableFluid, Xenth,
};
use magnetron::{buffer::BufferIndex, creator::Creator, StageState};
use serde::{Deserialize, Serialize};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};

use crate::{
    audio::AudioStage,
    control::LiveParameter,
    magnetron::source::{LfSource, NoAccess},
    piano::{Backend, DummyBackend},
    tunable::TunableBackend,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FluidSpec {
    pub soundfont_location: String,
    pub out_buffers: (usize, usize),
}

impl FluidSpec {
    pub fn create<
        I: From<FluidInfo> + From<FluidError> + Send + 'static,
        S: Copy + Eq + Hash + Send + 'static + Debug,
    >(
        &self,
        info_sender: &Sender<I>,
        creator: &Creator<LfSource<NoAccess, LiveParameter>>,
        sample_rate: SampleRate,
        backends: &mut Vec<Box<dyn Backend<S>>>,
        stages: &mut Vec<AudioStage>,
    ) {
        let soundfont = match File::open(&self.soundfont_location)
            .map_err(|err| err.to_string())
            .and_then(|mut soundfont_file| {
                SoundFont::load(&mut soundfont_file)
                    .map_err(|()| "Could not load soundfont".to_owned())
            }) {
            Ok(soundfont) => soundfont,
            Err(error_message) => {
                let fluid_error = FluidError {
                    soundfont_location: self.soundfont_location.to_owned().into(),
                    error_message,
                };
                backends.push(Box::new(DummyBackend::new(info_sender, fluid_error)));
                return;
            }
        };

        let synth_descriptor = SynthDescriptor {
            sample_rate: sample_rate.0 as f32,
            ..Default::default()
        };

        let (mut xenth, xenth_control) = fluid_xenth::create::<S>(synth_descriptor, 16).unwrap();

        xenth.synth_mut().add_font(soundfont, false);

        let mut backend = FluidBackend {
            backend: TunableBackend::new(xenth_control.into_iter().next().unwrap()),
            soundfont_location: self.soundfont_location.to_owned().into(),
            info_sender: info_sender.clone(),
        };
        backend.program_change(Box::new(|_| 0));

        backends.push(Box::new(backend));
        stages.push(create_stage(creator, self.out_buffers, xenth));
    }
}

struct FluidBackend<I, S> {
    backend: TunableBackend<S, TunableFluid>,
    soundfont_location: Arc<str>,
    info_sender: Sender<I>,
}

impl<I: From<FluidInfo> + Send + 'static, S: Copy + Eq + Hash + Send + Debug> Backend<S>
    for FluidBackend<I, S>
{
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        self.backend.set_tuning(tuning);
    }

    fn set_no_tuning(&mut self) {
        self.backend.set_no_tuning();
    }

    fn send_status(&mut self) {
        let is_tuned = self.backend.is_tuned();
        let soundfont_location = self.soundfont_location.clone();
        let info_sender = self.info_sender.clone();

        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                if channel == 0 {
                    let preset = s.channel_preset(0);
                    let program = preset.map(|p| p.num());
                    let program_name = preset.map(|p| p.name()).map(str::to_owned);
                    info_sender
                        .send(
                            FluidInfo {
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

    fn start(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.start(id, degree, pitch, velocity);
    }

    fn update_pitch(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.update_pitch(id, degree, pitch, velocity);
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.backend.update_pressure(id, pressure);
    }

    fn stop(&mut self, id: S, velocity: u8) {
        self.backend.stop(id, velocity);
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

fn create_stage(
    creator: &Creator<LfSource<NoAccess, LiveParameter>>,
    out_buffers: (usize, usize),
    mut xenth: Xenth,
) -> AudioStage {
    creator.create_stage((), move |buffers, ()| {
        let mut next_sample = xenth.read().unwrap();
        buffers.read_0_write_2(
            (
                BufferIndex::Internal(out_buffers.0),
                BufferIndex::Internal(out_buffers.1),
            ),
            || {
                let next_sample = next_sample();
                (f64::from(next_sample.0), f64::from(next_sample.1))
            },
        );
        StageState::Active
    })
}

pub struct FluidInfo {
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
