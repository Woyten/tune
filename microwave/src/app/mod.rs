mod input;
mod resources;
mod view;

use std::slice;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::PresentMode;
use clap::ValueEnum;
use flume::Receiver;
use flume::Sender;
use input::InputPlugin;
pub use resources::virtual_keyboard::VirtualKeyboardResource;
use tune::note::NoteLetter;
use tune::pitch::Pitched;
use tune::scala::Scl;

use crate::app::resources::MainViewResource;
use crate::app::resources::MenuStackResource;
use crate::app::resources::PianoEngineResource;
use crate::app::resources::PianoEngineStateResource;
use crate::app::resources::PipelineEventsResource;
use crate::app::view::ViewPlugin;
use crate::lumatone::LumatoneLayout;
use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::pipeline::PipelineEvent;

pub fn start(
    engine: Arc<PianoEngine>,
    engine_state: PianoEngineState,
    physical_layout: PhysicalKeyboardLayout,
    virtual_keyboard: VirtualKeyboardResource,
    odd_limit: u16,
    events: Receiver<PipelineEvent>,
    lumatone_send: Option<Sender<LumatoneLayout>>,
) {
    if let Some(lumatone_send) = &lumatone_send {
        lumatone_send
            .send(LumatoneLayout::from_virtual_keyboard(&virtual_keyboard))
            .unwrap();
    }

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Microwave - Microtonal Waveform Synthesizer by Woyten".to_owned(),
                    resolution: (1280.0, 640.0).into(),
                    present_mode: PresentMode::AutoVsync,
                    // Only relevant for WASM environment
                    canvas: Some("#app".to_owned()),
                    ..default()
                }),
                ..default()
            }),
            InputPlugin,
            ViewPlugin,
        ))
        .insert_resource(physical_layout)
        .insert_resource(virtual_keyboard)
        .insert_resource(PianoEngineResource(engine))
        .insert_resource(PianoEngineStateResource(engine_state))
        .insert_resource(PipelineEventsResource(events))
        .insert_resource(MenuStackResource::default())
        .insert_resource(MainViewResource {
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            odd_limit,
        })
        .insert_resource(LumatoneConnection(lumatone_send))
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
pub struct Toggle<T> {
    options: Vec<T>,
    curr_index: usize,
}

impl<T> Toggle<T> {
    pub fn curr_index(&self) -> usize {
        self.curr_index
    }

    pub fn toggle_next(&mut self) {
        self.curr_index = (self.curr_index + 1) % self.options.len();
    }

    pub fn inc(&mut self) {
        self.curr_index = (self.curr_index.saturating_add(1)).min(self.options.len() - 1);
    }

    pub fn dec(&mut self) {
        self.curr_index = self.curr_index.saturating_sub(1);
    }

    pub fn curr_option(&self) -> &T {
        &self.options[self.curr_index]
    }

    pub fn curr_option_mut(&mut self) -> &mut T {
        &mut self.options[self.curr_index]
    }
}

impl<'a, T> IntoIterator for &'a mut Toggle<T> {
    type Item = &'a mut T;

    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.options.iter_mut()
    }
}

impl<T> From<Vec<T>> for Toggle<T> {
    fn from(options: Vec<T>) -> Self {
        Toggle {
            options,
            curr_index: 0,
        }
    }
}

#[derive(Resource)]
struct LumatoneConnection(Option<Sender<LumatoneLayout>>);
