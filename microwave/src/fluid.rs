use std::{
    fmt::Debug,
    fs::File,
    hash::Hash,
    path::Path,
    sync::{mpsc::Sender, Arc},
};

use fluid_xenth::{
    oxisynth::{MidiEvent, SoundFont, SynthDescriptor},
    TunableFluid, Xenth,
};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};
use tune_cli::CliResult;

use crate::{piano::Backend, tunable::TunableBackend};

pub struct FluidBackend<I, S> {
    backend: TunableBackend<S, TunableFluid>,
    soundfont_file_location: Option<Arc<str>>,
    info_sender: Sender<I>,
}

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    soundfont_file_location: Option<&Path>,
    sample_rate: f64,
) -> CliResult<(FluidBackend<I, S>, FluidSynth)> {
    let synth_descriptor = SynthDescriptor {
        sample_rate: sample_rate as f32,
        ..Default::default()
    };

    let (mut xenth, xenth_control) = fluid_xenth::create::<S>(synth_descriptor, 16).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        let mut soundfont_file = File::open(soundfont_file_location)?;
        let soundfont = SoundFont::load(&mut soundfont_file)
            .map_err(|()| "Could not load soundfont".to_owned())?;
        xenth.synth_mut().add_font(soundfont, false);
    }

    Ok((
        FluidBackend {
            backend: TunableBackend::new(xenth_control.into_iter().next().unwrap()),
            soundfont_file_location: soundfont_file_location
                .and_then(Path::to_str)
                .map(|l| l.to_owned().into()),
            info_sender,
        },
        FluidSynth { xenth },
    ))
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
        let soundfont_file_location = self.soundfont_file_location.clone();
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
                                soundfont_file_location: soundfont_file_location.clone(),
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

pub struct FluidSynth {
    xenth: Xenth,
}

impl FluidSynth {
    pub fn write(&mut self, buffer: &mut [f64]) {
        let mut index = 0;
        self.xenth
            .write(buffer.len() / 2, |(l, r)| {
                buffer[index] = f64::from(l);
                index += 1;
                buffer[index] = f64::from(r);
                index += 1;
            })
            .unwrap();
    }
}

pub struct FluidInfo {
    pub soundfont_file_location: Option<Arc<str>>,
    pub program: Option<u32>,
    pub program_name: Option<String>,
    pub is_tuned: bool,
}
