use std::{
    fmt::Debug,
    hash::Hash,
    path::Path,
    sync::{mpsc::Sender, Arc},
};

use fluid_xenth::{Xenth, XenthControl};
use fluidlite::{IsPreset, IsSamples, IsSettings, Settings, Synth};
use tune::{
    pitch::Pitch,
    scala::{KbmRoot, Scl},
};

use crate::piano::Backend;

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    soundfont_file_location: Option<&Path>,
) -> (FluidBackend<I, S>, FluidSynth) {
    let settings = Settings::new().unwrap();
    settings
        .str_("synth.drums-channel.active")
        .unwrap()
        .set("no");

    let synth = Synth::new(settings).unwrap();

    if let Some(soundfont_file_location) = soundfont_file_location {
        synth.sfload(soundfont_file_location, false).unwrap();
    }

    let (xenth, xenth_control) = fluid_xenth::create(synth, 16);

    (
        FluidBackend {
            xenth_control,
            info_sender,
            soundfont_file_location: soundfont_file_location
                .and_then(Path::to_str)
                .map(|l| l.to_owned().into()),
        },
        FluidSynth { xenth },
    )
}

pub struct FluidBackend<I, S> {
    xenth_control: XenthControl<S>,
    info_sender: Sender<I>,
    soundfont_file_location: Option<Arc<str>>,
}

impl<I: From<FluidInfo> + Send + 'static, S: Copy + Eq + Hash + Send + Debug> Backend<S>
    for FluidBackend<I, S>
{
    fn set_tuning(&mut self, _tuning: (&Scl, KbmRoot)) {}

    fn set_no_tuning(&mut self) {}

    fn send_status(&self) {
        let info_sender = self.info_sender.clone();
        let soundfont_file_location = self.soundfont_file_location.clone();

        self.xenth_control
            .send_command(move |s| {
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
                            soundfont_file_location,
                            program,
                            program_name,
                        }
                        .into(),
                    )
                    .unwrap();
                Ok(())
            })
            .unwrap();
    }

    fn start(&mut self, id: S, _degree: i32, pitch: Pitch, velocity: u8) {
        self.xenth_control.note_on(0, id, pitch, velocity).unwrap();
    }

    fn update_pitch(&mut self, _id: S, _degree: i32, _pitch: Pitch, _velocity: u8) {
        // FluidLite does not update sounding notes.
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.xenth_control.key_pressure(0, &id, pressure).unwrap();
    }

    fn stop(&mut self, id: S, _velocity: u8) {
        self.xenth_control.note_off(0, &id).unwrap();
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.xenth_control
            .send_channel_command(0, move |s, channel| {
                let (_, _, curr_program) = s.get_program(channel)?;
                let curr_program = usize::try_from(curr_program).unwrap();
                let updated_program = u32::try_from(update_fn(curr_program + 128) % 128).unwrap();
                s.program_change(channel, updated_program)
            })
            .unwrap();
        self.send_status();
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.xenth_control
            .send_channel_command(0, move |s, channel| {
                s.cc(channel, u32::from(controller), u32::from(value))
            })
            .unwrap();
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.xenth_control
            .send_channel_command(0, move |s, channel| {
                s.channel_pressure(channel, u32::from(pressure))
            })
            .unwrap();
    }

    fn pitch_bend(&mut self, value: i16) {
        self.xenth_control
            .send_channel_command(0, move |s, channel| {
                s.pitch_bend(channel, (value + 8192) as u32)
            })
            .unwrap();
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        false
    }
}

pub struct FluidSynth {
    xenth: Xenth,
}

impl FluidSynth {
    pub fn write<T: IsSamples>(&mut self, buffer: T) {
        self.xenth.write(buffer).unwrap();
    }
}

pub struct FluidInfo {
    pub soundfont_file_location: Option<Arc<str>>,
    pub program: Option<u8>,
    pub program_name: Option<String>,
}
