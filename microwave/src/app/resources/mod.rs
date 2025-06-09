pub mod virtual_keyboard;

use std::sync::Arc;

use bevy::prelude::Resource;
use flume::Receiver;
use tune::{
    pitch::{Pitch, Ratio},
    scala::Scl,
};

use crate::{
    app::input::MenuMode,
    piano::{PianoEngine, PianoEngineState},
    pipeline::PipelineEvent,
};

#[derive(Resource)]
pub struct PianoEngineResource(pub Arc<PianoEngine>);

#[derive(Resource)]
pub struct PianoEngineStateResource(pub PianoEngineState);

#[derive(Resource)]
pub struct PipelineEventsResource(pub Receiver<PipelineEvent>);

#[derive(Resource)]
pub struct MainViewResource {
    pub viewport_left: Pitch,
    pub viewport_right: Pitch,
    pub reference_scl: Scl,
    pub odd_limit: u16,
}

impl MainViewResource {
    pub fn pitch_range(&self) -> Ratio {
        Ratio::between_pitches(self.viewport_left, self.viewport_right)
    }

    pub fn hor_world_coord(&self, pitch: Pitch) -> f64 {
        Ratio::between_pitches(self.viewport_left, pitch)
            .num_equal_steps_of_size(self.pitch_range())
            - 0.5
    }
}

#[derive(Default, Resource)]
pub struct MenuStackResource(Vec<MenuMode>);

impl MenuStackResource {
    pub fn push(&mut self, mode: MenuMode) {
        self.0.push(mode);
    }

    pub fn pop(&mut self) -> Option<MenuMode> {
        self.0.pop()
    }

    pub fn top(&self) -> Option<&MenuMode> {
        self.0.last()
    }
}
