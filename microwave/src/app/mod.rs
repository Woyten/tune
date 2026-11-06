mod input;
mod shapes;
mod state;
mod view;

use std::any::Any;
use std::sync::Arc;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::PresentMode;
use clap::ValueEnum;
use flume::Receiver;
use input::InputPlugin;
#[cfg(test)]
pub(crate) use state::VisualizerMode;
#[cfg(test)]
pub(crate) use state::VisualizerState;

use crate::app::state::StatePlugin;
use crate::app::view::ViewPlugin;
use crate::magnetron::BufferName;
use crate::piano::PianoEngine;
use crate::pipeline::PipelineEvent;
use crate::recorder::RecorderWorkers;
use crate::visualizer::AudioVisualizer;
use crate::visualizer::VisualizerControl;

pub fn start(
    engine: PianoEngine,
    physical_layout: PhysicalKeyboardLayout,
    odd_limit: u16,
    events: Receiver<PipelineEvent>,
    resources: Vec<Box<dyn Any>>,
    audio_visualizer: AudioVisualizer,
    visualizer_buffer_names: Vec<BufferName>,
    visualizer_control: Arc<VisualizerControl>,
    recorder_workers: RecorderWorkers,
) {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .set(WindowPlugin {
                primary_window: {
                    let mut window = Window {
                        title: "Microwave - Microtonal Waveform Synthesizer by Woyten".to_owned(),
                        present_mode: PresentMode::AutoVsync,
                        canvas: Some("#app".to_owned()),
                        ..default()
                    };
                    window.set_maximized(true);
                    Some(window)
                },
                ..default()
            }),
        StatePlugin {
            engine,
            events,
            odd_limit,
            visualizer_buffer_names,
            visualizer_control,
        },
        InputPlugin { physical_layout },
        ViewPlugin,
    ))
    .insert_non_send(audio_visualizer)
    .insert_non_send(recorder_workers)
    .insert_non_send(resources);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Update, start_audio_streams_on_user_input);
    app.add_systems(Update, poll_recorder_workers);
    app.run();
}

fn poll_recorder_workers(mut workers: NonSendMut<RecorderWorkers>) {
    workers.poll();
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
