use std::{any::Any, fmt, sync::Arc};

use bevy::{prelude::*, window::PresentMode};
use crossbeam::channel::Receiver;
use tune::{key::Keyboard, scala::Scl};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor, KeyboardLayout,
};

use self::{
    input::InputPlugin,
    model::{Model, PianoEngineResource, Viewport},
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
    key_colors: Vec<KeyColor>,
    keyboard: Keyboard,
    layout: KeyboardLayout,
    odd_limit: u16,
    info_updates: Receiver<DynViewInfo>,
    resources: Vec<Box<dyn Any>>,
) {
    let model = Model {
        engine,
        key_colors,
        reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
        odd_limit,
    };

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
            ViewPlugin,
        ))
        .insert_resource(model)
        .insert_resource(PianoEngineResource(engine_state))
        .init_resource::<Viewport>()
        .insert_resource(EventReceiver(info_updates))
        .insert_resource(ClearColor(Color::hex("222222").unwrap()))
        .insert_non_send_resource(resources)
        .run();
}

#[derive(Resource)]
struct EventReceiver<T>(pub Receiver<T>);

#[derive(Resource)]
pub struct DynViewInfo(pub Box<dyn ViewModel>);

pub trait ViewModel: Sync + Send + 'static {
    fn description(&self) -> &'static str;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}
