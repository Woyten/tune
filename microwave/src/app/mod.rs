use std::{any::Any, fmt, sync::Arc};

use bevy::{prelude::*, window::PresentMode};
use clap::ValueEnum;
use crossbeam::channel::Receiver;
use tune::{
    layout::IsomorphicKeyboard,
    note::NoteLetter,
    pitch::{Pitched, Ratio},
    scala::Scl,
};

use crate::piano::{PianoEngine, PianoEngineState};

use self::{
    input::InputPlugin,
    model::{
        BackendInfoResource, OnScreenKeyboards, PianoEngineResource, PianoEngineStateResource,
        ViewModel,
    },
    view::ViewPlugin,
};

mod input;
mod model;
mod view;

pub fn start(
    engine: Arc<PianoEngine>,
    engine_state: PianoEngineState,
    physical_layout: PhysicalKeyboardLayout,
    virtual_layouts: Vec<VirtualKeyboardLayout>,
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
        .insert_resource(Toggle::from(virtual_layouts))
        .insert_resource(PianoEngineResource(engine))
        .insert_resource(PianoEngineStateResource(engine_state))
        .insert_resource(BackendInfoResource(info_updates))
        .insert_resource(ViewModel {
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            on_screen_keyboards: vec![
                OnScreenKeyboards::Isomorphic,
                OnScreenKeyboards::Scale,
                OnScreenKeyboards::Reference,
                OnScreenKeyboards::IsomorphicAndReference,
                OnScreenKeyboards::ScaleAndReference,
                OnScreenKeyboards::None,
            ]
            .into(),
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

pub struct VirtualKeyboardLayout {
    pub description: String,
    pub keyboard: IsomorphicKeyboard,
    pub num_primary_steps: u16,
    pub num_secondary_steps: u16,
    pub period: Ratio,
    pub colors: Vec<Color>,
}

#[derive(Resource)]
pub struct DynBackendInfo(pub Box<dyn BackendInfo>);

pub trait BackendInfo: Sync + Send + 'static {
    fn description(&self) -> &'static str;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}

#[derive(Resource)]
pub struct Toggle<T> {
    options: Vec<T>,
    curr_option: usize,
}

impl<T> Toggle<T> {
    pub fn toggle_next(&mut self) {
        self.curr_option = (self.curr_option + 1) % self.options.len();
    }

    pub fn curr_option(&self) -> &T {
        &self.options[self.curr_option]
    }
}

impl<T> From<Vec<T>> for Toggle<T> {
    fn from(options: Vec<T>) -> Self {
        Toggle {
            options,
            curr_option: 0,
        }
    }
}
