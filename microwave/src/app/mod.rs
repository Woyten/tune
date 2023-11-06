use std::{any::Any, fmt, sync::Arc};

use bevy::{prelude::*, window::PresentMode};
use clap::ValueEnum;
use crossbeam::channel::Receiver;
use tune::{
    key::Keyboard,
    note::NoteLetter,
    pitch::{Pitched, Ratio},
    scala::Scl,
};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor,
};

use self::{
    input::InputPlugin,
    model::{
        BackendInfoResource, OnScreenKeyboards, PianoEngineResource, PianoEngineStateResource,
        ViewModel,
    },
    view::ViewPlugin,
};

mod input;
mod keyboard;
mod model;
mod view;

pub fn start(
    engine: Arc<PianoEngine>,
    engine_state: PianoEngineState,
    scale_keyboard_colors: Vec<KeyColor>,
    physical_layout: PhysicalKeyboardLayout,
    virtual_layout: VirtualKeyboardLayout,
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
            InputPlugin,
            ViewPlugin,
        ))
        .insert_resource(physical_layout)
        .insert_resource(virtual_layout)
        .insert_resource(PianoEngineResource(engine))
        .insert_resource(PianoEngineStateResource(engine_state))
        .insert_resource(BackendInfoResource(info_updates))
        .insert_resource(ViewModel {
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            on_screen_keyboards: OnScreenKeyboards::Isomorphic,
            scale_keyboard_colors,
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            odd_limit,
        })
        .insert_non_send_resource(resources)
        .run();
}

#[derive(Clone, Resource, ValueEnum)]
pub enum PhysicalKeyboardLayout {
    #[value(name = "ansi")]
    Ansi,
    #[value(name = "var")]
    Variant,
    #[value(name = "iso")]
    Iso,
}

#[derive(Resource)]
pub struct VirtualKeyboardLayout {
    pub keyboard: Keyboard,
    pub num_primary_steps: u8,
    pub num_secondary_steps: u8,
    pub period: Ratio,
}

#[derive(Resource)]
pub struct DynBackendInfo(pub Box<dyn BackendInfo>);

pub trait BackendInfo: Sync + Send + 'static {
    fn description(&self) -> &'static str;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}
