use std::sync::Arc;

use bevy::prelude::Resource;
use crossbeam::channel::Receiver;
use tune::{
    pitch::{Pitch, Ratio},
    scala::Scl,
};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor,
};

use super::{DynBackendInfo, Toggle};

#[derive(Resource)]
pub struct PianoEngineResource(pub Arc<PianoEngine>);

#[derive(Resource)]
pub struct PianoEngineStateResource(pub PianoEngineState);

#[derive(Resource)]
pub struct BackendInfoResource(pub Receiver<DynBackendInfo>);

#[derive(Resource)]
pub struct ViewModel {
    pub viewport_left: Pitch,
    pub viewport_right: Pitch,
    pub on_screen_keyboards: Toggle<OnScreenKeyboards>,
    pub scale_keyboard_colors: Vec<KeyColor>,
    pub reference_scl: Scl,
    pub odd_limit: u16,
}

#[derive(Clone, Copy)]
pub enum OnScreenKeyboards {
    Isomorphic,
    Scale,
    Reference,
    IsomorphicAndReference,
    ScaleAndReference,
    None,
}

impl ViewModel {
    pub fn pitch_range(&self) -> Ratio {
        Ratio::between_pitches(self.viewport_left, self.viewport_right)
    }

    pub fn hor_world_coord(&self, pitch: Pitch) -> f64 {
        Ratio::between_pitches(self.viewport_left, pitch)
            .num_equal_steps_of_size(self.pitch_range())
            - 0.5
    }
}
