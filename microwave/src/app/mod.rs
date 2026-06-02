mod input;
pub(crate) mod resources;
mod view;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy::window::WindowResolution;
use clap::ValueEnum;
use flume::Receiver;
use input::InputPlugin;
use tune::note::NoteLetter;
use tune::pitch::Pitched;
use tune::scala::Scl;

use crate::app::resources::KeyboardViewSettings;
use crate::app::resources::MainViewResource;
use crate::app::resources::MenuStackResource;
use crate::app::resources::PipelineEventsResource;
use crate::app::view::ViewPlugin;
use crate::piano::PianoEngine;
use crate::pipeline::PipelineEvent;

pub fn start(
    engine: PianoEngine,
    physical_layout: PhysicalKeyboardLayout,
    view_settings: KeyboardViewSettings,
    odd_limit: u16,
    events: Receiver<PipelineEvent>,
) {
    App::new()
        .add_plugins((
            DefaultPlugins
                .build()
                .disable::<LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Microwave - Microtonal Waveform Synthesizer by Woyten".to_owned(),
                        resolution: WindowResolution::new(1280, 640),
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
        .insert_resource(view_settings)
        .insert_resource(engine.capture_state())
        .insert_resource(engine)
        .insert_resource(PipelineEventsResource(events))
        .insert_resource(MenuStackResource::default())
        .insert_resource(MainViewResource {
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            odd_limit,
        })
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
