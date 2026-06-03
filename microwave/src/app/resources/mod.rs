pub mod view_settings;

use bevy::prelude::Resource;
use flume::Receiver;
pub use view_settings::ViewSettings;

use crate::app::input::MenuMode;
use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::pipeline::PipelineEvent;

impl Resource for PianoEngine {}

impl Resource for PianoEngineState {}

#[derive(Resource)]
pub struct PipelineEventsResource(pub Receiver<PipelineEvent>);

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
