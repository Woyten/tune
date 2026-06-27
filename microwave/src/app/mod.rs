mod input;
mod resources;
mod view;

use std::any::Any;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy::window::WindowResolution;
use clap::ValueEnum;
use flume::Receiver;
use input::InputPlugin;

use crate::app::resources::PipelineEventsResource;
use crate::app::resources::ViewSettings;
use crate::app::view::ViewPlugin;
use crate::piano::PianoEngine;
use crate::pipeline::PipelineEvent;

pub fn start(
    engine: PianoEngine,
    physical_layout: PhysicalKeyboardLayout,
    odd_limit: u16,
    events: Receiver<PipelineEvent>,
    resources: Vec<Box<dyn Any>>,
) {
    let mut app = App::new();
    app.add_plugins((
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
    .insert_resource(engine.capture_state())
    .insert_resource(engine)
    .insert_resource(PipelineEventsResource(events))
    .insert_resource(resources::build_menu())
    .insert_resource(ViewSettings::new(odd_limit))
    .insert_non_send(resources);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, start_audio_streams_on_user_input);
    app.run();
}

/// System required to start the audio stream on the first user input, as some browsers require user interaction before allowing audio playback.
#[cfg(target_arch = "wasm32")]
fn start_audio_streams_on_user_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut audio_started: Local<bool>,
    resources: NonSend<Vec<Box<dyn Any>>>,
) {
    use cpal::Stream;
    use cpal::traits::StreamTrait;

    if !*audio_started
        && (keys.get_just_pressed().next().is_some()
            || mouse_buttons.get_just_pressed().next().is_some())
    {
        for resource in &*resources {
            if let Some(stream) = resource.downcast_ref::<Stream>() {
                stream.play().unwrap();
            }
        }
        *audio_started = true;
    }
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
