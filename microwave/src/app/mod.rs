use std::{any::Any, fmt, sync::Arc};

use bevy::{prelude::*, window::PresentMode};
use crossbeam::channel::Receiver;
use tune::{key::Keyboard, note::NoteLetter, pitch::Pitched, scala::Scl};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor, KeyboardLayout,
};

use self::{
    input::InputPlugin,
    model::{OnScreenKeyboards, PianoEngineResource, PianoEngineStateResource, ViewModel},
    view::ViewPlugin,
};

mod input;
mod keyboard;
mod model;
mod view;

#[allow(clippy::too_many_arguments)]
pub fn start(
    engine: Arc<PianoEngine>,
    engine_state: PianoEngineState,
    scale_keyboard_colors: Vec<KeyColor>,
    keyboard: Keyboard,
    layout: KeyboardLayout,
    odd_limit: u16,
    info_updates: Receiver<DynBackendInfo>,
    resources: Vec<Box<dyn Any>>,
) {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Microwave - Microtonal Waveform Synthesizer by Woyten".to_owned(),
                    resolution: (1280.0, 640.0).into(),
                    present_mode: PresentMode::AutoVsync,
                    // Only relevant for WASM environment
                    canvas: Some("#app".to_owned()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
            InputPlugin { keyboard, layout },
            ViewPlugin {
                info_updates: info_updates.into(),
            },
        ))
        .insert_resource(PianoEngineResource(engine))
        .insert_resource(PianoEngineStateResource(engine_state))
        .insert_resource(ViewModel {
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            on_screen_keyboards: OnScreenKeyboards::Scale,
            scale_keyboard_colors,
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            odd_limit,
        })
        .insert_non_send_resource(resources)
        .run();
}

#[derive(Resource)]
pub struct DynBackendInfo(pub Box<dyn BackendInfo>);

pub trait BackendInfo: Sync + Send + 'static {
    fn description(&self) -> &'static str;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}
