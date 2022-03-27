use std::{
    fmt::Debug,
    hash::Hash,
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

use fluid_xenth::{
    fluidlite::{IsPreset, IsSamples, IsSettings, Settings, Synth},
    TunableFluid, Xenth,
};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};

use crate::{piano::Backend, tunable::TunableBackend};

pub struct FluidBackend<S> {
    backend: TunableBackend<S, TunableFluid>,
    status_sender: Sender<bool>,
}

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    soundfont_file_location: Option<&Path>,
    sample_rate: f64,
) -> (FluidBackend<S>, FluidSynth<I>) {
    let settings = Settings::new().unwrap();
    settings
        .str_("synth.drums-channel.active")
        .unwrap()
        .set("no");
    settings.num("synth.sample-rate").unwrap().set(sample_rate);

    let synth = Synth::new(settings).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        synth.sfload(soundfont_file_location, false).unwrap();
    }

    let (xenth, xenth_control) = fluid_xenth::create::<S>(synth, 16);
    let (status_sender, status_receiver) = mpsc::channel();

    (
        FluidBackend {
            backend: TunableBackend::new(xenth_control.into_iter().next().unwrap()),
            status_sender,
        },
        FluidSynth {
            xenth,
            status_receiver,
            soundfont_file_location: soundfont_file_location
                .and_then(Path::to_str)
                .map(|l| l.to_owned().into()),
            info_sender,
        },
    )
}

impl<S: Copy + Eq + Hash + Send + Debug> Backend<S> for FluidBackend<S> {
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        self.backend.set_tuning(tuning);

        self.send_status();
    }

    fn set_no_tuning(&mut self) {
        self.backend.set_no_tuning();

        self.send_status();
    }

    fn send_status(&mut self) {
        self.status_sender.send(self.backend.is_tuned()).unwrap();
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
                let curr_program = usize::try_from(curr_program).unwrap();
                let updated_program = u32::try_from(update_fn(curr_program + 128) % 128).unwrap();
                s.program_change(channel, updated_program)
            }));

        self.send_status();
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.cc(channel, u32::from(controller), u32::from(value))
            }));
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.channel_pressure(channel, u32::from(pressure))
            }));
    }

    fn pitch_bend(&mut self, value: i16) {
        self.backend
            .send_monophonic_message(Box::new(move |s, channel| {
                s.pitch_bend(channel, (value + 8192) as u32)
            }));
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        self.backend.is_aot()
    }
}

pub struct FluidSynth<I> {
    xenth: Xenth,
    status_receiver: Receiver<bool>,
    soundfont_file_location: Option<Arc<str>>,
    info_sender: Sender<I>,
}

impl<I: From<FluidInfo> + Send + 'static> FluidSynth<I> {
    pub fn write<T: IsSamples>(&mut self, buffer: T) {
        self.xenth.write(buffer).unwrap();

        self.send_info();
    }

    fn send_info(&mut self) {
        for is_tuned in self.status_receiver.try_iter() {
            let info_sender = self.info_sender.clone();
            let soundfont_file_location = self.soundfont_file_location.clone();

            let s = self.xenth.synth();
            let preset = s.get_channel_preset(0);
            let program = preset
                .as_ref()
                .and_then(IsPreset::get_num)
                .and_then(|p| u8::try_from(p).ok());
            let program_name = preset
                .as_ref()
                .and_then(IsPreset::get_name)
                .map(str::to_owned);
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
    }
}

pub struct FluidInfo {
    pub soundfont_file_location: Option<Arc<str>>,
    pub program: Option<u8>,
    pub program_name: Option<String>,
    pub is_tuned: bool,
}
